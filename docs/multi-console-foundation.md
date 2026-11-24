# 🌈 Multi-console foundation: architecture and scope

## 🌈 1. Purpose

RustMultiConsoleFoundation is a reusable research and architecture foundation for building games in Rust and delivering them across modern operating systems, browsers, legacy desktop Windows releases, and classic consoles.

The project is not a promise that every target can execute the same Rust binary, use the same runtime, or expose the same graphics API. The goal is to preserve as much game logic and content as is practical while deliberately adapting execution, rendering, audio, input, storage, and assets to each target.

The first expected consumers are:

- **StreetChavesRust**, whose target-specific needs include sprite-based renderers and a pre-existing Sega Master System ROM project that can inform a dedicated target pipeline.
- **PointBlankRust**, whose 3D models, geometry, materials, and first-person gameplay will need different renderer and asset budgets across desktop, web, and consoles.
- Future Rust games that can reuse the platform contracts, capability profiles, build tooling, and asset conversion workflows without becoming coupled to either game.

This repository is currently a research and architecture foundation. A toolchain, target, wrapper, or project listed here is not automatically implemented or validated by this repository.

## 🌈 2. Three kinds of output

The foundation must distinguish three materially different delivery strategies.

### 🌈 2.1 Native Rust runtime

Rust code is compiled for a target and linked to that target's runtime, SDK, ABI, and system libraries.

Examples include desktop operating systems and console homebrew ecosystems such as the GBA, 3DS, PSP, N64, and experimental PS1/PS2/PS3 or Wii/GameCube work.

A CPU code generator is only one requirement. A working target also needs startup code, a linker configuration, memory layout, system calls or SDK bindings, asset loading, input, graphics, audio, packaging, and a way to test the result.

### 🌈 2.2 Rust as a host-side ROM or executable generator

Rust runs on the developer's modern workstation and emits target machine code, assembly, binary data, or a ROM image for a machine that does not run a conventional Rust runtime.

This is relevant to very constrained 8-bit systems. A Rust-based assembler, compiler, asset packer, or ROM builder can be useful even when the game itself contains no Rust runtime.

The Atari 2600 is the clearest example: a host-side Rust program can help produce a 6502/6507 ROM, but the console's tiny RAM and scanline-sensitive TIA video kernel require extremely careful low-level code. A successful host-side Rust tool is not the same claim as a conventional Rust application running on the console.

### 🌈 2.3 Hybrid runtime

Rust can own shared logic while target-specific C, assembly, SDK code, or generated binary components handle hardware-facing operations.

This is a valid architecture, not a compromise to hide. It is likely to be necessary for some systems and may be the fastest route to a working game on others.

## 🌈 3. Separate CPU support from console support

A target is not complete merely because LLVM can emit instructions for its CPU family.

Every target investigation must answer these questions independently:

1. **Compiler backend:** Can the selected Rust/LLVM toolchain generate the required instruction set and CPU mode?
2. **ABI and linker:** Can it follow the actual calling convention, object format, relocation model, endianness, alignment, and memory map?
3. **Startup and runtime:** Is there startup code, panic handling, allocation policy, interrupt handling, and a supported entry point?
4. **SDK and hardware APIs:** Can the game access graphics, audio, controls, storage, timers, networking, and other required services?
5. **Packaging:** Can the build produce a valid executable, module, ROM, disc image, or installable package?
6. **Validation:** Can the artifact be launched and tested in an emulator and, where possible, on physical hardware?

The words “Rust target exists”, “binary links”, “homebrew boots”, “game runs”, and “game is feature-complete” must never be used interchangeably.

## 🌈 4. Proposed architecture layers

The following boundaries are design proposals. They should be implemented incrementally and only when real consumers need them.

| Layer | Responsibility |
|---|---|
| Game core | Rules, entities, simulation, deterministic state transitions, gameplay AI, collision logic, and game-specific state |
| Shared platform contracts | Narrow interfaces for time, input, audio output, display submission, storage, and diagnostics |
| Platform capability profile | Machine-readable limits and capabilities used by the build and asset pipeline |
| Platform adapter | Connects shared contracts to native OS APIs, console SDKs, or bare-metal hardware |
| Renderer backend | Converts game render data into the target's actual rendering model |
| Asset pipeline | Converts canonical source assets into target-specific sprites, tiles, models, textures, palettes, animations, and audio |
| Build and packaging tools | Pins toolchains, invokes compilers/linkers, converts assets, packages outputs, and records provenance |
| Validation layer | Performs boot tests, functional tests, asset checks, binary comparisons, and emulator/hardware smoke tests |

The game core should not directly include console-specific SDK headers or call a console's hardware registers. Such code belongs in a platform adapter or a clearly isolated target module.

## 🌈 5. Runtime profiles: core, allocation, and standard library

The foundation must not assume that every target supports Rust's full standard library.

