# 🌈 Asset pipeline and target capability profiles

## 🌈 1. Objective

A shared game should not require one identical sprite sheet, model, texture, or audio file to fit every platform.

RustMultiConsoleFoundation owns the canonical source-asset workflow and target-build orchestration. The initial Web/WASM build implementation compiles consumer Rust source, stages a versioned resource package, generates the HTML shell and ZIP, and preserves source-to-output provenance. The broader target-specific conversion pipeline described below remains an implementation roadmap: palette/tiling conversion, model reduction, audio transcoding, streaming audio and console-specific packers are not implied by the Web build alone.

## 🌈 2. Canonical source assets and target outputs

Use this pipeline:

1. **Authoring source:** high-quality source art, animation, models, music, sound effects, and metadata.
2. **Canonical intermediate representation:** normalized dimensions, coordinate conventions, timing, color information, skeleton/animation descriptions, audio metadata, and stable identifiers.
3. **Target capability profile:** hardware and project budgets selected for the build.
4. **Target conversion:** quantization, tiling, palette reduction, mesh simplification, texture conversion, sample conversion, animation baking, and format-specific packing.
5. **Validation:** size/budget checks, visual/audio previews, deterministic hashes, and target-specific runtime tests.
6. **Packaging:** place the generated assets in the correct ROM, executable, filesystem, or installable package.

The source assets should remain lossless whenever practical. Never overwrite the canonical source with a lossy platform conversion.

## 🌈 3. Capability profile contents

A target profile should eventually use a versioned, machine-readable schema. The following is an illustrative shape, not a finalized schema:

~~~json
{
  "schema_version": 1,
  "target_id": "example-target",
  "runtime": {
    "rust_profile": "core-alloc",
    "execution_model": "native"
  },
  "display": {
    "width": 0,
    "height": 0,
    "pixel_formats": [],
    "palette_entries": null,
    "framebuffer_modes": []
  },
  "graphics": {
    "sprites": true,
    "tiles": true,
    "hardware_3d": false,
    "max_texture_dimensions": null,
    "project_budgets": {}
  },
  "memory": {
    "ram_bytes": null,
    "video_ram_bytes": null,
    "rom_budget_bytes": null,
    "reserved_bytes": null
  },
  "audio": {
    "channels": null,
    "sample_formats": [],
    "sample_rates": [],
    "hardware_synthesis": []
  },
  "input": {
    "buttons": [],
    "pointer": false,
    "touch": false
  },
  "artifact": {
    "format": "target-specific",
    "packaging": []
  }
}
~~~

Unknown values should remain null or be omitted. A profile must never fabricate a hardware limit. Separate documented hardware limits from conservative budgets chosen by a particular game.

Profiles should also record their evidence: hardware manual, SDK documentation, measurement, or project policy.

## 🌈 4. Sprite and tile pipeline

For sprite-heavy games such as StreetChavesRust, keep these concepts distinct:

- Source image and source animation.
- Sprite frames and frame timing.
- Animation state and transitions.
- Collision boxes, hurt boxes, and attack hit boxes.
- Palette and transparency metadata.
- Tile maps, tile atlases, and sprite atlases.
- Target-specific frame and tile packing.

Example conversions:

- Desktop/web can retain larger images, full color, and high-resolution animation.
- GBA needs palette-aware graphics, target-compatible tile/sprite data, and memory-conscious animation.
- Master System needs a palette- and tile-aware pack with console-specific layout and color restrictions.
- Game Boy and NES need their own palette, tile, sprite, and scanline policies.
- Atari 2600 needs a radically different representation; source animation may need to be redesigned as timed kernel data rather than converted as a conventional sprite sheet.

Do not assume that a single sprite atlas can be resized and become valid for every console. Conversion may involve palette quantization, tile deduplication, layout changes, animation simplification, or authored variants.

## 🌈 5. 3D model and renderer pipeline

For PointBlankRust and other 3D games, the source model should be independent of a specific console renderer.

Candidate stages:

