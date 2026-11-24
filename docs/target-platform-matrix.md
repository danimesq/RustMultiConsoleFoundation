# 🌈 Target platform and toolchain matrix

## 🌈 1. How to read this matrix

This matrix records the research direction discussed for RustMultiConsoleFoundation as of 2026-11-24. It is not a support promise.

**Evidence level**

- **Established ecosystem:** a maintained or documented Rust target/library/toolchain exists for the relevant platform. Foundation integration still needs testing.
- **Promising experimental SDK:** a concrete SDK or runtime exists, but coverage, hardware integration, stability, or game-level validation is incomplete.
- **Custom compiler / ROM pipeline:** Rust can participate through a special backend, fork, assembler, or host-side generator rather than a normal supported Rust target.
- **Research only:** an architecture or SDK path appears plausible, but this review did not establish a complete Rust game toolchain.

A project is not Foundation-supported merely because a related repository exists. Validate its current branch, pinned toolchain, license, generated artifact, and real hardware or emulator behavior before integration.

## 🌈 2. Desktop and web

| Target | Planned route | Evidence / caveat |
|---|---|---|
| Linux | Native Rust build | Primary native development and validation environment. |
| Windows 10+ | Native Rust build | Current Rust Windows targets have modern OS baselines; SDL can be an optional adapter. |
| RedoxOS | Native Rust target where available | Verify the exact target tier, ABI, libc/runtime assumptions, and required APIs before claiming support. |
| WebAssembly | wasm32 browser runtime | Treat browser APIs, graphics backend, storage, audio, and threading separately from native builds. |
| Emscripten | wasm32-emscripten toolchain | Distinct from the browser's unknown target; validate Emscripten SDK version, system libraries, packaging, and browser support. |
| Windows XP | Legacy x86 desktop profile | SDL 2/3 documents XP support, but current Rust target baselines do not promise XP. Requires a separate pinned-toolchain experiment. |
| Windows 98 / 98 SE | Legacy x86 desktop profile | SDL 1.2 supports Win9x; modern Rust std is not a supported Win98 runtime. Requires a constrained no_std/FFI or hybrid experiment. |

See [Legacy Windows: Rust and SDL compatibility](legacy-windows-sdl-rust.md) for the compatibility split.

## 🌈 3. Nintendo platforms

