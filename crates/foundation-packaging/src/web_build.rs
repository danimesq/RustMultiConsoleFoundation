//! Source-driven WebAssembly builds and Web package publishing.
//!
//! The game project supplies its Cargo crate, a browser bootstrap module and
//! an asset-source catalog. Foundation compiles the crate, runs wasm-bindgen,
//! generates the HTML shell, stages assets, writes web-export.json and creates
//! a ZIP archive. Consumers do not compile or package the browser build
//! themselves.

use foundation_runtime::web_export::{
    WebAssetKind, WebExportAsset, WebExportBuilder, WebExportManifest,
};
use serde::Deserialize;
use std::error::Error;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

pub const WEB_BUILD_CONFIG_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct WebBuildConfig {
    pub schema_version: u32,
    pub application: WebBuildApplication,
    pub build: WebBuildCargo,
    pub assets_root: PathBuf,
    pub web: WebBuildWeb,
    #[serde(default)]
    pub assets: Vec<WebBuildAsset>,
    pub output: WebBuildOutput,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct WebBuildApplication {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct WebBuildCargo {
    /// Directory containing the consumer Cargo workspace/package.
    pub project_dir: PathBuf,
    /// Cargo manifest relative to project_dir.
    #[serde(default = "default_cargo_manifest")]
    pub manifest_path: PathBuf,
    /// Cargo workspace package name passed to `cargo build --package`.
    pub package: String,
    /// Rust library crate name used for the generated .wasm and JS glue.
    pub crate_name: String,
    #[serde(default)]
    pub features: Vec<String>,
    #[serde(default = "default_wasm_target")]
    pub target: String,
    #[serde(default = "default_profile")]
    pub profile: String,
    /// Optional Cargo target directory. Defaults to project_dir/target.
    pub target_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct WebBuildWeb {
    pub title: String,
    #[serde(default = "default_canvas_id")]
    pub canvas_id: String,
    /// Consumer-provided JS entry module. If omitted, Foundation generates a
    /// minimal module that initializes wasm-bindgen output.
    pub bootstrap: Option<PathBuf>,
    /// Optional user-gesture button rendered by Foundation's generated HTML shell.
    pub audio_button_id: Option<String>,
    /// Optional live diagnostics element rendered by Foundation's generated HTML shell.
    pub diagnostics_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct WebBuildAsset {
    pub id: String,
    /// Source path relative to assets_root. Must remain inside that directory.
    pub source: PathBuf,
    /// Published path relative to web-export.json.
    pub path: String,
    pub kind: WebAssetKind,
    #[serde(default)]
    pub volume_percent: Option<i32>,
    #[serde(default)]
    pub fallbacks: Vec<WebBuildAssetFallback>,
    /// Optional upstream repository/file URL retained in build provenance.
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct WebBuildAssetFallback {
    pub source: PathBuf,
    pub path: String,
    pub source_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct WebBuildOutput {
    /// Published directory, relative to project_dir unless absolute.
    pub directory: PathBuf,
    /// Optional ZIP archive, relative to project_dir unless absolute.
    pub archive: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct WebBuildReport {
    pub manifest: WebExportManifest,
    pub output_directory: PathBuf,
    pub archive: Option<PathBuf>,
    pub wasm_target: String,
    pub package_name: String,
}

#[derive(Debug, Clone)]
struct ProvenanceEntry {
    id: String,
    published_path: String,
    source_url: String,
}

struct TemporaryDirectory(PathBuf);

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(1);

/// Load and validate a TOML Web build configuration.
pub fn load_web_build_config(path: impl AsRef<Path>) -> Result<WebBuildConfig, Box<dyn Error>> {
    let path = path.as_ref();
    let source = fs::read_to_string(path)?;
    let config: WebBuildConfig = toml::from_str(&source).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid Foundation Web build config {}: {error}", path.display()),
        )
    })?;
    validate_config(&config)?;
    Ok(config)
}

/// Compile a consumer crate and publish a complete WebAssembly directory + ZIP.
pub fn build_web_from_file(
    config_path: impl AsRef<Path>,
) -> Result<WebBuildReport, Box<dyn Error>> {
    let config_path = config_path.as_ref();
    let config = load_web_build_config(config_path)?;
    let config_root = config_path
        .canonicalize()?
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "config has no parent directory"))?
        .to_path_buf();

    let project_root = resolve_from(&config_root, &config.build.project_dir);
    let project_root = project_root.canonicalize()?;
    let cargo_manifest = project_root.join(&config.build.manifest_path);
    if !cargo_manifest.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Cargo manifest not found: {}", cargo_manifest.display()),
        )
        .into());
    }

    let assets_root = resolve_from(&project_root, &config.assets_root).canonicalize()?;
    if !assets_root.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("asset source directory not found: {}", assets_root.display()),
        )
        .into());
    }

    validate_asset_sources(&config.assets, &assets_root)?;

    let target_dir = match &config.build.target_dir {
        Some(path) => resolve_from(&project_root, path),
        None => project_root.join("target"),
    };

    let profile_directory = match config.build.profile.as_str() {
        "release" => "release",
        "debug" => "debug",
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "build.profile must be either 'release' or 'debug'",
            )
            .into())
        }
    };

    let wasm_path = target_dir
        .join(&config.build.target)
        .join(profile_directory)
        .join(format!("{}.wasm", config.build.crate_name));

    run_cargo_build(&config, &project_root, &cargo_manifest, &target_dir)?;
    if !wasm_path.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "Cargo completed, but the expected WASM artifact was not created: {}.                  Ensure the selected library target has crate-type = [\"cdylib\"] and crate-name matches the .wasm output.",
                wasm_path.display()
            ),
        )
        .into());
    }

    let temporary_root = unique_temp_path("foundation-web-build");
    fs::create_dir_all(&temporary_root)?;
    let _temporary = TemporaryDirectory(temporary_root.clone());
    let generated_package = temporary_root.join("pkg");
    fs::create_dir_all(&generated_package)?;

    run_wasm_bindgen(&config, &project_root, &wasm_path, &generated_package)?;

    let javascript_name = format!("{}.js", config.build.crate_name);
    let wasm_name = format!("{}_bg.wasm", config.build.crate_name);
    let javascript_source = generated_package.join(&javascript_name);
    let wasm_source = generated_package.join(&wasm_name);
    if !javascript_source.is_file() || !wasm_source.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "wasm-bindgen did not produce the expected pair {} and {}",
                javascript_source.display(),
                wasm_source.display()
            ),
        )
        .into());
    }

    let output_directory = resolve_from(&project_root, &config.output.directory);
    if output_directory == project_root
        || output_directory.starts_with(&assets_root)
        || assets_root.starts_with(&output_directory)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Web output directory must be separate from the project and asset source directories",
        )
        .into());
    }
    let output_parent = output_directory.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "output directory has no parent")
    })?;
    fs::create_dir_all(output_parent)?;
    let staged_output = unique_sibling_path(&output_directory, "staging");
    let _staged_output = TemporaryDirectory(staged_output.clone());
    let mut builder = WebExportBuilder::new(
        config.application.id.clone(),
        config.application.name.clone(),
    )
    .wasm_bundle_files(
        &javascript_source,
        &wasm_source,
        format!("pkg/{javascript_name}"),
        format!("pkg/{wasm_name}"),
    );

    let mut provenance = Vec::<ProvenanceEntry>::new();
    for asset in &config.assets {
        let source = canonical_asset_source(&assets_root, &asset.source)?;
        let mut published_asset = WebExportAsset::new(
            asset.id.clone(),
            asset.path.clone(),
            asset.kind,
        );
        if let Some(volume) = asset.volume_percent {
            published_asset = published_asset.with_volume_percent(volume);
        }
        if !asset.fallbacks.is_empty() {
            published_asset = published_asset.with_fallbacks(
                asset.fallbacks.iter().map(|fallback| fallback.path.clone()),
            );
        }
        builder = builder.asset_file(published_asset, source);

        if let Some(source_url) = &asset.source_url {
            provenance.push(ProvenanceEntry {
                id: asset.id.clone(),
                published_path: asset.path.clone(),
                source_url: source_url.clone(),
            });
        }

        for fallback in &asset.fallbacks {
            let fallback_source = canonical_asset_source(&assets_root, &fallback.source)?;
            builder = builder.package_file(fallback_source, fallback.path.clone());
            if let Some(source_url) = &fallback.source_url {
                provenance.push(ProvenanceEntry {
                    id: asset.id.clone(),
                    published_path: fallback.path.clone(),
                    source_url: source_url.clone(),
                });
            }
        }
    }

    let generated_html = temporary_root.join("index.html");
    fs::write(
        &generated_html,
        generate_html(
            &config.web.title,
            &config.web.canvas_id,
            config.web.audio_button_id.as_deref(),
            config.web.diagnostics_id.as_deref(),
        ),
    )?;
    let generated_bootstrap = temporary_root.join("main.js");
    if let Some(bootstrap) = &config.web.bootstrap {
        let bootstrap_source = resolve_from(&project_root, bootstrap);
        if !bootstrap_source.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Web bootstrap module not found: {}", bootstrap_source.display()),
            )
            .into());
        }
        builder = builder.package_file(bootstrap_source, "main.js");
    } else {
        fs::write(
            &generated_bootstrap,
            format!(
                "import init from \"./pkg/{}.js\";\n\
                 await init();\n",
                config.build.crate_name
            ),
        )?;
        builder = builder.package_file(&generated_bootstrap, "main.js");
    }
    builder = builder.package_file(&generated_html, "index.html");

    let manifest = builder
        .write_to(&staged_output)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;

    if !provenance.is_empty() {
        let provenance_path = staged_output.join("web-build-provenance.json");
        let entries: Vec<_> = provenance
            .iter()
            .map(|entry| {
                serde_json::json!({
                    "id": entry.id,
                    "published_path": entry.published_path,
                    "source_url": entry.source_url
                })
            })
            .collect();
        fs::write(
            provenance_path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema_version": 1,
                "assets": entries
            }))?,
        )?;
    }

    if output_directory.exists() {
        fs::remove_dir_all(&output_directory)?;
    }
    fs::rename(&staged_output, &output_directory)?;

    let archive = config.output.archive.as_ref().map(|path| resolve_from(&project_root, path));
    if let Some(archive_path) = &archive {
        if archive_path.starts_with(&output_directory) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Web ZIP archive must be outside the directory it archives",
            )
            .into());
        }
        create_zip_archive(&output_directory, archive_path)?;
    }

    Ok(WebBuildReport {
        manifest,
        output_directory,
        archive,
        wasm_target: config.build.target,
        package_name: config.build.package,
    })
}

