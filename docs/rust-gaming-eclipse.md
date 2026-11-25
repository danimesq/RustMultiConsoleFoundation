# 🌘 Rust Gaming Eclipse: shared architecture and integration contract

**Status:** Shared architecture proposal, 2026-11-24.  
**Role of this document:** Cross-repository reference for StreetChavesRust, ImpossibleEngine, and RustMultiConsoleFoundation.  
**Implementation status:** This document defines future alignment. It does not claim that a complete Foundation runtime API or a Bevy bridge already exists.

## 🌘 1. The Eclipse: three projects, three responsibilities

Rust Gaming Eclipse aligns three independent repositories:

| Repository | Product role | Owns |
|---|---|---|
| [StreetChavesRust](https://github.com/danimesq/StreetChavesRust) | Consumer game / demanding integration reference | Street Chaves rules, simulation, source evidence, game data, replay, game-specific rendering/audio intent |
| [ImpossibleEngine](https://github.com/danimesq/ImpossibleEngine) | Authoring and compilation environment | Project format, Event Sheets, semantic specification, typed IR, compiler/lowering, generated-game composition |
| **RustMultiConsoleFoundation** | Shared platform and delivery infrastructure | Public platform contracts, capability profiles, platform adapters, asset adaptation, build/toolchain orchestration, packaging |

The dependency graph is intentionally one-way:

```text
             StreetChavesRust       ImpossibleEngine
                game core             editor/compiler
                     \                 /
                      \               /
                       v             v
                RustMultiConsoleFoundation
              contracts · adapters · capabilities
                 · assets · build · packaging
                              |
                              v
                  platform SDKs / host APIs
```

The Foundation must not depend on either consumer. ImpossibleEngine and StreetChavesRust may consume a versioned public Foundation API independently. They do not depend on one another.

## 🌘 2. Keep four architectural axes distinct

A cross-platform game build is a combination of separate choices:

1. **Target platform:** Windows, Linux, Web, mobile, PSP, GBA, PS1, and so on.
2. **Host/runtime:** Foundation-native host, browser host, console SDK, or another validated platform adapter.
3. **Game engine organization:** hand-authored game core, ECS, Bevy-hosted app, or a specialized runtime.
4. **Physics provider:** game-specific deterministic rules, standalone Rapier, Bevy Rapier, or another explicitly chosen implementation.

These are not synonyms. Bevy is a game engine, Rapier is a physics engine, SDL is a multimedia/platform library, and this repository is intended to provide reusable platform contracts and target integrations. None should be presented as a drop-in replacement for the others.

## 🌘 3. Foundation public API: the first missing piece

The current RustMultiConsoleFoundation Cargo workspace contains only `crates/foundation-packaging`. The repository has research and architecture documents for broader platform abstractions, but it does not yet expose the general-purpose game runtime crate envisioned by these consumers.

The next implementation should introduce a small, versioned public API only after the contract is agreed. The exact package/crate name is intentionally not frozen in this proposal.

The minimum contract families to design are:

| Contract family | Game-facing responsibility |
|---|---|
| Time and scheduling | Monotonic elapsed time, fixed-tick opportunities, frame pacing, pause/resume lifecycle |
| Input | Normalized keyboard, pointer, touch, and controller actions; pressed/held/released distinctions |
| Display/presentation | Logical resolution, viewport/scaling, frame or draw-data submission |
| Audio output | Semantic audio requests and capability/error reporting |
| Assets | Stable logical asset IDs, target-specific resolution, loading and validation |
| Storage | Save/config access through explicit capabilities |
| Diagnostics | Structured logs, backend identity, capability states and actionable errors |
| Build/target metadata | Target profile, toolchain identity, feature/capability manifest |
| Packaging | Reuse the existing packaging adapter where appropriate, without pretending packaging compiles the game |

Keep the API narrow and platform-neutral. Do not expose SDL event unions, SDL windows/textures, Orbital handles, browser DOM objects, console SDK structs, Bevy entities, or Rapier rigid-body types through the generic contract.

The Foundation should report whether a capability is supported, unavailable, emulated/fallback, degraded, or unknown. A request that cannot be fulfilled must produce a visible fallback or typed error, never a silent success.

### 🌘 API release gates

Before a consumer adds a Foundation dependency, the Foundation must provide:

- A real workspace crate and public Rust module path.
- A documented feature/target matrix and declared MSRV.
- A minimal compiling consumer example.
- At least one validated host adapter and a headless/mock path for tests.
- Capability and error semantics.
- Versioning and compatibility policy.

During development, consumers should pin an immutable Git tag/revision. Once the API is published, use semver dependencies. Do not depend on a moving branch. No consumer should guess a package name or import path before this gate is met.

## 🌘 4. Foundation-hosted and Bevy-hosted modes

A platform build should have one clear owner for each host responsibility.

### A. Foundation-hosted

The Foundation chooses the platform adapter and owns the host integration that it provides: window/display, normalized input, time, audio output, storage, diagnostics, and related platform services. A game supplies its own domain logic and renderer-neutral presentation data.

This is the lowest-friction mode for games that do not need a complete external game engine.

### B. Bevy-hosted

Bevy owns its own app schedule and the Bevy-managed window/input/rendering/asset path. A future Foundation-Bevy bridge may expose compatible Foundation capabilities as Bevy resources or plugins and delegate host responsibilities to Bevy.

The bridge must not open a second window, run a competing event loop, or create duplicate input/audio/display ownership. It is optional and only applies to targets on which the full Bevy dependency stack is supported and validated.

These modes are alternatives for a given host responsibility, not two layers that blindly initialize alongside each other.

## 🌘 5. Bevy and Rapier: integration policy

Research snapshot as of 2026-11-24:

- Bevy **0.20.1** is the latest stable release found (2026-10-28).
- `bevy_rapier2d` **0.37.0** declares a dependency on Bevy `^0.20`; use a matching release line.
- Bevy's official platform overview lists Windows, macOS, Linux, Web, iOS, and Android. That list does not establish PSP or classic-console support.
- Bevy's crate documentation warns that the engine is still early-stage, that breaking API changes arrive approximately every three months, and that its MSRV follows the latest stable Rust.
- Bevy 0.20's release highlights include Solari and DLSS work. Advanced rendering features must remain optional and must not be mistaken for universal target capabilities.

References:

- https://github.com/bevyengine/bevy/releases
- https://bevy.org/news/bevy-0-20/
- https://bevy.org/
- https://docs.rs/crate/bevy/0.20.1
- https://docs.rs/crate/bevy_rapier2d/0.37.0
- https://www.rapier.rs/docs/user_guides/bevy_plugin/getting_started_bevy/

Policy:

- Bevy is an optional engine/runtime integration, not a mandatory Foundation dependency.
- The Foundation's core platform contracts must not depend on Bevy ECS, Bevy rendering, or Bevy windowing.
- A Foundation-Bevy integration must live behind an optional adapter boundary.
- A target profile may select Bevy only when its toolchain and host services are validated.
- Rapier is a physics provider, not a platform backend. It may be used standalone or through `bevy_rapier2d`/3D when the chosen runtime is Bevy.
- Physics adoption is a game-level decision. It must not silently replace a consumer's source-compatible collision semantics.

## 🌘 6. StreetChavesRust: first demanding consumer

StreetChavesRust is useful because it exercises much more than a window and a moving rectangle: it has a deterministic 28 ms game tick, source-ordered combat behavior, explicit RNG compatibility, strict AABB geometry, hit/hurt-box lifecycle rules, audio intent, asset handling, and planned native/Web presentation.

Integration contract:

1. Keep the game core headless and authoritative.
2. Keep renderer-neutral frame output, logical input, audio events, and the fixed-tick simulation boundary independent of a selected host.
3. Use the Foundation only after its runtime API exists.
4. Evaluate Bevy as a separate hosted mode. Do not rewrite the core into ECS merely to use Bevy.
5. Keep Rapier optional and isolated until differential tests establish whether it preserves the required behavior.
6. Record each target/capability result independently. A successful SDL window or Bevy desktop run is not proof of console support.

The game remains independent. It is a consumer and an integration example, not source code to be absorbed into the Foundation.

## 🌘 7. ImpossibleEngine: compiler and authoring consumer

ImpossibleEngine owns the higher-level project and compilation pipeline:

```text
Project JSON → schema/parser → semantic IR → target lowering
            → generated game project → Foundation API
            → target adapter → build/link/assets/package
```

The Foundation does not parse Event Sheets or define game semantics. ImpossibleEngine does not implement SDL/Orbital/browser/console adapters.

For native Rust and supported Web targets, generated projects may call the public Foundation API. For constrained consoles, bare-metal systems, ROM outputs, or targets without a suitable Foundation game runtime, ImpossibleEngine may use a dedicated source, assembly, binary, or hybrid lowering path. The Foundation's game runtime API must not be imposed on every target.

Bevy is represented as an optional game-engine/runtime choice, not as a synonym for a platform. Event Sheet IR must remain independent of Bevy types. The generated project selects Bevy only when the selected host profile and toolchain support it.

StreetChavesRust can validate a future import/migration path, but the compiler must not depend on its crate or copy its game-specific rules into generic infrastructure. Unsupported imported semantics must be reported explicitly.

## 🌘 8. Recommended delivery order

| Stage | Repository | Deliverable / exit gate |
|---|---|---|
| 0 | All three | Merge this architecture proposal after review; no runtime dependency changes yet |
| 1 | RustMultiConsoleFoundation | Extract and document a minimal runtime API crate; test a headless/mock consumer and one real host adapter |
| 2 | RustMultiConsoleFoundation | Validate multiPong as the small conformance/reference application and expose capability diagnostics |
| 3 | StreetChavesRust | Add a minimal Foundation consumer adapter on one supported target; preserve deterministic core tests |
| 4 | RustMultiConsoleFoundation / StreetChavesRust | Run an isolated Bevy + matching Rapier compatibility spike; record MSRV, build, ownership, fixed-tick, and target results |
| 5 | ImpossibleEngine | Define typed Event Sheet IR and generate a tiny Rust app against the real Foundation contract |
| 6 | ImpossibleEngine / StreetChavesRust | Import StreetChaves incrementally; gate each migrated semantic slice with deterministic fixtures |
| 7 | All three | Expand target adapters and specialized console lowering without weakening capability or provenance reporting |

The merge order for these documentation PRs should be Foundation contract first, then the StreetChaves consumer plan and ImpossibleEngine compiler plan. The PRs are cross-referenced so that reviewers can inspect the whole proposal together.

## 🌘 9. Explicit non-goals

This proposal does not:

- Add Bevy or Rapier as dependencies to all three repositories.
- Replace SDL with Rapier or treat Bevy as a universal console runtime.
- Claim that the Foundation already exports a runtime crate.
- Make ImpossibleEngine depend on StreetChavesRust.
- Move all gameplay into Event Sheets or promise a lossless Street Chaves import today.
- Claim support for PSP, PS1, N64, or any console merely because a CPU backend or profile exists.
- Merge the repositories into one workspace or monorepo.

## 🌘 10. Open decisions for implementation PRs

- Final name, feature layout, MSRV, and semver policy for the Foundation runtime crate.
- Exact API ownership of fixed-tick scheduling versus host frame pacing.
- Whether the Bevy bridge is maintained in this repository or a sibling integration crate.
- Which backends own audio and asset loading in Bevy-hosted mode.
- Whether standalone Rapier has a consumer use case that is demonstrably better than game-specific physics.
- The interchange/package format through which ImpossibleEngine imports existing Rust projects.
