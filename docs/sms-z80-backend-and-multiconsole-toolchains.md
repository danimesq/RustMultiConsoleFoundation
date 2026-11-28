# SMS/Z80 backend and multi-console toolchain architecture

Status: architectural proposal, 28 November 2026.

## Purpose

RustMultiConsoleFoundation is the platform/toolchain layer shared by game
projects and authoring tools. It should provide reusable contracts and
orchestration for multiple console and modern targets without forcing them
into one runtime or one compiler.

The Street Chaves SMS project is the first concrete Master System validation
consumer. Its incremental Rust migration is independent and remains usable
without this Foundation. The Foundation may absorb stable, reusable
components after they have been proven in that repository.

The target vision is a common project/game representation with specialized
backends for Sega Master System, Mega Drive and modern platforms. A single
backend must never pretend that different consoles have equivalent CPUs,
memory maps, graphics, audio or cartridge formats.

## Responsibility boundary

Foundation owns:

- platform-neutral runtime and SDK contracts;
- versioned target profiles and capability descriptions;
- target-specific asset conversion and validation;
- compiler/assembler/linker discovery and orchestration;
- ROM/image packaging and deterministic build manifests;
- host-side emulation/verification adapters and conformance tests;
- structured diagnostics, budgets, provenance and reproducible builds.

ImpossibleEngine owns authoring, scenes, Event Sheets, add-on semantics and
versioned project/semantic IR. It asks Foundation to validate and build a
selected target; it does not implement console hardware.

Game repositories own their gameplay, project-specific assets and compatibility
evidence. They should consume Foundation contracts rather than directly
depending on SDL, a window system, an emulator CPU crate or a particular
assembler in every project module.

## Backend model

A backend is a target-specific implementation of Foundation contracts. It
should expose:

- target identity and profile version;
- supported execution model and ABI;
- capability report and known hardware budgets;
- asset conversion/packing stages;
- code generation and link/placement strategy;
- runtime services available to the game;
- artifact format and packaging;
- validation commands and evidence.

Backends can share infrastructure but not false assumptions. For example,
Master System and Mega Drive may share asset-manifest and build orchestration
code, but require separate CPU/codegen, memory, graphics, audio, input and
cartridge handling.

## Proposed target layers

```text
Game source / ImpossibleEngine project
                 |
                 v
        Project + semantic IR
                 |
                 v
       Foundation target resolver
                 |
        capability analysis
                 |
          target backend
          /     |      \
         v      v       v
      assets  codegen  runtime adapters
         \      |       /
          \     v      /
          linker + packager
                 |
                 v
         target artifact + manifest
                 |
                 v
       emulator / hardware validation
```

The IR is a contract between authoring and backend layers. It is not the
backend's runtime object model. Backends may lower it into target-specific
representations.

## Master System profile

The SMS backend must be based on verified hardware facts and explicit project
budgets. The initial profile should cover:

- CPU: Z80 execution, supported ABI and interrupt policy;
- cartridge: Sega mapper, ROM bank layout, header and output constraints;
- memory: address map, RAM reservations, stack and bank windows;
- video: VDP modes, VRAM, CRAM, name table, tile and sprite rules;
- audio: PSG channels, noise modes, register/event timing;
- input: controller ports and sampling;
- runtime: startup, frame loop, interrupt entry/exit and deterministic timing;
- assets: 8x8 4bpp planar tiles, palette conversion, tilemaps, sprite layout and
  ROM-bank packing;
- output: reproducible SMS ROM plus machine-readable build report.

Do not encode unverified numeric limits as facts. Each profile field should
carry provenance such as hardware manual, SDK documentation, emulator
measurement or project-specific conservative budget. Unknown values remain
unknown.

## Rust-to-Z80 code generation

A central research question is how a Rust-authored routine becomes executable
Z80 code.

The current candidate, `rustz80`, is a restricted Rust frontend/code generator
that emits Z80 instructions. It is not a complete Rust target, LLVM backend or
full `core` implementation. Its current Spectrum-oriented integration does
not provide the SMS runtime, mapper, linker layout or hardware services.

