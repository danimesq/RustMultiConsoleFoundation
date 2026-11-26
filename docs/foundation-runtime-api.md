# foundation-runtime: consumer API

The `foundation-runtime` crate provides a platform-neutral API plus an optional native SDL3 host. Game code depends on Foundation types, never on SDL window, texture, event, or audio-device handles.

## Public boundary

- `RuntimeConfig`: window title, logical dimensions and pixel-art scaling preference.
- `Capabilities`: explicit availability for display, keyboard, pointer, gamepad, audio and storage.
- `InputSnapshot` / `PlayerInputSnapshot`: normalized UI and two-player input, including held, pressed and released masks.
- `RgbaImage`, `TextureId`, `Rect`, `DrawCommand` and `FrameOutput`: backend-neutral image and drawing vocabulary.
- `AudioClipId` / `AudioRequest`: semantic WAV playback, loop, stop and diagnostic beep requests.
- `RuntimeHost`: adapter contract for time, input, rendering and audio.
- `Runtime`: optional outer frame coordinator. Deterministic games should keep their own fixed-step simulation clock.

## Native backend

Enable the optional feature and construct the concrete host:

```toml
foundation-runtime = { git = "https://github.com/danimesq/RustMultiConsoleFoundation", features = ["native-sdl3"] }
```

```rust
use foundation_runtime::native_sdl::Sdl3Host;
use foundation_runtime::{RuntimeConfig, RuntimeHost, FrameOutput};

let mut host = Sdl3Host::new(RuntimeConfig {
    title: "My game".into(),
    width: 400,
    height: 300,
    pixel_art: true,
})?;
let input = host.poll_input()?;
host.present(&FrameOutput::default())?;
# Ok::<(), foundation_runtime::RuntimeError>(())
```

Run the native SDL3 demonstration from the Foundation workspace:

```sh
cargo run -p foundation-runtime --example native_sdl3_host --features native-sdl3
```

See [Native SDL3 host](native-sdl3-host.md) for the concrete APIs, keyboard map, audio mixer and validation boundaries.

## Timing and ownership

The Foundation host owns the SDL window/event pump, renderer textures, audio device callback and decoded audio clips. Consumers own game state, fixed-tick decisions, resource naming, sprite selection and semantic audio-event mapping. A consumer should not initialize a second SDL window or device callback beside the Foundation host.

`Runtime::step` is a convenience for frame-driven apps. StreetChavesRust uses a 28 ms deterministic simulation tick, so its native loop should use `FixedTimestepClock` and call the Foundation host's `poll_input`, `audio` and `present` methods around the existing simulation. The Foundation does not take over that game-specific cadence.

## Target boundary

The `native-sdl3` feature is opt-in and the SDL dependency is target-gated away from wasm32. The platform-neutral runtime API has a separate `wasm32-unknown-unknown` compile check. This PR does not implement a browser Canvas/WebGL host or console SDK adapters.