| Console | Candidate route | Evidence level | Notes |
|---|---|---|---|
| Sega Master System | Dedicated ROM/asset pipeline informed by the existing Street Chaves SMS project | Custom pipeline | The existing [Street Chaves SMS hack](https://github.com/danimesq/street-chaves-hack-sms) is a ROM-hack/reference project, not a Rust backend. Study its tile, palette, animation, memory, and build constraints. Do not assume that ROM contents or third-party assets can be copied into another repository without rights review. |
| Game Boy / Game Boy Color | Rust-GB / custom SM83 compiler path | Custom compiler / early | [rust-gb](https://github.com/zlfn/rust-gb) describes a custom Rust/LLVM backend for SM83 and explicitly characterizes the project as early-stage. The SM83 is related to, but not interchangeable with, a generic Z80 target. |
| Game Boy Advance | agb | Established ecosystem | [agb](https://github.com/agbrs/agb) provides a Rust game library, sprite/background/audio importing, fixed-point helpers, and core/alloc-oriented support. It is the strongest initial Rust-native retro-console candidate in this review. |
| Nintendo DS | nds-rs / libnds bindings | Promising experimental SDK | [nds-rs](https://github.com/BlueTheDuck/nds-rs) provides Rust abstractions and bindings around libnds. SDL's old DS support is not a substitute for a maintained native SDK. |
| Nintendo 3DS | rustc armv6k-nintendo-3ds + ctru-rs + cargo-3ds | Established ecosystem, Tier 3 compiler target | The Rust target is Tier 3, cross-compiled, emits ELF, has partially working std, and depends on devkitARM/libctru support for OS APIs. Dynamic linking and standard test support are limited. See the [official target documentation](https://doc.rust-lang.org/rustc/platform-support/armv6k-nintendo-3ds.html) and [Rust3DS](https://github.com/rust3ds). |
| Nintendo 64 | libdragon-rs / N64 Rust toolchain projects | Promising experimental SDK | [libdragon-rs](https://github.com/sarchar/libdragon-rs) provides safe and unsafe bindings, build tools, examples, and N64 ROM output. [n64.dev](https://n64.dev/) tracks a broader Rust toolchain ecosystem. Validate the selected libdragon revision and artifact on emulator and hardware. |
| GameCube | libogc bindings or direct Rust devkit work | Promising experimental | PowerPC similarity with Wii is helpful, but GameCube support must be tested independently. Do not infer complete compatibility from shared processor family alone. |
| Wii | ogc-rs / libogc; investigate pure-Rust alternatives | Promising experimental SDK | [ogc-rs](https://github.com/rust-wii/ogc-rs) wraps devkitPro's libogc, with low-level bindings and a safer wrapper. [devkit-dol](https://github.com/commie-caden/devkit-dol) is another experimental pure-Rust direction for GameCube/Wii. Check the maturity of each before choosing a dependency. |

## 🌈 4. Sony platforms

| Console | Candidate route | Evidence level | Notes |
|---|---|---|---|
| PlayStation 1 | PSoXide | Promising experimental SDK | [PSoXide](https://github.com/EBonura/PSoXide) describes a bare-metal Rust SDK, runtime, tools, asset pipeline, PS-X EXE output, and BIN/CUE disc-image building. This is stronger evidence than assuming that generic MIPS LLVM support alone is enough. Validate GPU/GTE, DMA, audio, CD-ROM, controller, memory-card, and timing behavior before a game port. |
| PlayStation 2 | Prussia | Promising experimental SDK | [Prussia](https://github.com/Ravenslofty/prussia) is a Rust PS2 SDK. Its README describes no_std development and a work-in-progress hardware interface roadmap, including DMA and Graphics Synthesizer work. It is not yet equivalent to a complete game SDK. |
| PlayStation Portable | rust-psp | Established project / experimental SDK integration | The original [rust-psp](https://github.com/overdrivenpotato/rust-psp) is a homebrew SDK. A more recent [modernized fork](https://github.com/AndrewAltimit/rust-psp) documents Edition 2024, a pinned nightly, a cargo-psp build flow, EBOOT.PBP packaging, and experimental std support. Select and pin one upstream; do not mix APIs or toolchain instructions from forks. |
| PlayStation 3 | rust-ps3 | Promising experimental SDK | [rust-ps3](https://github.com/ZephyrCodesStuff/rust-ps3) targets the Cell PPU and documents a Rust runtime/toolchain, allocation, LV2 syscall integration, post-linking, and development directions for SPU/RSX and packaging. Treat all advertised subsystems as experimental until independently built and tested for the intended game. |

## 🌈 5. Sega, Atari, and 8-bit targets

| Console / family | Candidate route | Evidence level | Notes |
|---|---|---|---|
| Sega Mega Drive / Genesis | rust-mega-drive / custom m68k LLVM backend | Custom compiler / experimental | [rust-mega-drive](https://github.com/ricky26/rust-mega-drive) requires a forked LLVM and Rust toolchain for Motorola 68000 code generation. This is not a normal upstream Rust target. |
| Atari 2600 | Host-side Rust ROM tools; investigate Rust MOS / LLVM-MOS | Custom compiler / ROM pipeline | The 6507 is a 6502-family CPU. [Rust MOS](https://www.llvm-mos.org/wiki/Rust) documents a Rust toolchain effort for MOS 6502-family platforms, including Atari. This does not remove the 2600's severe RAM, ROM, and cycle-timing constraints. A scanline kernel may still require hand-tuned assembly. |
| NES | Rust MOS / LLVM-MOS | Custom compiler / experimental | The MOS 6502-family backend is relevant, but the NES also requires a correct PPU/APU, mapper, memory and timing strategy. Validate the exact mapper and cartridge format. |
| Atari 8-bit computers | Rust MOS / 6502-family backend | Custom compiler / experimental | Investigate per-machine OS, memory map, video, audio, and cartridge/disk formats; do not treat all 6502 Atari systems as interchangeable with Atari 2600. |
| Game Boy | Custom SM83 route | Early custom compiler | Use the Game Boy-specific route above; it is not automatically covered by Rust MOS. |
| SNES | Dedicated 65C816 toolchain or assembly-assisted output | Research only | The 65C816 is not simply the same target as a MOS 6502 backend. A dedicated ISA/ABI/runtime and SNES graphics/audio toolchain would be needed. |
| Dreamcast | KallistiOS or other homebrew SDK plus custom Rust integration | Research only | SDL 1.2 had limited Dreamcast homebrew support, but that is not proof of a complete Rust-native Dreamcast runtime. Verify the current KallistiOS toolchain and Rust ABI separately. |

## 🌈 6. Xbox 360

| Console | Candidate route | Evidence level | Notes |
|---|---|---|---|
| Xbox 360 | Xenon PowerPC toolchain and homebrew packaging research | Research only / high effort | The PowerPC CPU family is not, by itself, the blocker. The missing pieces are a reproducible legal toolchain, ABI/runtime, system integration, executable packaging, and APIs. Do not label the platform impossible solely because it lacks a mature Rust SDK. |

## 🌈 7. Practical priority, not a ranking of hardware power

The first development wave should prefer platforms with an existing Rust SDK, a tractable asset path, and accessible emulator/hardware validation:

1. Desktop native and web builds establish the shared gameplay contracts.
2. GBA gives the project a relatively concrete Rust game framework and a constrained 2D target.
3. PSP, 3DS, and N64 offer identifiable Rust ecosystems worth prototyping.
4. PS1, Wii/GameCube, and NDS are promising experiments that need focused SDK and game-level checks.
5. PS2 and PS3 remain experimental until their graphics/audio/runtime paths are proven.
6. SMS, Atari 2600, NES, Game Boy, SNES, and Mega Drive need target-specific ROM/compiler/assembly strategies.
7. Windows 98 and XP are separate legacy desktop profiles, not console targets.

The order can change when an existing project or game-specific asset pipeline makes a different target materially cheaper.

## 🌈 8. Required evidence before changing a target's status

For every target, record:

- Exact SDK/toolchain repository, revision, license, and required host OS.
- Rust channel/version and whether build-std or a compiler fork is required.
- Target triple or custom target JSON, CPU mode, ABI, linker, startup/runtime, and libc assumptions.
- Output artifact format and packaging steps.
- Whether core, alloc, and std are available.
- Graphics, audio, input, storage, timing, threading, and networking status.
- Emulator and physical-hardware test results.
- Sample project build logs and known limitations.

A target should be called **Foundation-supported** only after a reproducible sample boots, reads input, presents a frame, produces audio where supported, and passes a documented smoke test on the intended emulator or hardware.
