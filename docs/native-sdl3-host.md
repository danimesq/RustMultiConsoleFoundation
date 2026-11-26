# Foundation runtime: executable consumer contract

The `foundation-runtime` crate now provides two things that are exercised by code, not just described in architecture documents:

1. A platform-neutral contract for normalized player input, render commands, resource IDs, capabilities and semantic audio.
2. An optional native SDL3 host behind the `native-sdl3` feature.

## Native host

Build and run the actual desktop window / texture / audio example:

```sh
cargo run -p foundation-runtime --example native_sdl3_host --features native-sdl3
```

The example opens an SDL3 window, uploads an RGBA texture, presents renderer-neutral draw commands, polls physical keyboard input and plays a tone through the host audio mixer when player one presses Z. Escape closes the window.

A consumer does not import SDL types or own an SDL window. It imports `Sdl3Host` and Foundation types only:

```rust
use foundation_runtime::native_sdl::Sdl3Host;
use foundation_runtime::{RuntimeConfig, RuntimeHost, FrameOutput};
```

## Practical API

- `Sdl3Host::new(RuntimeConfig)`: initialize SDL video/input and create the native window. Audio is optional and reported as unavailable if no output device opens.
- `Sdl3Host::load_bmp` / `load_png`: decode local images to Foundation-owned RGBA8 buffers.
- `Sdl3Host::register_texture`: upload an `RgbaImage` and return a backend-neutral `TextureId`.
- `Sdl3Host::load_wav`: decode and convert a WAV to 44.1 kHz signed 16-bit stereo, returning a Foundation-owned `AudioClipId`.
- `RuntimeHost::poll_input`: produce normalized two-player directional and button state, including pressed/released edges and five custom-special slots per player.
- `RuntimeHost::present`: draw ordered `Clear`, `Rect`, and `Texture` commands in the configured logical resolution with letterboxing.
- `RuntimeHost::audio`: play/loop a clip, stop a clip, stop all clips, or play a diagnostic tone.

The PCM mixer lives in `audio.rs`, outside the SDL adapter. It uses 44.1 kHz S16 stereo, 48 concurrent voices, one active voice per clip, oldest-voice stealing, loop support and saturating summation. This gives consumers a shared mix policy while allowing each target to provide its own device callback.

## Native keyboard defaults

| Player | Direction | Four attack slots | Five special slots |
|---|---|---|---|
| 1 | Arrows | Z, X, C, V | F1..F5 |
| 2 | W, A, S, D | R, T, Y, U | Keypad 1..5 |

Player input is independent of the UI-oriented `confirm/cancel` fields. A consumer maps `PlayerInputSnapshot` into its own game-specific `InputFrame`; Foundation never makes fighting-game move decisions.

## Asset ownership boundary

The Foundation decodes generic BMP/PNG/WAV files, stores RGBA data, uploads textures, and plays decoded clips. It does not parse Street Chaves INI/ROT, determine sprite-sheet cuts, know the Pascal white color key, choose fighter animation frames, or interpret `SomPendente` indices. Those are game asset/gameplay policies and remain in StreetChavesRust.

A future game migration should therefore replace its SDL device/window/renderer/audio code with this host, while keeping source-faithful asset extraction and event-to-clip mapping in the game. The Foundation receives only generic draw and audio requests.

## Target boundary and checks

- SDL3 0.22.1 is an optional dependency and is target-gated away from wasm32.
- Native consumers opt in with `features = ["native-sdl3"]`.
- The same `foundation-runtime` platform-neutral API can be built for `wasm32-unknown-unknown` without SDL. A browser host is not implemented by this PR.
- CI runs formatter, the workspace tests with native SDL3 enabled, and a wasm32 compile check for the platform-neutral runtime.
- The SDL3 `build-from-source-unix-console` option allows headless Linux CI runners without X11/Wayland development libraries. Desktop builds can still use available native window backends.

## Current backend limits

The first SDL3 host implements window creation, logical scaling/letterboxing, keyboard input, texture registration, BMP/PNG decode, WAV playback and audio clip control. Mouse and gamepad capabilities are currently reported as unavailable. Text rendering, controller hotplug, persistent storage and device hotplug/recovery remain future capabilities; unsupported features are not silently reported as working.


## Configurable physical-key capture

`InputSnapshot::physical_key_pressed` exposes the most recent non-repeat physical key press as a platform-neutral `PhysicalKey`. `RuntimeHost::set_player_one_key_map` updates the eight player-one bindings (four directions and four attacks) without changing game simulation code. SDL3 and Canvas 2D/WebAssembly use the same `PlayerOneKeyMap`; player two's default layout and the five special slots remain separate.
