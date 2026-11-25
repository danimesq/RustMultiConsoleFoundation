# RustMultiConsoleFoundation

A reusable Rust game-development foundation for exploring native, web, legacy desktop, and classic-console targets. The project focuses on shared game logic with target-specific runtimes, renderers, assets, and build pipelines.

## 🌈 Packaging

The Foundation includes a Rust packaging adapter powered by [Cargo Packager](https://github.com/crabnebula-dev/cargo-packager). It currently exposes a Foundation-owned CLI and a library API for reading TOML configuration and generating platform packages.

### Windows NSIS installer

1. Build the game executable for the intended Windows target.
2. Copy `Packager.windows.example.toml` to `Packager.toml` in the game project.
3. Set the product metadata, executable name, and paths in `Packager.toml`.
4. Install the required platform packaging tools, including NSIS.
5. From the game project root, run:

```sh
cargo run -p foundation-packaging -- --config Packager.toml
```

The generated installer is written to the configured `out-dir`. Cargo Packager does not compile the game automatically in this integration: the game build must finish before packaging begins.

The adapter pins Cargo Packager to version `0.11.8` and disables its CLI feature because the Foundation calls its Rust library API directly. The external NSIS/WiX toolchain and target-specific requirements remain separate prerequisites.

## 🌈 Documentation

- [Rust Gaming Eclipse: shared architecture and integration contract](docs/rust-gaming-eclipse.md)

- [Multi-console foundation: architecture and scope](docs/multi-console-foundation.md)
- [Multi-backend platform abstraction and target-specific SDL selection](docs/multi-backend-platform-abstraction.md)
- [Reproducible builds: architecture and implementation plan](docs/reproducible-builds-plan.md)
- [Target platform and toolchain matrix](docs/target-platform-matrix.md)
- [Asset pipeline and target capability profiles](docs/asset-and-capability-pipeline.md)
- [Legacy Windows: Rust and SDL compatibility](docs/legacy-windows-sdl-rust.md)
- [Multi-console implementation roadmap](docs/multiconsole-roadmap.md)
- [Byte-perfect ROM reconstruction and C-to-Rust research](docs/byte-perfect-rom-reconstruction.md)
- [Impossible Engine compiler architecture](docs/impossible-engine-architecture.md)
- [Built-in multiPong reference game and Rust Pong research](docs/builtin-multipong-reference.md)

## 🌈 Research status

The documents describe research findings, architectural proposals, and validation plans. They do not imply that every listed console, operating system, renderer, or SDK is already implemented or tested by this repository.

The first intended game consumers are StreetChavesRust and PointBlankRust. The Foundation is designed to remain reusable by other games and tools as well.