Treat the following as distinct runtime profiles:

- **core-only:** no heap allocation; suitable for highly constrained or bare-metal targets.
- **core + alloc:** allocation is available through a target-provided allocator, but filesystem, threads, process APIs, and other standard-library services are not assumed.
- **std:** the target has a usable implementation of the Rust standard library, with any target-specific gaps documented.
- **host tools:** build-time utilities run on supported developer operating systems and can use the full standard library independently of the final game's runtime.

A game should depend on the narrowest profile it can use. For example, gameplay rules may fit in core-only code while a desktop launcher, editor, asset converter, or diagnostics tool uses std.

The presence of alloc does not imply that a target has a general-purpose operating system or a filesystem.

## 🌈 6. Platform contracts and renderer boundaries

Shared contracts should be deliberately small. Candidate interfaces include:

- Clock and fixed-step simulation timing.
- Input snapshots and edge-triggered button events.
- Audio command submission and voice/channel management.
- Display dimensions, frame submission, and presentation synchronization.
- Persistent storage with explicit capacity and failure behavior.
- Diagnostics and panic reporting.

Do not force one universal renderer on all targets.

For example:

- Desktop and web may use a modern 2D/3D renderer.
- GBA and Master System use sprite/tile-oriented pipelines with palette and memory constraints.
- PS1 requires a renderer designed around the console's actual GPU/GTE capabilities and texture/geometry budgets.
- PS2, PSP, 3DS, GameCube, Wii, N64, and PS3 can each have different 3D paths.
- Atari 2600 and other 8-bit targets may need generated kernels, carefully timed assembly, or entirely target-specific presentation logic.

A common render description can express intent, but each backend must be allowed to reject unsupported features, approximate them explicitly, or request target-specific assets.

## 🌈 7. Capability profiles instead of console-name conditionals

The build and asset tools should consume a capability profile rather than spreading console-specific checks throughout gameplay code.

A future profile may describe:

- CPU architecture, instruction set, endianness, and ABI.
- Available runtime profile: core-only, core + alloc, or std.
- Main RAM, video RAM, ROM budget, and reserved memory.
- Display width, height, refresh mode, framebuffer format, and palette limits.
- Sprite dimensions, tile sizes, sprite count, scanline limits, and animation constraints.
- 3D support, polygon/vertex budgets, texture dimensions, texture formats, and transparency modes.
- Audio channels, synthesis capabilities, sample rates, bit depths, and supported music formats.
- Storage types, capacity, read/write behavior, and save-data constraints.
- Input devices and available buttons.
- Executable and packaging format.
- Unsupported or degraded features.

Profiles must include the source of each measured limit and distinguish hard hardware limits from conservative project budgets. Values should not be invented merely to fill a schema.

## 🌈 8. SDL's place in the architecture

SDL is a useful desktop portability layer for video, audio, keyboard, mouse, and controller input. It is not a universal console SDK.

Use SDL as an optional desktop adapter where its version supports the intended operating system. Keep console backends connected to their own SDKs or hardware interfaces.

Legacy Windows requires version-specific SDL choices:

- Windows 98: SDL 1.2 is the relevant official legacy branch.
- Windows XP: SDL 2 and the current SDL 3 Windows documentation list XP as supported.
- SDL 1.2 is deprecated and should be isolated as a compatibility backend rather than chosen as the default for new desktop builds.

See [Legacy Windows: Rust and SDL compatibility](legacy-windows-sdl-rust.md).

## 🌈 9. Relationship with Impossible Engine

The existing [Impossible Engine compiler architecture](impossible-engine-architecture.md) describes a declarative event-sheet and compiler direction.

The two projects should connect through a boundary, not become one large coupled codebase:

- Impossible Engine owns event-sheet parsing, semantic analysis, intermediate representation, scheduling semantics, and compiler lowering.
- RustMultiConsoleFoundation owns reusable platform contracts, target capability profiles, asset conversion, target toolchain integration, and packaging/validation helpers.
- A future Impossible backend may consume the Foundation's target metadata and build interfaces.
- The Foundation must remain usable by games that do not use Impossible Engine.

The [byte-perfect ROM reconstruction research](byte-perfect-rom-reconstruction.md) is a separate experimental discipline. Reconstructing an existing binary and compiling a new game for a target are related but different goals.

## 🌈 10. Current project boundaries

The following are not claimed to exist yet:

- A stable public platform API.
- A complete cross-console Cargo workspace.
- A universal renderer or audio engine.
- A verified Windows 98 or Windows XP Rust runtime.
- A Foundation-owned SDK for every listed console.
- A demonstrated build of StreetChavesRust or PointBlankRust for every target in the matrix.

The target matrix and roadmap are research planning tools. A target becomes supported only after the project has a reproducible build, a documented runtime and asset path, and a passing target-specific validation gate.
