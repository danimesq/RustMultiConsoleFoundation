//! Small CLI wrapper so games can use one Foundation-owned packaging command.

use std::error::Error;
use std::io;
use std::path::PathBuf;

fn main() {
    if let Err(error) = run() {
        eprintln!("foundation-packager: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let mut config = PathBuf::from("Packager.toml");

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

    let count = foundation_packaging::package_from_file(&config)?;
    println!("Created {count} package(s).");
    Ok(())
}

fn print_help() {
    println!(
        "foundation-packager

Usage:
  cargo run -p foundation-packaging -- [--config <Packager.toml>]

Options:
  -c, --config <PATH>  Read packaging configuration (default: Packager.toml)
  -h, --help           Show this help"
    );
}