fn validate_config(config: &WebBuildConfig) -> Result<(), Box<dyn Error>> {
    if config.schema_version != WEB_BUILD_CONFIG_SCHEMA_VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "unsupported Foundation Web build schema version {}; supported version is {}",
                config.schema_version, WEB_BUILD_CONFIG_SCHEMA_VERSION
            ),
        )
        .into());
    }
    if config.application.id.trim().is_empty() || config.application.name.trim().is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "application id and name are required").into());
    }
    if config.build.package.trim().is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "build.package is required").into());
    }
    if config.build.crate_name.is_empty()
        || !config.build.crate_name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "build.crate-name must be a Rust artifact name using ASCII letters, digits and underscores",
        )
        .into());
    }
    if config.build.target != "wasm32-unknown-unknown" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "this Web publisher currently supports wasm32-unknown-unknown",
        )
        .into());
    }
    if !matches!(config.build.profile.as_str(), "release" | "debug") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "build.profile must be either 'release' or 'debug'",
        )
        .into());
    }
    if config.web.title.trim().is_empty() || config.web.canvas_id.trim().is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "web.title and web.canvas-id are required").into());
    }
    for (label, id) in [
        ("web.audio-button-id", config.web.audio_button_id.as_deref()),
        ("web.diagnostics-id", config.web.diagnostics_id.as_deref()),
    ] {
        if id.is_some_and(|value| value.trim().is_empty()) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, format!("{label} cannot be empty")).into());
        }
    }
    let mut ids = std::collections::HashSet::new();
    for asset in &config.assets {
        if asset.id.trim().is_empty() || !ids.insert(asset.id.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("empty or duplicate Web asset ID: {}", asset.id),
            )
            .into());
        }
        if asset.volume_percent.unwrap_or(100) < 0 || asset.volume_percent.unwrap_or(100) > 400 {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "asset volume must be between 0 and 400").into());
        }
        for path in std::iter::once(&asset.path).chain(asset.fallbacks.iter().map(|fallback| &fallback.path)) {
            foundation_runtime::web_export::validate_web_export_path(path)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error.to_string()))?;
        }
    }
    Ok(())
}

