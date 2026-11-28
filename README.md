# RustMultiConsoleFoundation

A reusable Rust game-development foundation for exploring native, web, legacy desktop, and classic-console targets. The project focuses on shared game logic with target-specific runtimes, renderers, assets, and build pipelines.

## 🌈 Runtime API and native SDL3 host

The `foundation-runtime` crate contains a platform-neutral consumer API, an optional **native SDL3 host**, and a browser **Canvas 2D + WebAudio host** for `wasm32-unknown-unknown`. The native host opens an actual window, accepts normalized keyboard input, uploads RGBA textures, executes renderer-neutral draw commands, loads WAV clips and plays audio through a Foundation-owned PCM mixer. The browser host consumes the same draw/audio requests using Canvas 2D and WebAudio, including browser-native audio decoding and alternate packaged encodings; WebGL is intentionally deferred.

Run the native proof application:

```sh
cargo run -p foundation-runtime --example native_sdl3_host --features native-sdl3
```

The demo opens a logical 400×300 pixel-art window, renders a texture and plays a tone when player one presses Z. Escape closes the window.

- Runtime API and service boundaries: [foundation-runtime API](docs/foundation-runtime-api.md)
- Native SDL3 host details: [native SDL3 host](docs/native-sdl3-host.md)
- Example source: `crates/foundation-runtime/examples/native_sdl3_host.rs`
- Platform-neutral tests: `cargo test -p foundation-runtime`
- Full host tests: `cargo test --workspace --features foundation-runtime/native-sdl3`
- WASM core compile check: `cargo check -p foundation-runtime --target wasm32-unknown-unknown`
- WASM Canvas 2D + WebAudio host: `cargo check -p foundation-runtime --target wasm32-unknown-unknown --features web-canvas`
- Browser host contract and manifest: [Canvas 2D + WebAudio guide](docs/web-canvas2d-webaudio.md)

Consumers depend on Foundation types and the optional `native-sdl3` feature. They do not import SDL types, create a second window, or implement a competing device callback. The game continues to own gameplay rules and source-specific asset interpretation.

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

### WebAssembly browser build

Foundation also owns a source-driven Web build. Unlike the native installer step above, this pipeline compiles the consumer's Rust source itself, then generates JavaScript/WASM glue, an HTML shell, the resource package, a versioned manifest and an optional ZIP archive.

Install the browser target and a `wasm-bindgen` CLI version that matches the consumer's `wasm-bindgen` crate:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli
```

Add a `FoundationWebBuild.toml` to the game repository with its Cargo package/crate name, features, source-asset root, resource mappings, HTML title/canvas, bootstrap module and output paths. Then invoke Foundation:

```sh
cargo run -p foundation-packaging --bin foundation-web-build -- \
  --config FoundationWebBuild.toml
```

Foundation runs `cargo build --target wasm32-unknown-unknown`, runs `wasm-bindgen`, stages the source assets using `WebExportBuilder`, generates `index.html`, and writes both the deployable directory and the requested ZIP. Consumers do not pass a precompiled WASM file to Foundation and do not own the packaging command.

See [Foundation WebAssembly export and browser runtime](docs/web-canvas2d-webaudio.md) for the configuration schema and example.

## 🌈 Documentation

- [Rust Gaming Eclipse: shared architecture and integration contract](docs/rust-gaming-eclipse.md)
- [Foundation runtime API](docs/foundation-runtime-api.md)
- [Native SDL3 host](docs/native-sdl3-host.md)
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
- [Bevy integration strategy](docs/bevy-integration-strategy.md)

## 🌈 Research status

The Foundation now has one executable native desktop backend. Other operating systems, console platforms, browser hosts, gamepad input and legacy targets remain separate validation tracks. An implemented SDL3 desktop host does not imply that those other targets are ready.
