//! Platform-neutral contract for Foundation WebAssembly exports.
//!
//! Consumers build their own WASM module and browser glue, then use this API
//! to describe and validate the files that make up the published web package.
//! The same manifest is consumed by the browser runtime host.

use crate::RuntimeError;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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

    pub fn with_fallbacks(
        mut self,
        fallbacks: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
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

/// Builder used by engines and games to define their own Foundation Web export.
#[derive(Debug, Clone)]
pub struct WebExportBuilder {
    manifest: WebExportManifest,
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
        }
    }

    /// Declare the files generated by the consuming crate's WASM build.
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

    /// Add an image, audio or game-data resource to the published package.
    pub fn asset(mut self, asset: WebExportAsset) -> Self {
        self.manifest.assets.push(asset);
        self
    }

    /// Validate and finish the export descriptor.
    pub fn build(self) -> Result<WebExportManifest, RuntimeError> {
        self.manifest.validate()?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_manifest() -> WebExportManifest {
        WebExportBuilder::new("street-chaves", "Street Chaves")
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
            .unwrap()
    }

    #[test]
    fn builder_serializes_and_parser_round_trips_the_export_contract() {
        let manifest = sample_manifest();
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
}