fn validate_asset_sources(assets: &[WebBuildAsset], root: &Path) -> Result<(), Box<dyn Error>> {
    for asset in assets {
        canonical_asset_source(root, &asset.source)?;
        for fallback in &asset.fallbacks {
            canonical_asset_source(root, &fallback.source)?;
        }
    }
    Ok(())
}

fn canonical_asset_source(root: &Path, source: &Path) -> Result<PathBuf, Box<dyn Error>> {
    if source.components().any(|component| matches!(component, Component::ParentDir | Component::RootDir | Component::Prefix(_))) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("asset source must be a relative path inside assets_root: {}", source.display()),
        )
        .into());
    }
    let canonical = root.join(source).canonicalize()?;
    if !canonical.starts_with(root) || !canonical.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("asset source escapes assets_root or is not a file: {}", source.display()),
        )
        .into());
    }
    Ok(canonical)
}

fn default_cargo_manifest() -> PathBuf {
    PathBuf::from("Cargo.toml")
}

fn default_wasm_target() -> String {
    "wasm32-unknown-unknown".to_owned()
}

fn default_profile() -> String {
    "release".to_owned()
}

fn default_canvas_id() -> String {
    "game-canvas".to_owned()
}

fn resolve_from(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn run_cargo_build(
    config: &WebBuildConfig,
    project_root: &Path,
    cargo_manifest: &Path,
    target_dir: &Path,
) -> Result<(), Box<dyn Error>> {
    let mut command = Command::new("cargo");
    command
        .arg("build")
        .arg("--manifest-path")
        .arg(cargo_manifest)
        .arg("--package")
        .arg(&config.build.package)
        .arg("--lib")
        .arg("--target")
        .arg(&config.build.target)
        .arg("--target-dir")
        .arg(target_dir);

    if config.build.profile == "release" {
        command.arg("--release");
    }
    if !config.build.features.is_empty() {
        command.arg("--features").arg(config.build.features.join(","));
    }

    let output = command
        .current_dir(project_root)
        .output()
        .map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("could not start Cargo Web build: {error}"),
            )
        })?;
    ensure_success("cargo build", output)?;
    Ok(())
}

