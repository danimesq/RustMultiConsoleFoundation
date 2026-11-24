//! Cargo Packager integration for RustMultiConsoleFoundation.
//!
//! The Foundation owns the stable entry point and configuration loading;
//! Cargo Packager owns the platform-specific installer generation.

use std::error::Error;
use std::fs;
use std::io;
use std::path::Path;

/// Load a Cargo Packager configuration from a TOML file.
pub fn load_config(path: impl AsRef<Path>) -> Result<cargo_packager::Config, Box<dyn Error>> {
    let path = path.as_ref();
    let source = fs::read_to_string(path)?;
    let config = toml::from_str::<cargo_packager::Config>(&source).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid packager configuration {}: {error}", path.display()),
        )
    })?;

    Ok(config)
}

/// Generate packages from a Cargo Packager TOML configuration.
///
/// Paths inside the configuration are interpreted by Cargo Packager relative
/// to the process working directory, so invoke this from the game project root.
pub fn package_from_file(path: impl AsRef<Path>) -> Result<usize, Box<dyn Error>> {
    let config = load_config(path)?;
    let outputs = cargo_packager::package(&config)?;
    Ok(outputs.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_minimal_windows_installer_configuration() {
        let config = toml::from_str::<cargo_packager::Config>(
            r#"
product-name = "Foundation Demo"
version = "0.1.0"
identifier = "org.example.foundation-demo"
out-dir = "dist"
binaries-dir = "target/release"
target-triple = "x86_64-pc-windows-msvc"
formats = ["nsis"]

[[binaries]]
path = "foundation-demo.exe"
main = true
"#,
        )
        .expect("the sample configuration should deserialize");

        assert_eq!(config.product_name, "Foundation Demo");
        assert_eq!(config.version, "0.1.0");
        assert_eq!(
            config.target_triple.as_deref(),
            Some("x86_64-pc-windows-msvc")
        );
        assert_eq!(config.binaries.len(), 1);
    }
}