The Foundation should isolate the compiler behind a replaceable codegen
adapter. The SMS backend must not expose compiler-specific types throughout
the engine or game core.

The adapter contract should eventually describe:

- supported Rust syntax/types and integer widths;
- calling convention, register use, stack behavior and symbol naming;
- code/data sections and relocation format;
- memory model and bank-placement constraints;
- unsupported constructs and actionable diagnostics;
- compiler version, options and deterministic output metadata.

A custom Rust target specification alone does not create a Z80 backend. Do not
claim that a normal `cargo build --target z80-unknown-none` path exists unless
a real compiler backend and linker are supplied.

A practical first integration may combine generated Z80 routines with small
Assembly shims for reset/startup, interrupt boundaries, mapper access, VDP and
PSG I/O, and cartridge packaging. This is acceptable as long as the amount of
Rust-originated game logic is measured honestly.

## Toolchain components

Candidate tools must be evaluated and version-pinned before becoming
dependencies:

| Component | Candidate/reference | Role and limitation |
|---|---|---|
| Rust-to-Z80 codegen | rustz80 | Restricted Rust subset to Z80; not a full Rust toolchain |
| Z80 CPU emulation | Rust `z80` crate | CPU core only; not a complete SMS emulator |
| SMS hardware reference | devkitSMS / SMSlib | C/SDCC and Assembly reference, not a Rust SDK |
| Assembly library | smslib | Z80/SMS routines; ABI/link compatibility must be verified |
| Assembler/linker | WLA-DX | Z80 assembly, bank maps and ROM construction; not a Rust compiler |
| SMS verification | existing project Python harness, then Rust harness | Reference behavior; CPU, mapper, VDP, input and PSG must be tested separately |

No candidate should be promoted from research to a required dependency without
checking maintenance, license, reproducibility, API, target support and a
minimal end-to-end test.

## Rust SMS runtime profile

The code executing on SMS must use an explicitly constrained environment.
Do not assume an operating system, heap, threads, filesystem, floating point,
unwinding or full standard library.

Prefer:
- fixed-width integer types and explicit overflow semantics;
- fixed-capacity arrays/structures;
- deterministic state transitions and update order;
- explicit memory regions and stack budgets;
- small hardware-facing APIs with typed errors where feasible;
- compile-time rejection of unsupported features.

Keep host-side build tools and emulator code separate from the on-cartridge
runtime. Host tools may use richer Rust facilities; SMS code must not inherit
those assumptions.

## Asset pipeline

Canonical source assets remain lossless and target-independent where possible.
The Foundation pipeline resolves a logical asset ID to a target variant and
then converts/validates it for the selected profile.

### Graphics stages

1. Import source image/animation and preserve provenance.
2. Normalize frame dimensions, pivots, timing and logical identity.
3. Select or generate an SMS variant.
4. Quantize to supported palette precision and validate palette assignment.
5. Convert to 8x8 4bpp planar tile data.
6. Deduplicate tiles and generate tile maps/frame layouts.
7. Validate background/sprite composition and hardware sprite constraints.
8. Place resources into VRAM/ROM-bank-aware packages.
9. Emit reports: tile count, palette use, frame size, VRAM requirements,
   bank placement, warnings and output hashes.

The modern target can use larger textures and richer animation. SMS conversion
may need palette redesign, tile deduplication, frame composition changes and
animation simplification. Resizing alone is insufficient.

### Audio stages

A future audio IR should represent musical/effect intent separately from
device-specific samples or PSG registers. The modern backend can produce
digital audio; SMS lowering can produce PSG-compatible patterns/events.

If a sound cannot be represented by the PSG, require an explicit replacement,
approximation or unsupported diagnostic. Do not silently discard channels or
claim sample parity. Validate PSG emulation and actual output separately.

## Multi-console expansion: Mega Drive and modern platforms