fn run_wasm_bindgen(
    config: &WebBuildConfig,
    project_root: &Path,
    wasm_path: &Path,
    output_directory: &Path,
) -> Result<(), Box<dyn Error>> {
    let output = Command::new("wasm-bindgen")
        .arg(wasm_path)
        .arg("--target")
        .arg("web")
        .arg("--out-dir")
        .arg(output_directory)
        .arg("--out-name")
        .arg(&config.build.crate_name)
        .arg("--no-typescript")
        .current_dir(project_root)
        .output()
        .map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("could not start wasm-bindgen; install the matching wasm-bindgen CLI: {error}"),
            )
        })?;
    ensure_success("wasm-bindgen", output)?;
    Ok(())
}

fn ensure_success(program: &str, output: Output) -> Result<(), Box<dyn Error>> {
    if output.status.success() {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::Other,
        format!(
            "{program} failed with {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ),
    )
    .into())
}

fn generate_html(
    title: &str,
    canvas_id: &str,
    audio_button_id: Option<&str>,
    diagnostics_id: Option<&str>,
) -> String {
    let title = escape_html(title);
    let canvas_id = escape_html(canvas_id);
    let audio_button = audio_button_id
        .map(|id| {
            format!(
                "    <button id=\"{}\" type=\"button\">Enable audio</button>\\n",
                escape_html(id)
            )
        })
        .unwrap_or_default();
    let diagnostics = diagnostics_id
        .map(|id| {
            format!(
                "    <pre id=\"{}\" aria-live=\"polite\"></pre>\\n",
                escape_html(id)
            )
        })
        .unwrap_or_default();
    format!(
        "<!doctype html>\\n<html lang=\"en\">\\n<head>\\n  <meta charset=\"utf-8\">\\n  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\\n  <title>{title}</title>\\n  <style>\\n    html, body {{ margin: 0; min-height: 100%; background: #101522; }}\\n    body {{ min-height: 100vh; display: grid; place-items: center; }}\\n    main {{ display: grid; justify-items: center; gap: 1rem; }}\\n    canvas {{ display: block; width: min(100vw, 133.333vh); aspect-ratio: 4 / 3; image-rendering: pixelated; }}\\n  </style>\\n</head>\\n<body>\\n  <main>\\n    <canvas id=\"{canvas_id}\" width=\"400\" height=\"300\"></canvas>\\n{audio_button}{diagnostics}  </main>\\n  <script type=\"module\" src=\"./main.js\"></script>\\n</body>\\n</html>\\n"
    )
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn create_zip_archive(directory: &Path, archive_path: &Path) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = archive_path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    if archive_path.exists() {
        fs::remove_file(archive_path)?;
    }
    let file = File::create(archive_path)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    add_directory_to_zip(&mut zip, directory, directory, options)?;
    zip.finish()?;
    Ok(())
}

