//! Platform-neutral builder and contract for Foundation WebAssembly exports.
//!
//! Consumers build their own WASM module and browser glue, then use this API
//! to stage those files and their resources into a portable web package.
//! The same manifest is consumed by the browser runtime host.

use crate::RuntimeError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const WEB_EXPORT_SCHEMA_VERSION: u32 = 1;
pub const WEB_EXPORT_MANIFEST_FILE: &str = "web-export.json";

fn default_asset_volume() -> i32 {
    100
}

/// Identity of the application being published.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebExportApplication {
    pub id: String,
    pub name: String,
}

/// Files produced by the consumer's wasm-bindgen/wasm-pack build.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebWasmBundle {
    /// JavaScript glue generated for the consumer's WASM module.
    pub javascript_glue: String,
    /// The compiled WebAssembly module generated for the consumer.
    pub wasm_module: String,
}

/// A complete, portable description of a Foundation Web/WASM export.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebExportManifest {
    pub schema_version: u32,
    pub application: WebExportApplication,
    pub wasm: WebWasmBundle,
    #[serde(default)]
    pub assets: Vec<WebExportAsset>,
}

/// One resource with a stable ID and a path relative to web-export.json.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebExportAsset {
    pub id: String,
    pub path: String,
    /// Alternate encodings or URLs, attempted in order when the primary fails.
    #[serde(default)]
    pub fallbacks: Vec<String>,
    /// Audio gain percentage, clamped by the runtime to the shared 0..=400 range.
    /// Ignored for image/data entries.
    #[serde(default = "default_asset_volume")]
    pub volume_percent: i32,
    pub kind: WebAssetKind,
}

impl WebExportAsset {
    pub fn new(id: impl Into<String>, path: impl Into<String>, kind: WebAssetKind) -> Self {
        Self {
            id: id.into(),
            path: path.into(),
            fallbacks: Vec::new(),
            volume_percent: 100,
            kind,
        }
    }

    pub fn with_fallbacks<I, S>(mut self, fallbacks: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.fallbacks = fallbacks.into_iter().map(Into::into).collect();
        self
    }