- Normalize coordinate system, units, winding, pivots, and skeleton conventions.
- Remove or bake unsupported modifiers and procedural features.
- Simplify geometry and enforce target polygon/vertex budgets.
- Reduce or bake materials and shader effects.
- Convert textures to supported dimensions and formats.
- Bake lighting or vertex colors where programmable shaders are unavailable.
- Reduce animation bones, keyframes, precision, or blend complexity when required.
- Build target-specific collision geometry separately from render meshes.

A modern desktop model should not be shipped unchanged to a constrained console merely because the file format can be parsed.

Renderer strategy is per target. PS1, PS2, PSP, N64, GameCube/Wii, 3DS, and PS3 do not share one equivalent GPU API or one equivalent performance budget. A PS1 renderer may rely on fixed-function GPU/GTE workflows and carefully controlled geometry; a desktop renderer may use programmable shaders. The shared layer should represent scene/game intent and permit specialized renderers to implement it differently.

A first-person shooter is possible on hardware with substantially different graphics capabilities, but the renderer, model complexity, visibility strategy, textures, animation, and frame budget must be designed for each target. “The CPU is 32-bit” is not a meaningful performance guarantee.

## 🌈 6. Audio pipeline

Keep music composition and source recordings separate from target audio data.

A future audio pipeline may convert:

- MIDI or tracker compositions into target sequencer patterns and instrument assignments.
- Sample-based instruments into target bit depths, rates, loop formats, and banks.
- Sound effects into compressed or PCM data supported by the target.
- Multi-channel recordings into stems or simplified arrangements.
- Timing and envelopes into target-native playback commands.

The resulting audio may differ materially between platforms. Some systems have sample playback, some have hardware synthesis channels, and others have very limited or highly specialized sound hardware.

The build should report sample memory, channel usage, playback format, and any conversion loss. Never claim that an audio conversion preserves the original arrangement unless it has been compared and verified.

## 🌈 7. Data-driven characters and objects

Game-specific character definitions can remain data-driven without making the runtime depend on a single asset format.

A character definition may reference:

- Stable character identifier and metadata.
- Animation states and transition rules.
- Sprite/model asset references.
- Attacks, special moves, timing, and hit boxes.
- Attributes and gameplay parameters.
- Voice and sound-effect identifiers.
- Platform-independent gameplay behavior.

The asset compiler can then build a target-specific manifest and packed resources. A character may use a sprite set on one target and a lower-polygon model or alternate animation set on another.

Keep gameplay data separate from target-rendering details wherever possible. Target variants should be explicit and inspectable rather than hidden behind silent runtime substitutions.

## 🌈 8. Deterministic asset builds and provenance

The first source-driven Web publisher is implemented in `foundation-packaging::web_build`. It accepts source-root-relative files and explicit package destinations, validates that asset inputs stay inside the configured source root, records optional upstream URLs, and emits a deployable directory and ZIP. Target conversion tools will be added as Foundation build stages rather than pushed into each game repository.

For repeatable builds:

- Pin converter versions and settings.
- Sort input files and manifest entries deterministically.
- Record source hashes and generated-output hashes.
- Use stable names and identifiers.
- Record lossy transformations and quality settings.
- Make unsupported assets fail with useful diagnostics.
- Keep generated assets out of source directories unless the project explicitly chooses checked-in generated artifacts.
- Preserve author credits, upstream license notices, and provenance.

ROM hacks and decompilation repositories can be valuable technical references, but their ROMs, artwork, music, and extracted assets may have separate copyright and redistribution restrictions. In particular, the Street Chaves Master System project should initially be treated as a source of technical knowledge. Reuse only material whose rights and license permit it.

## 🌈 9. Validation gates

Every target asset pack should pass:

- File-format validation.
- Declared memory and hardware-budget checks.
- Palette/tile/texture dimension checks.
- Animation timing and missing-frame checks.
- Audio format, duration, channel, and memory checks.
- Deterministic rebuild/hash checks.
- Preview rendering or playback in a host tool where feasible.
- Emulator or hardware validation for the final target.

The converter should report which assets were reduced, remapped, baked, dropped, or replaced with target-specific variants. Silent degradation is not acceptable.
