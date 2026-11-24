# Pokémon Red → Rust, and a shared Red / FireRed source strategy

## 1. Updated experiment order

The first practical experiment is now **Pokémon Red/Blue**, not FireRed:

1. Use the mature [pret/pokered](https://github.com/pret/pokered) disassembly as the original byte-matching reference.
2. Study and compare the existing Rust reimplementations:
   - [liuyanghejerry/open-pokered](https://github.com/liuyanghejerry/open-pokered)
   - [xjose97x/pokerust](https://github.com/xjose97x/pokerust)
3. Audit what each Rust project implements, how it models the original game's data and behavior, and what would need to change for a ROM-producing, byte-matching pipeline.
4. Establish the untouched pret build as the control, then begin a controlled Rust port/reconstruction experiment.
5. Use the findings to design the larger shared-source Red + FireRed architecture described below.

The Rust projects are **comparison subjects and sources of engineering lessons**, not assumed byte-perfect ports. Their stated goals are native/platform-independent reimplementations; do not describe them as generating a byte-identical Game Boy ROM unless a reproducible build and full binary comparison prove that.

## 2. The Red reference and its exact outputs

Reference repository: [pret/pokered](https://github.com/pret/pokered)

The upstream README declares these outputs:

| Output | SHA-1 |
|---|---|
| Pokémon Red (UE) [S][!].gb | ea9bcae617fdf159b045185467ae58b2e4a48b9a |
| Pokémon Blue (UE) [S][!].gb | d7037c83e1ae5b39bde3c30787637ba1d4c48ce2 |
| BLUEMONS.GB (debug build) | 5b1456177671b79b263c614ea0e7cc9ac542e9c4 |
| dmgapae0.e69.patch | 0fb5f743696adfe1dbb2e062111f08f9bc5a293a |
| dmgapee0.e68.patch | ed4be94dc29c64271942c87f2157bca9ca1019c7 |

These hashes are target identities, not permission to redistribute ROMs. Re-check the upstream README when pinning the actual experiment because upstream targets can evolve.

The first baseline should be one exact target, preferably the declared Pokémon Red (UE) image. Record the pret commit, RGBDS version, build environment, output size, SHA-1 and SHA-256. Build the unmodified disassembly before changing anything.

## 3. What the Rust projects offer

### 3.1 open-pokered

Repository: https://github.com/liuyanghejerry/open-pokered

A Rust rewrite/reimplementation based on the pret Red/Blue disassembly. It targets multiple host platforms and separates reusable engine functionality from game-specific data and behavior.

Research questions:
- Which game systems and data structures are already represented?
- How closely do its scripts, maps, saves, graphics, audio and timing correspond to pret?
- Which parts are faithful translations, which are redesigned abstractions, and which remain incomplete?
- Can parsing, data-model, test or behavior-validation work be reused without importing assumptions that prevent ROM matching?

It is not established as a Game Boy ROM compiler or byte-perfect rebuild.

### 3.2 pokerust

Repository: https://github.com/xjose97x/pokerust

An experimental native Rust Red/Blue reimplementation that uses a local pret/pokered checkout as a source of reference behavior and assets. Its reported scope is early/partial, including title-screen and basic overworld functionality.

Research questions:
- How does it read and map pret data?
- Which mechanics are implemented and how are they tested?
- What is the relationship between its runtime model and the original GB memory/CPU model?
- Are there useful small modules or test cases for differential experiments?

It is also not established as a byte-perfect ROM-producing port.

### 3.3 Keep the categories separate

| Project type | What it proves | What it does not prove |
|---|---|---|
| Native Rust reimplementation | The game can be recreated in Rust for another runtime/platform | Original Game Boy ROM bytes can be reproduced |
| Rust source port | Original logic has been translated into Rust | Identical machine code or final ROM |
| ROM-producing Rust toolchain | Rust participates in generating a target ROM | Byte identity, unless measured |
| Byte-perfect reconstruction | The produced ROM passes full binary comparison against a pinned reference | That the same source/build works for another revision or console |

The first task is a comparative audit, not a blind merge or wholesale adoption of either Rust repository.

## 4. Red's byte-perfect challenge differs from GBA C → Rust

Pokémon Red's pret project is a **disassembly** written primarily in Game Boy SM83 assembly and macros. It is not a C codebase waiting for a mechanical C → Rust translation.

That makes the experiment especially useful, but changes the route:

- Understand the original assembly, macros, memory map, bank layout, symbols, data declarations and RGBDS build pipeline.
- Determine which Rust components can model/validate the game and which must ultimately lower to GB instructions, data, bank placement and linker/layout directives.
- Compare the Rust reimplementations against the original disassembly at the system/behavior/data level.
- For byte matching, preserve or regenerate exact target assembly and data layout. A host-native Rust executable is not a substitute for a Game Boy ROM.
- Treat Rust as the compiler/orchestration/semantic-analysis core if necessary, with a GB-specific assembler/backend responsible for the actual target artifact.

A normal Rust compiler targeting a host CPU cannot emit a Game Boy ROM. Even a Rust compiler targeting a suitable CPU architecture would not automatically reproduce RGBDS instruction choices, bank placement, macros, symbol ordering or padding.

## 5. Ambitious direction: one Rust source family for Red and FireRed

Long-term proposal:

**Create a shared Rust source architecture that contains reusable Pokémon Red logic and can also serve as the shared foundation for Pokémon FireRed, while independently producing byte-perfect outputs for both original targets.**

Conceptual view:

    Shared Pokémon source / semantic core (Rust modules)
                       |
             +---------+---------+
             |                   |
       Game Boy profile      GBA FireRed profile
       Red / Blue data       FireRed data
       GB-specific systems   GBA-specific systems
             |                   |
       GB target lowering   GBA target lowering
       SM83 instructions    ARM7TDMI ARM/Thumb
       RGBDS-compatible     original-compatible
       layout               C/ASM/link pipeline
             |                   |
       Red/Blue ROM         FireRed ROM
             |                   |
       compare to pret      compare to pret
       pokered              pokefirered
             |                   |
       independent byte     independent byte
       gate                 gate

This is a research target, not a claim that Red and FireRed already share a directly portable source tree.

### 5.1 Share semantics and genuinely common logic, not bytes by assumption

Red and FireRed are different generations and platforms. FireRed is a later remake with substantial engine, data, feature, hardware and implementation differences. The architecture must not force superficial code sharing.

Potential shared layers:
- Domain concepts and canonical data models where meanings truly align.
- Pokémon species/moves/items concepts, with versioned data profiles.
- Formula specifications where the games demonstrably use the same formula.
- Text/script semantic models and validation tools.
- Deterministic RNG abstractions with target/game-specific algorithms and call ordering.
- Battle concepts and state-machine specifications, only where behavior is verified to match.
- Test vectors and cross-version comparison harnesses.
- Source parsing, symbol inventories, asset manifests and ROM diff tooling.

Keep target/game-specific:
- CPU instructions, ABI, calling conventions and memory maps.
- Rendering, audio, input and hardware interfaces.
- Save formats and storage layout.
- Game-specific battle rules, data, scripts, maps and event ordering.
- Timing, frame/tick behavior, interrupt handling and engine internals.
- Linker scripts, bank/section placement, assembly and ROM packaging.

Use explicit capabilities and versioned profiles rather than implicit conditional compilation.

### 5.2 Suggested Rust workspace

These are candidate boundaries. Split crates only when dependency direction and implementation justify them.

| Component | Responsibility |
|---|---|
| pokemon-shared-model | Shared domain types and versioned concepts |
| pokemon-red-data | Red/Blue-specific constants, tables, scripts and asset metadata |
| pokemon-firered-data | FireRed-specific constants, tables, scripts and asset metadata |
| pokemon-rules | Explicitly versioned formulas and rule specifications |
| pokemon-differential-tests | Cross-version vectors and behavior comparisons |
| pokered-rust | Red/Blue reconstruction source and GB profile |
| pokefirered-rust | FireRed reconstruction source and GBA profile |
| gb-target | GB instruction/data lowering and RGBDS-compatible output |
| gba-target | GBA ARM/Thumb and compatible object/ROM pipeline |
| rom-matcher | Hashing, byte diffs, offsets and region classification |

A shared Rust crate may be used by both builds only where its semantics and target-specific lowering can preserve each original. If a shared abstraction changes generated bytes or behavior, keep the original-specific implementation separate.

### 5.3 The dual byte-perfection rule

There are two independent acceptance gates.

**Red gate**
- Pin a specific Red revision and pret commit.
- Reproduce the original pret build.
- Build the Rust-driven GB reconstruction.
- Compare size, SHA-1/SHA-256 and every byte against the selected Red reference.

**FireRed gate**
- Pin a specific FireRed revision and pret commit.
- Reproduce the original pret/pokefirered build.
- Build the Rust-driven GBA reconstruction.
- Compare size, SHA-1/SHA-256 and every byte against the selected FireRed reference.

A pass on one target does not imply a pass on the other. Shared source is an architectural goal; exact output is a target-specific measured result.

### 5.4 A realistic staged route

1. **Audit the references.** Read pret/pokered, open-pokered and pokerust side by side. Record implemented systems, source/data provenance, architecture, test coverage and known gaps.
2. **Reproduce Red.** Build the unmodified pret project and verify its declared ROM hash.
3. **Build a Red behavior map.** Map assembly routines, data tables, RAM layout, bank structure, scripts and assets.
4. **Create the Rust model.** Begin with data readers, parsers, validators and deterministic test harnesses. Do not claim this is yet the ROM implementation.
5. **Choose a tiny vertical slice.** Reconstruct one isolated subsystem end-to-end, with original assembly/data retained wherever needed.
6. **Prove the GB pipeline.** Generate a real ROM and measure binary differences. Find whether Rust can orchestrate or generate parts without breaking exact output.
7. **Audit FireRed independently.** Establish the original GBA baseline and map its C, assembly, assets, ABI, linker and build pipeline.
8. **Identify real shared semantics.** Compare Red and FireRed behavior and data. Promote only verified common concepts into shared Rust modules.
9. **Build the FireRed profile.** Keep GBA-specific implementation and binary constraints independent.
10. **Run both acceptance gates continuously.** Every shared-code change must run Red and FireRed behavioral tests and both ROM comparisons.

## 6. Relationship to the existing FireRed and Emerald C → Rust study

The earlier FireRed and Emerald plan remains valid, but the project sequence changes:

- **First:** Red/Blue disassembly + Rust reimplementation comparison, then a byte-matching GB experiment.
- **Second:** FireRed as the ambitious shared-source counterpart and a separate GBA byte-matching experiment.
- **Also:** Emerald remains a major C/ARM/Thumb → Rust study, useful for testing whether the GBA findings generalize beyond FireRed.

FireRed and Emerald remain distinct codebases. Do not assume a successful FireRed port automatically applies to Emerald.

## 7. Complete repository inventory mentioned in this research

### Original pret ROM disassembly/decompilation references

**Game Boy / Game Boy Color**
- [pret/pokered](https://github.com/pret/pokered) — Pokémon Red/Blue
- [pret/pokeyellow](https://github.com/pret/pokeyellow) — Pokémon Yellow
- [pret/pokegold](https://github.com/pret/pokegold) — Pokémon Gold/Silver
- [pret/pokecrystal](https://github.com/pret/pokecrystal) — Pokémon Crystal

**Game Boy Advance**
- [pret/pokeruby](https://github.com/pret/pokeruby) — Pokémon Ruby/Sapphire
- [pret/pokeemerald](https://github.com/pret/pokeemerald) — Pokémon Emerald
- [pret/pokefirered](https://github.com/pret/pokefirered) — Pokémon FireRed/LeafGreen

**Nintendo DS**
- [pret/pokediamond](https://github.com/pret/pokediamond) — Pokémon Diamond/Pearl
- [pret/pokeplatinum](https://github.com/pret/pokeplatinum) — Pokémon Platinum
- [pret/pokeheartgold](https://github.com/pret/pokeheartgold) — Pokémon HeartGold/SoulSilver

The DS projects have differing maturity/completeness and should remain exploratory references until their exact target/build status is audited.

### Rust reimplementations discussed

- [liuyanghejerry/open-pokered](https://github.com/liuyanghejerry/open-pokered) — Rust Red/Blue reimplementation.
- [xjose97x/pokerust](https://github.com/xjose97x/pokerust) — experimental native Rust Red/Blue reimplementation.
- [LunchBox951/pokeemerald-rs](https://github.com/LunchBox951/pokeemerald-rs) — pre-alpha native Rust Emerald reimplementation; requires user-supplied Emerald ROM content and is not a byte-perfect ROM rebuild.
- [Lyspresso/pok4](https://github.com/Lyspresso/pok4) — Rust runtime project discussed for Red, Gold, Crystal and an experimental Platinum slice; not a matching ROM compiler.

### Foundation and architecture repository

- [DaniMesq/RustMultiConsoleFoundation](https://github.com/DaniMesq/RustMultiConsoleFoundation) — this research, methodology and future cross-console/compiler foundation.

## 8. Evidence and claim discipline

- Treat upstream pret READMEs and pinned commits as authority for declared target ROM hashes.
- Treat the Rust projects' own READMEs and source as authority for their actual implementation scope.
- Do not call a native reimplementation a decompilation or a byte-perfect port.
- Do not call a ROM build byte-perfect until a clean reproducible build passes a full byte comparison.
- Keep copyrighted ROM images out of the repository; record hashes and instructions, and require lawful user-provided reference images.
- Preserve licenses and attribution for any reused code or data.

The ambition is deliberately large: one Rust-centered source family, two very different hardware targets, and two independent byte-perfect proofs. The architecture must earn each shared abstraction through evidence, not force both games into the same mold.
