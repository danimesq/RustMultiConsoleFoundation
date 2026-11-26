# Foundation WebAssembly export and browser runtime

## Purpose and ownership

RustMultiConsoleFoundation defines the shared **Web/WASM export contract** for games and engines that use the library. A consumer supplies its application identity, the JavaScript/WASM files produced by its own build, and an explicit catalog of resources. Foundation validates that description, serializes the export manifest and loads the same package in its browser runtime.

The ownership split is:

- **ImpossibleEngine:** editable projects, including the native `project.ieproj` format, editor data, scenes, Event Sheets, objects, behaviors, scripts and source assets. Its publishing backend can use the Foundation export API.
- **RustMultiConsoleFoundation:** the common Web/WASM package contract, manifest builder and validation, browser URL resolution, resource fetching/decoding, Canvas 2D and WebAudio host.
- **StreetChavesRust:** game simulation, game-specific resource identity and interpretation, and the Street Chaves export assembled through the Foundation API.

The manifest is generic. It does not identify or emulate another editor's export layout, and it does not guess asset filenames or directory conventions. Every file path is explicit and relative to `web-export.json`.

## The Web export package

A package uses one manifest at its root:

```text
web/
  index.html
  main.js
  web-export.json
  pkg/
    game.js
    game_bg.wasm
  assets/
    images/
    audio/
      effects/
      music/
    data/
```

The JavaScript glue and WASM module are produced by the consuming game's own `wasm-bindgen`/`wasm-pack` build. The Foundation builder records their package-relative paths alongside the application metadata and resource catalog.

Example `web-export.json`:

```json
{
  "schema_version": 1,
  "application": {
    "id": "street-chaves",
    "name": "Street Chaves"
  },
  "wasm": {
    "javascript_glue": "pkg/street_chaves.js",
    "wasm_module": "pkg/street_chaves_bg.wasm"
  },
  "assets": [
    {
      "id": "stage_01",
      "path": "assets/images/stages/stage-01.webp",
      "kind": "image"
    },
    {
      "id": "fighter_chaves",
      "path": "assets/images/fighters/C01.png",
      "kind": "image"
    },
    {
      "id": "fight_hit_01",
      "path": "assets/audio/effects/hit-01.ogg",
      "fallbacks": [
        "assets/audio/effects/hit-01.m4a",
        "assets/audio/effects/hit-01.wav"
      ],
      "volume_percent": 85,
      "kind": "audio"
    },
    {
      "id": "fight_music",
      "path": "assets/audio/music/theme.ogg",
      "fallbacks": [
        "assets/audio/music/theme.m4a",
        "assets/audio/music/theme.mp3"
      ],
      "kind": "audio"
    },
    {
      "id": "fighter_config_chaves",
      "path": "assets/data/C01.ini",
      "kind": "data"
    }
  ]
}
```

The schema is versioned independently from the consumer's game and editor versions. Resource IDs are stable game-facing names; paths describe the actual files shipped in the package. Folder names and filename case are preserved exactly as authored.

## Build a manifest with the Foundation library

The manifest API is available on native and WASM targets through `foundation_runtime::web_export`. Engine exporters and game build tools can use the same types:

```rust
use foundation_runtime::web_export::{
    WebAssetKind, WebExportAsset, WebExportBuilder, WEB_EXPORT_MANIFEST_FILE,
};

let manifest = WebExportBuilder::new("street-chaves", "Street Chaves")
    .wasm_bundle(
        "pkg/street_chaves.js",
        "pkg/street_chaves_bg.wasm",
    )
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
        .with_fallbacks([
            "assets/audio/music/theme.m4a",
            "assets/audio/music/theme.mp3",
        ]),
    )
    .build()?;

let json = manifest.to_json()?;
std::fs::write(output_dir.join(WEB_EXPORT_MANIFEST_FILE), json)?;
```

`WebExportBuilder` validates the application ID, JavaScript/WASM paths, unique asset IDs, asset paths, fallback paths and audio volume range. `WebExportManifest::parse` applies the same validation when reading a published package. All paths must be safe relative paths; absolute paths, traversal segments, URL schemes and backslashes are rejected.

