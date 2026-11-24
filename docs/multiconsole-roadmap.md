# 🌈 Multi-console implementation roadmap

## 🌈 1. Guiding rule

Advance by validated vertical slices, not by adding dozens of target names to Cargo configuration.

A target is ready to move forward only when its compiler, linker, runtime/SDK, packaging, assets, and test path have all been demonstrated.

## 🌈 2. Phase 0 — Repository and evidence baseline

- Keep the existing ROM reconstruction and Impossible Engine documents as separate research tracks.
- Pin the revisions of external SDKs and toolchains before experiments.
- Record target licenses and redistribution requirements.
- Maintain a target matrix with evidence level and last verification date.
- Establish a shared vocabulary for compile support, runtime support, SDK support, packaging, emulator testing, and real-hardware testing.

## 🌈 3. Phase 1 — Foundation contracts

Design and prototype only the smallest shared contracts needed by the first two games:

- Fixed-step clock and update ordering.
- Input snapshots and button-edge events.
- Audio command submission.
- Renderer-facing data independent of any one console API.
- Storage interface with explicit capabilities.
- Diagnostics and build/asset provenance.

Keep target-specific APIs behind adapters. Avoid creating empty crates for every console before a real sample requires them.

## 🌈 4. Phase 2 — Desktop and browser baseline

Build and validate:

- Linux and current Windows native profiles.
- WebAssembly browser output.
- Emscripten output as a separately configured WebAssembly path.
- Optional SDL-based desktop adapter.

This phase establishes the control environment for behavior tests and asset previews.

## 🌈 5. Phase 3 — Legacy Windows profiles

Develop Windows XP and Windows 98 independently.

- XP: start with an SDL 2/3 native smoke test, then test a pinned Rust toolchain and FFI/runtime configuration.
- Win98: first prove SDL 1.2.15 in a C test, then investigate Rust static-library/no_std integration.
- Keep legacy dependencies and profiles isolated from current desktop builds.
- Do not call either OS supported until the acceptance tests in the legacy Windows document pass.

## 🌈 6. Phase 4 — First console vertical slices

Recommended initial experiments:

1. **GBA:** use agb to build a small sample with input, sprites, sound, and ROM packaging.
2. **PSP:** pin one rust-psp SDK fork and build a minimal EBOOT.PBP with video, input, and audio.
3. **3DS:** build through the official Rust 3DS target ecosystem, ctru-rs, and cargo-3ds.
4. **N64:** use libdragon-rs for a minimal ROM and validate the selected toolchain.
5. **PS1:** use PSoXide to build a minimal PS-X EXE and disc image, then test rendering and timing.

The order can change based on project needs and SDK health. The goal is one thoroughly tested slice, not simultaneous half-finished ports.

## 🌈 7. Phase 5 — More console adapters

After the first vertical slices, investigate:

- NDS using a maintained Rust/libnds project.
- Wii and GameCube using ogc-rs or a direct Rust devkit.
- PS2 using Prussia after graphics, DMA, and controller/audio support are sufficient.
- PS3 using rust-ps3 after its selected PPU/RSX/SPU/runtime path is tested.
- SMS, NES, Game Boy, Atari 2600, and Mega Drive through target-specific compilers, assembly, ROM generators, and asset pipelines.
- SNES and Xbox 360 only after dedicated feasibility spikes establish the required backend and runtime work.
- Dreamcast through a separately validated homebrew SDK and Rust integration path.

## 🌈 8. Phase 6 — Asset profiles and game adaptation

For each accepted target:

- Create a versioned capability profile from verified hardware and SDK documentation.
- Add asset converters only for real formats needed by the game.
- Produce a target-specific asset pack for one representative scene or level.
- Measure memory, ROM, video memory, CPU time, frame time, and audio resource usage.
- Keep alternate art/model/audio variants explicit and traceable.
- Record any gameplay or visual feature that must be disabled or redesigned.

StreetChavesRust should exercise sprite/tile, animation, palette, and audio conversion. PointBlankRust should exercise model simplification, texture conversion, geometry budgets, and 3D renderer specialization.

## 🌈 9. Definition of done for a target

A target may be marked **implemented** only when all applicable items pass:

- Reproducible build from documented and pinned toolchains.
- Valid executable/ROM/package with a recorded hash.
- Successful boot in a named emulator or on named hardware.
- Input works.
- A representative scene renders correctly.
- Audio works where required and supported.
- Save/storage works where required and supported.
- Memory and performance budgets are measured.
- Known limitations are documented.
- At least one game-specific sample is integrated.

A target may be marked **experimental** when a proof of concept builds or boots but one or more of these gates remain incomplete.

## 🌈 10. Deliverables

Every vertical slice should contribute:

- A minimal sample project.
- A pinned toolchain manifest.
- Build and packaging instructions.
- A target capability profile.
- A known-good asset pack and its hashes.
- Emulator/hardware test notes.
- A troubleshooting section.
- A concise status update to the target matrix.

Do not report a console as supported because a compiler emitted an object file. Report exactly what has been built, launched, exercised, and verified.
