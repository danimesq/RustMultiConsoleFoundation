# Byte-perfect ROM reconstruction: C → Rust

## 1. Objective

Use mature Pokémon decompilation projects as forensic references to investigate whether original C game code can be ported to Rust while preserving the exact ROM image produced by the original build.

The first two case studies are:

1. **Pokémon FireRed / LeafGreen**: `pret/pokefirered`
2. **Pokémon Emerald**: `pret/pokeemerald`

The intended learning sequence is:

`original ROM + matching decompilation → reproducible C build → C-to-Rust port → semantic equivalence → code-generation investigation → binary comparison`

This is an experiment, not an assumption that a normal Rust build will automatically match a ROM produced by the original C compiler.

## 2. What “byte-perfect” means

A build is byte-perfect only when the produced ROM has exactly the same bytes as the selected reference ROM. A matching SHA-1 is useful as a quick identity check; a byte-for-byte comparison is the definitive local test.

Keep these claims separate:

- **Source-level port**: C logic has been translated to Rust.
- **Semantic equivalence**: the Rust port has the same defined observable behavior for the tested inputs and machine state.
- **Object-level matching**: generated object code and relevant sections match.
- **ROM-level matching**: the final linked and packaged ROM is byte-identical to the reference.

Matching gameplay, matching source intent, matching symbols, and matching ROM bytes are not interchangeable.

## 3. Why C → Rust is difficult

A faithful port must investigate details that ordinary application ports can safely ignore.

### 3.1 C language semantics

Record and test:

- Integer widths, signedness, integer promotions, and usual arithmetic conversions.
- Signed overflow assumptions and unsigned wraparound.
- Shift behavior, division, remainder, and narrowing conversions.
- Pointer arithmetic, pointer provenance assumptions, aliasing, and alignment.
- Struct/union layout, padding, packing, bitfields, and enum representation.
- Array decay, flexible arrays, function pointers, callbacks, and variadic interfaces.
- Global initialization, zero-initialized storage, tentative definitions, and symbol visibility.
- `volatile`, memory-mapped hardware, interrupt-shared state, and ordering constraints.
- Compiler extensions, pragmas, inline assembly, and undefined or implementation-defined behavior.

Rust equivalents must be explicit where Rust's rules differ. Prefer fixed-width integer types, explicit wrapping operations where the original relies on wraparound, and carefully documented `#[repr(C)]` layouts where ABI compatibility is required. Do not assume `repr(C)` alone reproduces every compiler-specific C layout detail.

### 3.2 ABI and machine model

For each target, document:

- CPU architecture and instruction set mode (for GBA, ARM/Thumb state matters).
- Endianness, pointer width, alignment, stack conventions, register use, and calling convention.
- Struct and global symbol layout.
- Interrupt entry/exit behavior and hardware register access.
- Linker script, section placement, ROM address map, and fixed-address assumptions.

### 3.3 Compiler and linker behavior

The original build may depend on a particular compiler version, flags, optimization level, assembler, linker, archive ordering, section garbage collection, and post-link tools.

A Rust compiler normally makes different code-generation decisions from the original C compiler. Even semantically equivalent Rust can produce different instructions, register allocation, function ordering, padding, relocations, and section layout. Therefore:

**A C-to-Rust port does not imply byte-identical output.** Exact matching may require reproducing the original toolchain's code-generation behavior, retaining original assembly for selected routines, controlling object/link order, or using a hybrid pipeline. If those constraints prove incompatible with a fully Rust-generated binary, record that result rather than hiding it.

## 4. Experimental protocol

### Phase A — Establish the original baseline

1. Select an exact game, region, revision, and reference ROM.
2. Record the reference ROM's file size and SHA-1/SHA-256.
3. Follow the project's installation instructions and pin toolchain versions.
4. Build the untouched upstream project.
5. Verify the generated ROM against the published hash and a byte-level comparison.
6. Save build commands, environment, tool versions, and artifacts metadata.

Do not start porting until the baseline build is reproducible.

### Phase B — Map the original program

Inventory the source tree and classify each component:

- C gameplay/runtime code.
- Handwritten assembly and compiler-generated assembly.
- Startup code, interrupt handlers, and hardware interfaces.
- Data tables, constants, assets, graphics, audio, maps, scripts, and resources.
- Linker scripts, symbol maps, build scripts, and ROM packaging.
- Generated files and tools that transform source assets into binary data.

For every unit, record dependencies, public symbols, memory assumptions, and whether it contributes bytes to the final ROM.

### Phase C — Port C units to Rust incrementally

1. Choose a small, isolated C module.
2. Write down its inputs, outputs, global state, side effects, and ABI.
3. Port the implementation to Rust without redesigning its behavior.
4. Preserve externally visible names and layouts where required.
5. Keep handwritten assembly and hardware-sensitive code unchanged initially.
6. Build both versions and compare unit-level outputs and state transitions.
7. Add regression tests before moving to the next module.

Do not translate the entire game in one pass. A small differential loop makes mismatches attributable.

### Phase D — Investigate code generation

For each ported unit:

- Generate and archive Rust compiler assembly output.
- Compare C and Rust assembly side by side.
- Inspect instruction selection, register allocation, stack frames, prologues/epilogues, literal pools, alignment, and symbol placement.
- Experiment with compiler flags and target features in isolated builds.
- Identify which differences are local and which change downstream addresses or relocations.
- Preserve a known-good baseline for every experiment.

A matching function's machine code is not enough if its location, relocation records, padding, or referenced data differs in the final image.

### Phase E — Close the binary gap

Classify every differing ROM range:

- Code bytes.
- Constant/data bytes.
- Asset/resource bytes.
- Alignment or padding.
- Header/checksum fields.
- Relocations, symbol ordering, or section placement.
- Build-generated metadata.

Fix the cause, not just the observed byte. Avoid blindly patching output bytes: a patch may conceal an incorrect source port and break when neighboring code changes.

A hybrid result may be the most honest architecture: Rust for reconstructed logic, original assembly for irreducible matching routines, and deterministic original-compatible asset/link/packaging stages.

### Phase F — Verification gates

A candidate is not “byte-perfect” until all required checks pass:

- Clean build from a pinned environment.
- Expected output size.
- Expected reference SHA-1 and SHA-256.
- Full byte-for-byte comparison.
- First differing offset and total differing-byte count when failing.
- Optional per-region comparison using a documented ROM map.
- Repeated clean builds produce identical output.

Keep behavioral tests even after a binary match. Binary identity proves the artifact, while differential tests explain and protect the porting process.

## 5. First case study: FireRed / LeafGreen

Repository: https://github.com/pret/pokefirered

The upstream README declares these English ROM outputs and SHA-1 values:

| Output | SHA-1 |
|---|---|
| `pokefirered.gba` | `41cb23d8dccc8ebd7c649cd8fbb58eeace6e2fdc` |
| `pokeleafgreen.gba` | `574fa542ffebb14be69902d1d36f1ec0a4afd71e` |
| `pokefirered_rev1.gba` | `dd5945db9b930750cb39d00c84da8571feebf417` |
| `pokeleafgreen_rev1.gba` | `7862c67bdecbe21d1d69ce082ce34327e1c6ed5e` |
| `pokefirered_switch.gba` | `baa452d0b24629dd7782cfc07a8984085dde1311` |
| `pokeleafgreen_switch.gba` | `62b9fc77549dbc67032eb6cbd0ea6ad3b825690f` |

The project README is the authority for the current complete list and should be checked again when selecting a target revision.

Why start here:

- The project describes itself as a decompilation of English FireRed and LeafGreen.
- It declares multiple regional/revision-derived outputs, making revision selection and reproducibility explicit.
- It offers a substantial C/assembly codebase for incremental port experiments.

First target recommendation: choose one exact FireRed revision, reproduce its upstream build, then port one small pure-C module before touching hardware-facing code.

## 6. Second case study: Emerald

Repository: https://github.com/pret/pokeemerald

The upstream README declares:

- Output: `pokeemerald.gba`
- SHA-1: `f3ae088181bf583e55daf962a92bb46f4f1d07b7`

Why study it:

- C and ARM/Thumb assembly coexist.
- It contains gameplay systems, battle logic, scripts, maps, assets, and runtime infrastructure.
- It is a useful second test of whether lessons from FireRed generalize or depend on a particular codebase/build.

Do not assume the FireRed build system, ABI assumptions, compiler configuration, or code organization transfer unchanged to Emerald.

## 7. Other pret ROM reconstruction references

These projects publish target ROM hashes in their READMEs. Their project status and completeness differ; a listed hash is a declared build target, not a claim that every source file is fully decompiled or that a Rust port exists.

### Game Boy / Game Boy Color

- [pret/pokered — Pokémon Red and Blue](https://github.com/pret/pokered)
- [pret/pokeyellow — Pokémon Yellow](https://github.com/pret/pokeyellow)
- [pret/pokegold — Pokémon Gold and Silver](https://github.com/pret/pokegold)
- [pret/pokecrystal — Pokémon Crystal](https://github.com/pret/pokecrystal)

### Game Boy Advance

- [pret/pokeruby — Pokémon Ruby and Sapphire](https://github.com/pret/pokeruby)
- [pret/pokeemerald — Pokémon Emerald](https://github.com/pret/pokeemerald)
- [pret/pokefirered — Pokémon FireRed and LeafGreen](https://github.com/pret/pokefirered)

### Nintendo DS — additional, work-in-progress references

- [pret/pokediamond — Pokémon Diamond / Pearl](https://github.com/pret/pokediamond)
- [pret/pokeplatinum — Pokémon Platinum](https://github.com/pret/pokeplatinum)
- [pret/pokeheartgold — Pokémon HeartGold / SoulSilver](https://github.com/pret/pokeheartgold)

The DS repositories describe themselves as WIP decompilation/disassembly projects. In particular, pokeheartgold warns that it is not fully decompiled and that changes affecting function or data-structure size can crash the game. Treat these as exploratory references, not as equivalent in maturity to the selected GBA baselines.

## 8. Rules for the research

- Pin repository commits and toolchains; never cite a moving branch as the only reproducibility record.
- Record exact ROM revision and provenance. Do not redistribute copyrighted ROM images.
- Use published hashes as identifiers; obtain reference ROMs lawfully.
- Preserve upstream licenses and attribution when reusing code.
- Keep original C builds available as the control group.
- Separate source translation, semantic equivalence, ABI compatibility, and byte matching in reports.
- Never claim a Rust port is byte-perfect until the produced ROM has passed the complete comparison.

## 9. Expected deliverables

For each case study, maintain:

1. Upstream baseline build instructions and pinned commit.
2. ROM identity manifest (revision, size, SHA-1, SHA-256).
3. C-to-Rust module inventory and dependency map.
4. Rust port with comments documenting C-specific semantic decisions.
5. Differential tests and reproducible test inputs.
6. Assembly/code-generation comparison artifacts.
7. ROM diff reports with offsets and region classification.
8. A final feasibility report describing what matched, what did not, and why.

The result should become a repeatable engineering recipe for the Impossible Engine, not a one-off Pokémon port.
