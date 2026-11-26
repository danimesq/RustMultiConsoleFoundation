# Browser Canvas 2D and WebAudio host

## Scope

The optional `web-canvas` feature provides a browser host for `wasm32-unknown-unknown`. It uses the same Foundation runtime contract as the native SDL3 host:

- `Canvas2dHost` implements `RuntimeHost`.
- `DrawCommand`, `TextureId`, `AudioRequest`, and `AudioClipId` remain backend-neutral.
- HTML Canvas 2D draws the same ordered commands produced for SDL3.
- WebAudio plays clips registered by stable Foundation IDs.
- Browser key events are normalized into the same two-player input snapshots.
- WebGL is intentionally not part of this implementation.

The browser application owns its `requestAnimationFrame` loop and calls the host at the appropriate points. The game simulation must continue using its own fixed logical tick; display refresh rate must not become the simulation clock.

## Build

Add the target and compile the web feature:

```sh
rustup target add wasm32-unknown-unknown
cargo check -p foundation-runtime --target wasm32-unknown-unknown --features web-canvas
```

The feature is browser-only. SDL3 remains behind `native-sdl3` and is not compiled for `wasm32-unknown-unknown`.

## Canvas host

Create the host around an existing HTML canvas element:

```rust
let config = RuntimeConfig {
    title: "Street Chaves".into(),
    width: 400,
    height: 300,
    pixel_art: true,
};
let host = Canvas2dHost::from_canvas_id("game-canvas", config)?;
```

The host uses a Canvas 2D context. `present()` preserves command order, letterboxes the configured logical resolution, supports source rectangles and horizontal sprite flipping, and applies nearest-neighbor smoothing for pixel-art output.

The embedding page may resize the backing store using `resize_backing_store(css_width, css_height, device_pixel_ratio)`. It should call this after viewport, fullscreen, or device-pixel-ratio changes. The logical game size does not change.

The first browser input profile mirrors the native Street Chaves keyboard layout: player one uses arrows and Z/X/C/V; player two uses WASD and R/T/Y/U. F1-F5 and the numeric keypad provide the custom special-move slots. Escape requests quit and window blur releases held keys. Browser gamepad and pointer support remain separate capabilities and are reported as unavailable by this first adapter.

## Exported-game asset package

The host follows the familiar HTML5 export model used by Construct-style games: game resources are files shipped beside the exported page and described by a manifest. Native filesystem paths are never translated into browser URLs.

A manifest uses schema version 1:

```json
{
  "schema_version": 1,
  "assets": [
    { "id": "stage_01", "path": "assets/stage-01.bmp", "kind": "image" },
    { "id": "fighter_chaves", "path": "assets/chaves.png", "kind": "image" },
    {
      "id": "fight_hit_01",
      "path": "audio/hit-01.ogg",
      "fallbacks": ["audio/hit-01.m4a", "audio/hit-01.wav"],
      "volume_percent": 85,
      "kind": "audio"
    },
    { "id": "fighter_config_chaves", "path": "data/C01.ini", "kind": "data" }
  ]
}
```

Paths and optional `fallbacks` are resolved relative to the manifest URL, must be safe relative paths, and cannot escape the package directory with `..`. Resource IDs must be unique. Images are decoded from PNG/BMP into Foundation RGBA textures; audio files are decoded by the browser's `decodeAudioData()` implementation (for example WAV, OGG, MP3, or AAC/M4A where supported by that browser); data entries are returned unchanged as byte arrays so the consuming game can parse INI or other configuration formats. Alternate audio encodings can be listed in `fallbacks`, matching the multi-format packaging pattern used by HTML5 game exporters. The loader tries each source until one fetches and decodes successfully.

Load the manifest asynchronously:

```rust
let assets = host.load_asset_manifest("assets/street-chaves-manifest.json").await?;
let stage_texture = assets.textures["stage_01"];
let hit_clip = assets.audio_clips["fight_hit_01"];
let fighter_ini = std::str::from_utf8(&assets.data["fighter_config_chaves"])?;
```

All resources are fetched once and retained behind stable IDs. The web host reports HTTP, CORS, UTF-8, manifest, and decode failures instead of silently substituting arbitrary assets. The export/deployment must serve these resources from the same origin or configure suitable CORS headers.

This loader is generic. StreetChavesRust remains responsible for interpreting its Pascal resource-bank rules, sprite pivots, atlas frames, and fighter metadata. User-supplied custom-character packages can continue to use the existing browser package bridge and the same asset ID/resource registration path.

## WebAudio

The browser host decodes complete audio files with the browser's native WebAudio decoder into floating-point AudioBuffers, then applies the requested volume percentage with a per-voice GainNode. Playback uses `AudioBufferSourceNode` instances connected to the host AudioContext. The actual encoded format depends on the browser's supported codecs; exporters should provide fallback encodings for browsers with different codec support.

The adapter preserves the current clip-level behavior:

- A repeated `AudioClipId` stops the previous voice and starts again at the beginning.
- Music requests can loop.
- Stopping one clip leaves unrelated clips playing.
- Stop-all stops every active voice.
- The host caps tracked active voices at 48 and steals the oldest when full.

The browser performs the final device mixing. This is not claimed to be bit-identical to the native S16 PCM mixer: WebAudio handles resampling and device output. The game-facing `AudioRequest` contract and Pascal clip/bank IDs remain shared.

Browsers may suspend a newly created AudioContext until a user gesture. Call `resume_audio()` from the Start button's click/tap handler before the game begins playback. Audio unavailability is reported separately and does not prevent the game from rendering or simulating.

## Host responsibilities

The host deliberately does not own gameplay or the browser animation loop. It provides resource decoding/registration, normalized input, drawing and audio dispatch. The consumer remains responsible for:

- Fixed-timestep game simulation and catch-up policy.
- Resource semantics and mapping Pascal/game IDs to Foundation texture/audio IDs.
- Starting/resuming audio from a user gesture.
- Canvas viewport resize and fullscreen lifecycle.
- Loading and validating game-specific data assets.

## Deferred work

- WebGL 2 renderer and performance comparison.
- AudioWorklet-backed deterministic PCM mixer, if precise sample-mixer parity is required.
- Browser Gamepad API and pointer/touch mapping.
- Worker/OffscreenCanvas host profile.
- Device/context loss recovery and browser lifecycle smoke tests.