The SMS backend is a first proof of the architecture, not the template for all
console hardware.

Mega Drive needs its own profile and backend for its 68000 main CPU, Z80 sound
processor, VDP, memory/bus behavior, audio devices, controllers, ROM layout and
toolchain. Reuse orchestration, manifests, capability diagnostics and asset
pipeline interfaces, not SMS register-level implementations.

Modern native and WebAssembly backends may use richer runtime services and
host APIs. Existing Foundation work for SDL, WASM and other platforms should
remain independent adapters under stable contracts.

## Validation strategy

Validation must be layered and evidence-based:

1. **Toolchain smoke test:** compile/assemble/link a tiny cartridge.
2. **ROM structure:** validate header, size, bank map and deterministic hash.
3. **Boot test:** execute in an SMS emulator and reach a known checkpoint.
4. **Hardware model:** compare CPU, mapper, VDP, CRAM/VRAM, input and interrupts.
5. **Gameplay parity:** replay scripted inputs and compare selected state/frame
   checkpoints with the original ROM.
6. **Asset validation:** inspect palettes, tiles, animation layouts and budgets.
7. **Audio validation:** verify PSG register/event traces and audible output.
8. **Hardware validation:** test on physical hardware where available.

Keep emulator and hardware claims separate. A passing host unit test is not a
boot test; a boot test is not gameplay parity; visual output is not audio
validation.

## Proposed crate/service boundaries

Names below are architectural placeholders, not existing implemented crates:

- `target-profile`: versioned target metadata and capability reports.
- `asset-pipeline`: canonical asset metadata, converters and validators.
- `sms-sdk`: SMS-facing runtime/API surface for game code.
- `sms-backend`: lowering and orchestration for SMS targets.
- `z80-codegen-adapter`: replaceable interface to Rust-to-Z80 code generators.
- `sms-toolchain`: assembler, linker, startup and ROM packager orchestration.
- `sms-emulator-adapter`: host-side verification integration.
- `rom-builder`: deterministic bank layout, header and artifact generation.

Do not create crates merely to match this list. Start with the smallest
cohesive components that have independent tests and stable ownership.

## Migration and extraction sequence

1. Use the SMS repository to finish deterministic Rust tooling parity.
2. Build a Rust SMS verification harness and compare it with the Python model.
3. Prove Rust-to-Z80 in a standalone cartridge with a tiny routine.
4. Document ABI, memory, interrupt and bank-placement contracts.
5. Patch one isolated routine in a working ROM and prove behavioral parity.
6. Stabilize reusable profile, codegen and packaging interfaces.
7. Extract those proven components into Foundation crates/services.
8. Add target-specific asset conversion and PSG pipeline.
9. Integrate ImpossibleEngine's project/IR build request.
10. Research Mega Drive as a separate backend with its own hardware evidence.

The Foundation must not block the SMS project's early progress. Early prototypes
may use project-local code until a reusable API is demonstrated.

## Related documents

- [ImpossibleEngine retro-console targets and shared IR](https://github.com/danimesq/ImpossibleEngine/blob/main/docs/RETRO-CONSOLE-TARGETS-AND-SHARED-IR.md)
- [StreetChavesRust dual-target architecture](https://github.com/danimesq/StreetChavesRust/blob/main/docs/dual-target-sms-architecture.md)
- [Street Chaves SMS Rust migration plan](https://github.com/danimesq/street-chaves-hack-sms/blob/main/docs/RUST-PORT-PLAN.md)
- [Foundation asset and capability pipeline](asset-and-capability-pipeline.md)
- [Foundation multi-backend platform abstraction](multi-backend-platform-abstraction.md)
- [Foundation target platform matrix](target-platform-matrix.md)

## Non-goals

- One universal runtime for all consoles.
- Claiming that the current compiler candidate supports all Rust.
- Treating the SMS CPU crate as a full SMS emulator.
- Making the SMS project depend on Foundation before its interfaces stabilize.
- Using the editor as a substitute for hardware emulation or ROM validation.