fn add_directory_to_zip(
    zip: &mut ZipWriter<File>,
    root: &Path,
    current: &Path,
    options: SimpleFileOptions,
) -> Result<(), Box<dyn Error>> {
    let mut entries: Vec<_> = fs::read_dir(current)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "symlinks are not allowed in Web output").into());
        }
        if metadata.is_dir() {
            add_directory_to_zip(zip, root, &path, options)?;
        } else if metadata.is_file() {
            let name = path
                .strip_prefix(root)?
                .components()
                .map(|component| component.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            zip.start_file(name, options)?;
            let mut source = File::open(&path)?;
            io::copy(&mut source, zip)?;
        }
    }
    Ok(())
}

fn unique_temp_path(prefix: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

fn unique_sibling_path(output: &Path, suffix: &str) -> PathBuf {
    let parent = output.parent().unwrap_or_else(|| Path::new("."));
    let name = output.file_name().unwrap_or_else(|| OsStr::new("web"));
    parent.join(format!(
        ".{}.{}-{}-{}",
        name.to_string_lossy(),
        suffix,
        std::process::id(),
        NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_html_with_escaped_title_and_canvas_id() {
        let html = generate_html("Game <Night>", "game&canvas", Some("enable-audio"), Some("diagnostics"));
        assert!(html.contains("Game &lt;Night&gt;"));
        assert!(html.contains("id=\"game&amp;canvas\""));
        assert!(html.contains("./main.js"));
        assert!(html.contains("id=\"enable-audio\""));
        assert!(html.contains("id=\"diagnostics\""));
    }

    #[test]
    fn source_paths_must_stay_inside_the_asset_root() {
        let root = std::env::temp_dir().join(format!(
            "foundation-web-sources-{}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("assets")).unwrap();
        fs::write(root.join("assets/ok.bin"), [1, 2, 3]).unwrap();
        assert!(canonical_asset_source(&root, Path::new("assets/ok.bin")).is_ok());
        assert!(canonical_asset_source(&root, Path::new("../outside.bin")).is_err());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn parses_a_source_driven_web_build_config() {
        let config = r#"
schema-version = 1
assets-root = "assets"

[application]
id = "demo-game"
name = "Demo Game"

[build]
project-dir = "."
manifest-path = "Cargo.toml"
package = "demo-game"
crate-name = "demo_game"
features = ["web-foundation"]
target = "wasm32-unknown-unknown"
profile = "release"

[web]
title = "Demo Game"
canvas-id = "demo-canvas"
bootstrap = "web/main.js"

[[assets]]
id = "hero"
source = "hero.bmp"
path = "assets/hero.bmp"
kind = "image"

[output]
directory = "dist/web"
archive = "dist/demo-game-web.zip"
"#;
        let config: WebBuildConfig = toml::from_str(config).unwrap();
        validate_config(&config).unwrap();
        assert_eq!(config.build.crate_name, "demo_game");
        assert_eq!(config.assets.len(), 1);
    }
}
