# Bevy integration strategy

**Status:** architectural proposal; documentation only  
**Date:** 2026-11-28  
**Scope:** evaluate Bevy as an optional ECS/runtime/rendering backend for RustMultiConsoleFoundation while preserving the Foundation's platform-neutral contracts and multi-target purpose.

This proposal is intentionally compatible with an independent ImpossibleEngine editor migration to Rust + Bevy UI. Neither proposal requires the other.

## 1. Executive summary

Bevy can run headless. Its ECS, schedules, resources and plugins do not require a window, UI or renderer. Bevy's plugin model explicitly supports omitting `UiPlugin` and `RenderPlugin`, including for headless servers. Its Cargo feature collections also distinguish the core app from platform and rendering features.

Foundation should investigate Bevy as an **optional runtime/backend**, not replace its platform-neutral API or make Bevy mandatory for all consumers and targets.

The proposal has two independent integration dimensions:

1. **Headless engine services:** optional Bevy ECS/schedules/plugins for simulation and game logic, with no renderer or window.
2. **Rendering hosts:** optional Bevy 2D/3D rendering for targets where its renderer and platform stack are supported.

These dimensions can be adopted separately. A consumer may use Bevy ECS with Foundation's existing SDL3 or Canvas 2D host; another may use Bevy's own renderer; a minimal consumer may use neither.

## 2. Current Foundation boundary

Foundation currently provides:

- `foundation-runtime`: platform-neutral `RuntimeConfig`, capabilities, normalized input, frame context/output, draw commands, texture/audio handles and `RuntimeHost` contracts;
- optional native SDL3 host;
- optional browser Canvas 2D + WebAudio host for `wasm32-unknown-unknown`;
- shared PCM audio mixer and Web export/packaging support.

The consumer owns gameplay rules and source-specific asset interpretation. Foundation owns platform lifecycle and concrete host services. Existing hosts must remain valid and usable without Bevy.

## 3. Bevy capabilities relevant to Foundation

### 3.1 Headless ECS/runtime

A minimal Bevy App can register only the application/ECS/scheduling and selected project plugins. It need not add windowing, UI or rendering plugins.

Potential uses:

- entity/component simulation;
- behavior and gameplay systems;
- ordered schedules and fixed-step simulation;
- plugin-based service composition;
- deterministic or controlled-time tests;
- server, CLI, replay, validation and background processing.

A headless profile should not initialize a GPU, window, display server, audio device or input backend unless explicitly requested.

### 3.2 2D and 3D rendering

Bevy provides APIs for scenes, cameras, sprites, meshes, materials, lights, animation and UI. Its built-in renderer is an optional backend. Bevy's rendering API has also been separated into smaller crates/features, allowing API-level scene data to be used with custom renderers in some configurations.

This creates two distinct decisions:

- adopt Bevy ECS and scene APIs without adopting its renderer;
- adopt Bevy's built-in renderer for selected 2D/3D targets.

Do not assume that scene API availability means the built-in renderer, shaders or every platform backend are available on a target.

### 3.3 Feature profiles

Use `default-features = false` and deliberately selected Cargo features/collections where practical. Exact feature names must be verified against the pinned Bevy release during implementation.

Candidate profiles:

| Foundation profile | Intended contents |
| --- | --- |
| `foundation-bevy-headless` | Bevy app/ECS/schedules and Foundation integration, no window/UI/renderer |
| `foundation-bevy-2d` | Headless profile plus 2D API and selected rendering backend |
| `foundation-bevy-3d` | Headless profile plus 3D API and selected rendering backend |
| `foundation-bevy-ui` | Optional Bevy UI/window/input for applications that want it |

Prefer separate optional crates or feature-gated modules to forcing all Bevy dependencies into `foundation-runtime`'s default build.

## 4. Integration models

### Model A: Bevy ECS with existing Foundation hosts

Bevy owns simulation and entity/component state. A bridge translates normalized Foundation input into Bevy resources/messages and extracts renderer-neutral output or updates a consumer-owned world model. Foundation's SDL3 and Canvas 2D hosts continue to own windows, input capture, texture upload, drawing and audio.

Advantages:
- retains the current cross-platform host contract;
- headless tests and simulation are straightforward;
- Bevy is optional and can be used without its renderer;
- existing consumers do not need to migrate immediately.

Challenges:
- a mapping layer is required between Bevy entities/components and Foundation draw commands;
- avoid maintaining duplicate world state;
- establish frame/fixed-step timing and input ownership;
- Foundation's current draw-command API is 2D-oriented and is not itself a complete 3D renderer contract.

### Model B: Bevy as an optional rendering host

A separate optional Bevy adapter owns the Bevy App, window, input and renderer for selected targets. Consumers use a Foundation-owned lifecycle/capability boundary or a clearly documented Bevy-facing API, rather than opening a second SDL window.

Advantages:
- access to Bevy's 2D/3D renderer, scene, material and animation ecosystem;
- potential reduction of custom renderer maintenance;
- shared Bevy world for simulation and presentation.

Challenges:
- define ownership of window, input, audio, assets, frame loop and shutdown;
- prevent duplicate SDL/window/device callbacks when the existing SDL host is also enabled;
- align Bevy's renderer with Foundation capability reporting and packaging;
- keep this adapter optional and target-specific.