The builder creates the versioned export descriptor. The consuming build/publishing step is responsible for compiling the game crate, placing the generated JavaScript/WASM files and copying each declared asset to its declared path. This first slice does not yet provide a ZIP packager, HTML template generator, asset transcoder or content-addressed optimizer.

## Build the browser host

The optional `web-canvas` feature targets `wasm32-unknown-unknown` and provides:

- `Canvas2dHost` implementing the shared `RuntimeHost` contract.
- Canvas 2D rendering of ordered backend-neutral draw commands.
- Stable Foundation texture and audio IDs.
- Browser keyboard normalization into two-player input snapshots.
- WebAudio decoding, playback, looping, volume and stop operations.
- `load_web_export()`, which reads `web-export.json`, resolves every resource relative to its URL, fetches it and registers the decoded resource.

Build the browser target:

```sh
rustup target add wasm32-unknown-unknown
cargo check -p foundation-runtime --target wasm32-unknown-unknown --features web-canvas
```

A consumer application typically builds its own crate with `wasm-pack build --target web` and enables the Foundation browser feature through its dependency feature. The consumer owns its browser entry page and animation loop; Foundation owns the platform host and shared resource contract.

## Use the browser host

Create the host around an existing canvas element:

```rust
let config = RuntimeConfig {
    title: "Street Chaves".into(),
    width: 400,
    height: 300,
    pixel_art: true,
};
let host = Canvas2dHost::from_canvas_id("game-canvas", config)?;
let assets = host.load_web_export("./web-export.json").await?;
```

The browser application drives the host from its own `requestAnimationFrame` loop and calls the game simulation using the game's chosen timing policy. Display refresh rate must not silently become the simulation clock.

The embedding page can resize the backing store with `resize_backing_store(css_width, css_height, device_pixel_ratio)` after viewport, fullscreen or device-pixel-ratio changes. The logical game dimensions remain unchanged.

The first browser input profile mirrors the Street Chaves two-player keyboard layout. Browser gamepad, pointer/touch, worker and OffscreenCanvas support are separate capabilities and are not implied by this host.

## Resource loading and decoding

The loader resolves `path` and each optional `fallbacks` entry relative to the manifest URL. Paths cannot escape the package directory with `..`. Resource IDs must be unique.

- **Images:** PNG/BMP bytes are decoded into Foundation RGBA textures.
- **Audio:** the current browser host uses `decodeAudioData()` and retains decoded WebAudio buffers. Codec support depends on the browser; declare explicit alternate encodings when needed.
- **Data:** files are returned unchanged as byte arrays for game-specific parsing.

The catalog may organize effects and music in separate directories, but both currently use the same buffered audio registration path. Music streaming is **not** implemented by this host yet. A future streaming API must be a real capability with its own fetch/decode/playback lifecycle, not just a label in the manifest.

All resources are fetched once and retained behind stable IDs. HTTP, CORS, UTF-8, manifest and decode failures are surfaced as errors. Serve the package over HTTP(S) and configure suitable CORS headers when assets are hosted on another origin.

## Host responsibilities and current limits

Foundation provides browser display, input normalization, resource loading/decoding and audio dispatch. The consumer remains responsible for:

- Game simulation, deterministic/fixed-step timing and state transitions.
- Mapping game-specific IDs and metadata to Foundation resource IDs.
- Starting/resuming WebAudio from a user gesture where the browser requires it.
- Canvas viewport resize, fullscreen and application lifecycle.
- Generating the consumer's WASM module and JavaScript glue.

Current implementation limits:

- Canvas 2D is the browser renderer; WebGL is deferred.
- WebAudio decodes complete files into buffers; streaming music is not available.
- Browser gamepad and pointer/touch mappings are not implemented.
- There is no ZIP publisher, HTML shell generator, media transcoder or automatic asset optimizer in this slice.
- Browser output is not claimed to be bit-identical to the native S16 PCM mixer.
