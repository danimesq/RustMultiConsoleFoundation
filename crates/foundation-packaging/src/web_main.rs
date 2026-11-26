//! Foundation-owned source-to-WebAssembly build and publishing CLI.

use std::error::Error;
use std::io;
use std::path::PathBuf;

fn main() {
    if let Err(error) = run() {
        eprintln!("foundation-web-build: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let mut config = PathBuf::from("FoundationWebBuild.toml");

    while let Some(argument) = args.next() {
        if argument == "--help" || argument == "-h" {
            print_help();
            return Ok(());
        }
        if argument == "--config" || argument == "-c" {
            config = args.next().map(PathBuf::from).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "--config requires a file path")
            })?;
            continue;
        }
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unknown argument: {}", argument.to_string_lossy()),
        )
        .into());
    }

    let report = foundation_packaging::web_build::build_web_from_file(&config)?;
    println!("Built '{}' for {}.", report.package_name, report.wasm_target);
    println!("Web directory: {}", report.output_directory.display());
    if let Some(archive) = report.archive {
        println!("Web archive: {}", archive.display());
    }
    println!(
        "Registered {} resources in web-export.json.",
        report.manifest.assets.len()
    );
    Ok(())
}

fn print_help() {
    println!(
        "foundation-web-build

Usage:
  cargo run -p foundation-packaging --bin foundation-web-build -- [--config <FoundationWebBuild.toml>]

Options:
  -c, --config <PATH>  Read a source-driven Foundation Web build config
  -h, --help           Show this help

Build requirements:
  - Rust/Cargo with the wasm32-unknown-unknown target installed
  - wasm-bindgen CLI matching the wasm-bindgen crate used by the game
  - A consumer Cargo package with a cdylib library target"
    );
}
