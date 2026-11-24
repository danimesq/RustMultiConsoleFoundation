# Impossible Engine: compiler architecture

## 1. Long-term direction

Impossible is envisioned as a Rust-based authoring/compiler system in which declarative JSON event-sheets can be lowered into native implementations for different engines, runtimes, and historical console targets.

The aspirational pipeline is:

`Event-Sheets JSON → parser/validation → semantic IR → Rust compiler core → target-specific lowering → native source/assembly/binary → linker and asset pipeline → ROM or executable`

This is a long-term architecture, not an existing capability.

## 2. Separate the authoring model from target implementation

Event-sheets should describe gameplay intent and ordering, not hard-code a particular console's memory model.

The compiler should preserve:

- Event-sheet hierarchy and event ownership.
- Conditions, actions, subevents, loops, and execution order.
- Trigger versus per-tick evaluation semantics.
- Object and instance identity/lifetime.
- Variables, expressions, parameters, and type conversions.
- Waits, timers, animation state, collision state, and scene transitions.
- Deterministic ordering and explicit nondeterminism/RNG sources.

Target-specific behavior must be represented explicitly when a target cannot implement a semantic feature identically.

## 3. Proposed compiler crates

These are proposed boundaries, not a commitment to create every crate immediately.

| Crate | Responsibility |
|---|---|
| `impossible-event-sheet` | JSON schema, parsing, diagnostics, versioning |
| `impossible-ir` | Typed intermediate representation |
| `impossible-semantics` | Normative meaning of events and actions |
| `impossible-scheduler` | Tick/frame ordering, tasks, callbacks, timers |
| `impossible-lowering` | Transform common IR into target IR |
| `impossible-backend-gb` | Game Boy target lowering |
| `impossible-backend-gba` | Game Boy Advance target lowering |
| `impossible-backend-nds` | Nintendo DS target lowering |
| `impossible-backend-ps1` | PlayStation target lowering |
| `impossible-backend-n64` | Nintendo 64 target lowering |
| `impossible-linker` | Link orchestration, section/layout manifests |
| `impossible-assets` | Deterministic asset conversion and packing |
| `impossible-rom` | Console image packaging and validation |
| `impossible-matcher` | Hashing, byte comparison, diff reports |

Crate boundaries should follow actual dependency direction and implementation needs. Do not create empty crates merely to satisfy a diagram.

## 4. Three backend strategies

A target may use one or more strategies:

### Source backend

Generate source in the target's native language, such as C or C++. This is useful when a mature target toolchain and source-level runtime already exist.

### Assembly backend

Generate target assembly directly when exact instruction selection, register use, memory layout, or low-level hardware control is required.

### Binary backend

Emit machine code or binary structures directly. This offers control but carries the highest burden for relocations, instruction encoding, ABI, alignment, and linker integration.

A backend may combine strategies: generated C plus handwritten assembly, Rust runtime code plus target assembly, or generated object sections plus a deterministic packager.

## 5. Rust is the compiler core, not necessarily the target runtime

Rust should own parsing, validation, semantic analysis, IR transformations, scheduling models, target lowering, build orchestration, and verification.

The final game does not need to contain a Rust runtime. A backend may emit C, assembly, object code, or ROM data for a target that cannot run Rust natively.

## 6. Matching reconstruction is a separate mode

Keep two product goals distinct:

- **Native export**: compile a new game authored in Impossible for a selected target.
- **Matching reconstruction**: reconstruct a specific existing game and verify that the produced binary matches a selected original revision.

Matching reconstruction needs a reference manifest, pinned toolchain, known ROM layout, and byte-level verifier. It is not a magic property of arbitrary event-sheets.

The Pokémon FireRed and Emerald C-to-Rust experiments are intended to teach the project how source semantics, ABI, compiler behavior, assembly, assets, and linking interact. Their findings should inform the design of the Impossible compiler, but should not be confused with a generic event-sheet export path.

## 7. Determinism and fidelity

The IR and scheduler must define ordering rather than relying on incidental host behavior.

Specify:

- Tick/frame boundaries and update phases.
- Event traversal and subevent order.
- Callback/task scheduling and cancellation.
- Timer precision and rounding.
- Numeric widths, overflow, and conversion rules.
- RNG algorithms, seeds, and call ordering.
- Collision evaluation and resolution phases.
- Animation advancement and pause/slow-motion behavior.
- Serialization and asset ordering.

A deterministic IR is necessary for repeatable builds and behavior, but does not by itself guarantee byte-identical machine code. Code generation and linking must also be controlled.

## 8. Proposed implementation order

1. Establish FireRed's untouched upstream matching build.
2. Establish Emerald's untouched upstream matching build.
3. Map source, assembly, assets, scripts, linker configuration, and build tools.
4. Port one isolated C module to Rust and write differential tests.
5. Investigate ABI/layout and compare generated assembly.
6. Expand module-by-module only after each step is reproducible.
7. Document the feasible matching boundary and any hybrid assembly/toolchain requirements.
8. Extract reusable compiler lessons into Impossible's IR, semantics, scheduler, lowering, and backend design.
9. Prototype one small event-sheet that targets a constrained console feature set.
10. Validate native export separately from reconstruction matching.

## 9. Non-goals and guardrails

- Do not promise byte-perfect output from arbitrary Rust source.
- Do not conflate emulator accuracy with compiler or ROM matching.
- Do not assume one console's ABI or scheduler applies to another.
- Do not replace original toolchains before a reproducible baseline exists.
- Do not hide binary mismatches behind post-build patches.
- Do not make the generic IR encode quirks that belong only to one backend.

The guiding principle: define gameplay semantics once, then lower them deliberately to target-specific implementations while keeping fidelity claims measurable.