    pub fn with_volume_percent(mut self, volume_percent: i32) -> Self {
        self.volume_percent = volume_percent;
        self
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WebAssetKind {
    Image,
    Audio,
    Data,
}

impl WebExportManifest {
    /// Parse JSON and validate every package-relative path and resource ID.
    pub fn parse(json: &str) -> Result<Self, RuntimeError> {
        let manifest: Self = serde_json::from_str(json)
            .map_err(|error| RuntimeError::Host(format!("invalid web export manifest: {error}")))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Serialize the manifest ready to be written as web-export.json.
    pub fn to_json(&self) -> Result<String, RuntimeError> {
        self.validate()?;
        serde_json::to_string_pretty(self)
            .map_err(|error| RuntimeError::Host(format!("could not serialize web export: {error}")))
    }

    pub fn validate(&self) -> Result<(), RuntimeError> {
        if self.schema_version != WEB_EXPORT_SCHEMA_VERSION {
            return Err(RuntimeError::Unsupported("web export manifest schema version"));
        }
        validate_application_id(&self.application.id)?;
        if self.application.name.trim().is_empty() || self.application.name.len() > 256 {
            return Err(RuntimeError::InvalidConfig("invalid web export application name"));
        }
        validate_web_export_path(&self.wasm.javascript_glue)?;
        validate_web_export_path(&self.wasm.wasm_module)?;
        if self.wasm.javascript_glue == self.wasm.wasm_module {
            return Err(RuntimeError::InvalidConfig("WASM module and JavaScript glue paths must differ"));
        }

        let mut ids = HashSet::<String>::new();
        for asset in &self.assets {
            if asset.id.is_empty()
                || asset.id.len() > 128
                || !asset.id.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'/')
                })
            {
                return Err(RuntimeError::InvalidConfig("invalid web export asset ID"));
            }
            if !ids.insert(asset.id.clone()) {
                return Err(RuntimeError::Host(format!(
                    "duplicate web export asset ID: {}",
                    asset.id
                )));
            }
            validate_web_export_path(&asset.path)?;
            for fallback in &asset.fallbacks {
                validate_web_export_path(fallback)?;
            }
            if !(0..=400).contains(&asset.volume_percent) {
                return Err(RuntimeError::InvalidConfig("asset volume must be between 0 and 400"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct PackageFile {
    source: PathBuf,
    destination: String,
}

/// Builder used by engines and games to stage their own Foundation Web export.
#[derive(Debug, Clone)]
pub struct WebExportBuilder {
    manifest: WebExportManifest,
    files: Vec<PackageFile>,
}

impl WebExportBuilder {
    pub fn new(application_id: impl Into<String>, application_name: impl Into<String>) -> Self {
        Self {
            manifest: WebExportManifest {
                schema_version: WEB_EXPORT_SCHEMA_VERSION,
                application: WebExportApplication {
                    id: application_id.into(),
                    name: application_name.into(),
                },
                wasm: WebWasmBundle {
                    javascript_glue: String::new(),
                    wasm_module: String::new(),
                },
                assets: Vec::new(),
            },
            files: Vec::new(),
        }
    }

    /// Declare generated bundle paths without registering their source files yet.
    pub fn wasm_bundle(
        mut self,
        javascript_glue: impl Into<String>,
        wasm_module: impl Into<String>,
    ) -> Self {
        self.manifest.wasm = WebWasmBundle {
            javascript_glue: javascript_glue.into(),
            wasm_module: wasm_module.into(),
        };
        self
    }

    /// Register the generated JavaScript/WASM source files and their package paths.
    pub fn wasm_bundle_files(
        self,
        javascript_source: impl Into<PathBuf>,
        wasm_source: impl Into<PathBuf>,
        javascript_destination: impl Into<String>,
        wasm_destination: impl Into<String>,
    ) -> Self {
        let javascript_destination = javascript_destination.into();
        let wasm_destination = wasm_destination.into();
        self.wasm_bundle(
            javascript_destination.clone(),
            wasm_destination.clone(),
        )
        .package_file(javascript_source, javascript_destination)
        .package_file(wasm_source, wasm_destination)
    }

    /// Add an image, audio or game-data resource to the manifest.
    pub fn asset(mut self, asset: WebExportAsset) -> Self {
        self.manifest.assets.push(asset);
        self
    }

    /// Add a resource and register the source file to copy into the package.
    pub fn asset_file(
        self,
        asset: WebExportAsset,
        source: impl Into<PathBuf>,
    ) -> Self {
        let destination = asset.path.clone();
        self.asset(asset).package_file(source, destination)
    }

    /// Register any additional package file, such as index.html or main.js.
    pub fn package_file(
        mut self,
        source: impl Into<PathBuf>,
        destination: impl Into<String>,
    ) -> Self {
        self.files.push(PackageFile {
            source: source.into(),
            destination: destination.into(),
        });
        self
    }

    /// Validate and return the manifest without writing package files.
    pub fn build(self) -> Result<WebExportManifest, RuntimeError> {
        self.manifest.validate()?;
        Ok(self.manifest)
    }

    /// Create a deployable directory package, copying all declared files and writing the manifest.
    ///
    /// The consuming project must register a source file for the WASM glue, the WASM module,
    /// every primary asset and every declared fallback before calling this method.
    /// A ZIP/archive writer is intentionally a separate publishing layer.
    pub fn write_to(self, output_dir: impl AsRef<Path>) -> Result<WebExportManifest, RuntimeError> {
        self.manifest.validate()?;
        let root = output_dir.as_ref();
        fs::create_dir_all(root).map_err(|error| io_error("create export directory", error))?;
        let root = fs::canonicalize(root).map_err(|error| io_error("resolve export directory", error))?;

        let mut registered = HashSet::<String>::new();
        for file in &self.files {
            validate_web_export_path(&file.destination)?;
            if file.destination == WEB_EXPORT_MANIFEST_FILE {
                return Err(RuntimeError::InvalidConfig("web-export.json is reserved for the manifest"));
            }
            if !registered.insert(file.destination.clone()) {
                return Err(RuntimeError::Host(format!(
                    "duplicate Web export destination: {}",
                    file.destination
                )));
            }
        }

        let mut required = HashSet::<String>::new();
        required.insert(self.manifest.wasm.javascript_glue.clone());
        required.insert(self.manifest.wasm.wasm_module.clone());
        for asset in &self.manifest.assets {
            required.insert(asset.path.clone());
            required.extend(asset.fallbacks.iter().cloned());
        }
        for path in &required {
            if !registered.contains(path) {
                return Err(RuntimeError::Host(format!(
                    "no source file registered for Web export path: {path}"
                )));
            }
        }

        for file in &self.files {
            copy_into_package(&root, file)?;
        }

        let manifest_json = self.manifest.to_json()?;
        let manifest_path = root.join(WEB_EXPORT_MANIFEST_FILE);
        if fs::symlink_metadata(&manifest_path)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return Err(RuntimeError::InvalidConfig("manifest destination must not be a symbolic link"));
        }
        fs::write(&manifest_path, manifest_json)
            .map_err(|error| io_error("write Web export manifest", error))?;

        Ok(self.manifest)
    }
}

fn validate_application_id(id: &str) -> Result<(), RuntimeError> {
    if id.is_empty()
        || id.len() > 128
        || !id.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.')
        })
    {
        return Err(RuntimeError::InvalidConfig("invalid web export application ID"));
    }
    Ok(())
}

/// Ensure an export path stays inside the package directory.
pub fn validate_web_export_path(path: &str) -> Result<(), RuntimeError> {
    if path.is_empty()
        || path.starts_with('/')
        || path.starts_with("//")
        || path.contains('\\')
        || path.contains(':')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || path.chars().any(char::is_control)
    {
        return Err(RuntimeError::InvalidConfig(
            "export paths must be safe relative paths inside the web package",
        ));
    }
    Ok(())
}

fn copy_into_package(root: &Path, file: &PackageFile) -> Result<(), RuntimeError> {
    let target = root.join(&file.destination);
    let parent = target
        .parent()
        .ok_or(RuntimeError::InvalidConfig("invalid Web export destination"))?;
    fs::create_dir_all(parent).map_err(|error| io_error("create package subdirectory", error))?;
    let canonical_parent =
        fs::canonicalize(parent).map_err(|error| io_error("resolve package subdirectory", error))?;
    if !canonical_parent.starts_with(root) {
        return Err(RuntimeError::InvalidConfig("package destination escapes the export directory"));
    }
    if fs::symlink_metadata(&target)
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
    {
        return Err(RuntimeError::InvalidConfig("package destination must not be a symbolic link"));
    }
    fs::copy(&file.source, &target).map_err(|error| {
        RuntimeError::Host(format!(
            "copy {} to {}: {error}",
            file.source.display(),
            target.display()
        ))
    })?;
    Ok(())
}

fn io_error(context: &str, error: io::Error) -> RuntimeError {
    RuntimeError::Host(format!("{context}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn temporary_directory(label: &str) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        std::env::temp_dir().join(format!(
            "foundation-web-export-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn builder_serializes_and_parser_round_trips_the_export_contract() {
        let manifest = WebExportBuilder::new("street-chaves", "Street Chaves")
            .wasm_bundle("pkg/street_chaves.js", "pkg/street_chaves_bg.wasm")
            .asset(WebExportAsset::new(
                "stage_01",
                "assets/images/stages/stage-01.webp",
                WebAssetKind::Image,
            ))
            .asset(
                WebExportAsset::new(
                    "fight_music",
                    "assets/audio/music/theme.ogg",
                    WebAssetKind::Audio,
                )
                .with_fallbacks(["assets/audio/music/theme.m4a", "assets/audio/music/theme.mp3"]),
            )
            .build()
            .unwrap();
        let json = manifest.to_json().unwrap();
        let parsed = WebExportManifest::parse(&json).unwrap();
        assert_eq!(parsed, manifest);
        assert!(json.contains("\"wasm_module\": \"pkg/street_chaves_bg.wasm\""));
    }

    #[test]
    fn rejects_paths_that_escape_the_export_directory() {
        let manifest = WebExportBuilder::new("game", "Game")
            .wasm_bundle("../app.js", "pkg/app.wasm")
            .build();
        assert!(manifest.is_err());
    }

    #[test]
    fn rejects_duplicate_asset_ids_and_unsafe_fallbacks() {
        let duplicate = WebExportBuilder::new("game", "Game")
            .wasm_bundle("pkg/app.js", "pkg/app.wasm")
            .asset(WebExportAsset::new("hero", "assets/hero.png", WebAssetKind::Image))
            .asset(WebExportAsset::new("hero", "assets/other.png", WebAssetKind::Image))
            .build();
        assert!(duplicate.is_err());

        let unsafe_fallback = WebExportBuilder::new("game", "Game")
            .wasm_bundle("pkg/app.js", "pkg/app.wasm")
            .asset(
                WebExportAsset::new("music", "assets/music.ogg", WebAssetKind::Audio)
                    .with_fallbacks(["../../outside.mp3"]),
            )
            .build();
        assert!(unsafe_fallback.is_err());
    }

    #[test]
    fn write_to_stages_the_wasm_bundle_assets_and_manifest() {
        let root = temporary_directory("staging");
        let source = root.join("source");
        let output = root.join("output");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("game.js"), "exported-js").unwrap();
        fs::write(source.join("game.wasm"), [0, 97, 115, 109]).unwrap();
        fs::write(source.join("hero.png"), [1, 2, 3]).unwrap();

        let manifest = WebExportBuilder::new("game", "Game")
            .wasm_bundle_files(
                source.join("game.js"),
                source.join("game.wasm"),
                "pkg/game.js",
                "pkg/game.wasm",
            )
            .asset_file(
                WebExportAsset::new("hero", "assets/hero.png", WebAssetKind::Image),
                source.join("hero.png"),
            )
            .package_file(source.join("game.js"), "main.js")
            .write_to(&output)
            .unwrap();

        assert_eq!(fs::read(output.join("pkg/game.js")).unwrap(), b"exported-js");
        assert_eq!(fs::read(output.join("pkg/game.wasm")).unwrap(), [0, 97, 115, 109]);
        assert_eq!(fs::read(output.join("assets/hero.png")).unwrap(), [1, 2, 3]);
        assert!(output.join("main.js").is_file());
        assert_eq!(
            WebExportManifest::parse(&fs::read_to_string(output.join(WEB_EXPORT_MANIFEST_FILE)).unwrap())
                .unwrap(),
            manifest
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn write_to_rejects_missing_sources_and_duplicate_destinations() {
        let root = temporary_directory("validation");
        let missing = WebExportBuilder::new("game", "Game")
            .wasm_bundle("pkg/game.js", "pkg/game.wasm")
            .write_to(&root);
        assert!(missing.is_err());

        let duplicate = WebExportBuilder::new("game", "Game")
            .wasm_bundle_files("game.js", "game.wasm", "pkg/game.js", "pkg/game.wasm")
            .package_file("other.js", "pkg/game.js")
            .write_to(&root);
        assert!(duplicate.is_err());
        let _ = fs::remove_dir_all(root);
    }
}