### Model C: Bevy as an application framework

A consumer may choose Bevy for the complete application loop, UI, input, ECS and rendering. Foundation remains responsible for reusable platform-independent services, packaging, target capability metadata and non-Bevy hosts.

This is a valid consumer choice, but should not become the only supported Foundation architecture.

## 5. Recommended crate boundaries

Names below are illustrative; do not create them until the prototype validates the boundary.

- `foundation-runtime`: retain the current stable, renderer-neutral contracts and existing hosts.
- `foundation-bevy` or `foundation-bevy-runtime`: optional Bevy ECS lifecycle/input/capability bridge, no mandatory renderer.
- `foundation-bevy-render`: optional Bevy built-in renderer integration for supported native/web targets.
- `foundation-bevy-ui`: optional UI/window profile if there is a real consumer requirement.

Dependency direction:

```text
Consumer / game
   |                  \
foundation-runtime    optional foundation-bevy
   |                  |
SDL3 / Canvas hosts   Bevy App + ECS
                      |
              optional Bevy renderer
```

The optional Bevy integration may depend on Foundation contracts. The core Foundation runtime must not depend on ImpossibleEngine, and the Foundation packaging crate must not require Bevy merely to package an application.

Avoid circular dependencies and avoid leaking SDL, winit, wgpu or Bevy types into the existing platform-neutral API unless a separate Bevy-specific API explicitly intends to do so.

## 6. Relationship with ImpossibleEngine

ImpossibleEngine is an authoring environment/compiler; Foundation is a reusable platform/runtime foundation. Their responsibilities remain distinct:

- ImpossibleEngine owns project format, Event Sheets, add-on catalogue, compiler and typed IR.
- Foundation owns platform-neutral host contracts, platform adapters, target capabilities, assets/audio/input services and packaging.
- Bevy may provide an optional ECS/runtime/rendering implementation.
- A Bevy-facing compiler/runtime adapter may live in ImpossibleEngine or a narrowly scoped integration crate, but Foundation must not import ImpossibleEngine project concepts.

The two repositories can pursue complementary work:
- Foundation documents and prototypes optional Bevy integration and backend boundaries.
- ImpossibleEngine documents Bevy as one candidate runtime target and separately evaluates Bevy UI for its editor.
- The editor UI choice does not dictate the runtime renderer choice.

## 7. Multi-console and legacy target constraints

Bevy is not a universal console abstraction. Its built-in renderer/windowing stack must not be assumed to support every classic console, legacy desktop OS or proprietary SDK.

Foundation's multi-target purpose requires:
- a headless ECS option independent of rendering;
- the existing SDL3 and Canvas hosts to remain independent;
- custom/native renderer adapters for targets that cannot use Bevy's renderer;
- explicit capability status and degraded-feature reporting;
- per-target dependency and toolchain validation;
- no claim of console support based solely on a desktop Bevy build.

For unsupported targets, consumers may still use Foundation contracts and a non-Bevy runtime/renderer.

## 8. Prototype and decision gates

1. **Headless spike:** add a throwaway example outside the stable runtime API. Run a Bevy App with one entity, one component, one system and a deterministic test. Confirm no window/GPU/display dependency is initialized.
2. **Foundation bridge spike:** feed a Foundation `InputSnapshot` into Bevy and extract a small set of state/output without duplicating authoritative game state.
3. **Existing-host comparison:** run the same small 2D sample through Foundation SDL3 and Canvas 2D while Bevy owns only ECS.
4. **Renderer spike:** build a separate Bevy 2D and 3D sample on supported desktop targets. Measure compile time, binary size, startup, frame performance, asset/shader workflow and packaging.
5. **Target matrix:** validate browser and each intended desktop target independently; document console/legacy limitations instead of extrapolating.
6. **API review:** decide whether the integration belongs in an optional crate, optional module or consumer-only adapter. Keep default Foundation builds Bevy-free unless a measured benefit justifies otherwise.

Each spike must be independently discardable. Do not rewrite the existing hosts before the integration demonstrates measurable value.

## 9. Non-goals

- No immediate replacement of SDL3 or Canvas 2D.
- No mandatory Bevy dependency in `foundation-runtime`.
- No claim of automatic classic-console support.
- No assumption that Bevy ECS requires Bevy rendering.
- No assumption that Bevy's renderer should own every consumer's window/audio/input lifecycle.
- No dependency from Foundation to ImpossibleEngine.
- No implementation or Cargo manifest changes in this documentation-only proposal.

## 10. References

- [Bevy: Plugins and modularity](https://bevy.org/learn/quick-start/getting-started/plugins/)
- [Bevy API and Cargo feature profiles](https://docs.rs/bevy/latest/bevy/)
- [Bevy 0.17: decoupling rendering APIs from the built-in renderer](https://bevy.org/news/bevy-0-17/)
- [Foundation runtime API](foundation-runtime-api.md)
- [Native SDL3 host](native-sdl3-host.md)
- [Canvas 2D + WebAudio host](web-canvas2d-webaudio.md)
- [ImpossibleEngine](https://github.com/danimesq/ImpossibleEngine)
