#🌈 🏓 multiPong: built-in reference game

##🌈 Status and decision

**Decision (2026-11-24):** RustMultiConsoleFoundation will ship with its own built-in Pong game, named **multiPong**. It is the Foundation's canonical reference application and conformance game.

This document records the product decision, the technical rationale, the diagnostic mode, the subsystems multiPong is intended to exercise, research into existing Rust Pong repositories, and the proposed implementation strategy.

This is an architectural decision and research plan. It does **not** mean that multiPong has already been implemented or that any target has been validated.

##🌈 Why Pong is the right reference game

Pong is unusually effective as a cross-platform and console demonstration because its rules are tiny while its platform requirements are real.

- **Small gameplay surface:** paddles, a ball, collision, score, and a small state machine. There is little game-specific complexity to obscure platform problems.
- **Broad systems coverage:** a useful implementation needs drawing, timing, input, window or display management, and optionally audio, assets, controllers, and packaging.
- **Easy to inspect:** visual glitches, timing drift, missed input, scaling errors, and collision bugs are immediately observable.
- **Portable interaction model:** keyboard, gamepad, touch, browser input, and console controls can map to the same small set of actions.
- **Low asset burden:** the baseline can use primitive geometry and a tiny set of optional sound effects. This reduces asset-pipeline noise.
- **Comparable across devices:** the same rules can be exercised on desktop, browser, embedded hardware, and console-like targets without redesigning the game.
- **Good failure signal:** when a minimal game fails, the Foundation boundary or target adapter is a more likely suspect than a sprawling game-specific subsystem.

Pong is therefore not selected because it is technically impressive by itself. It is selected because it is a small, understandable workload that exposes whether the Foundation's abstractions actually work.

###🌈 What Pong can and cannot prove

A successful Pong run is evidence that the specific exercised path works. It does not prove that every API, peripheral, performance envelope, SDK, or target feature is supported. Validation must be recorded per target and per capability.

##🌈 Role inside the Foundation

The intended relationship is:

- **RustMultiConsoleFoundation** owns the platform abstractions, target selection, capability reporting, asset adaptation, and build/packaging integration.
- **multiPong** ships in the Foundation repository as its built-in reference game and exercises those public abstractions.
- **StreetChavesRust** and **PointBlankRust** remain independent consumer games. They should not need to depend on multiPong to use the Foundation.

The governing principle remains:

> The Foundation chooses; the game declares.

multiPong should declare what it needs through the public Foundation API. It must not select SDL, Orbital, or another platform backend directly from gameplay code. Backend choice belongs to the Foundation's target profile and adapter layer.

##🌈 Diagnostic mode: the Foundation's live status panel

multiPong must include an optional diagnostic mode, accessible through a documented key/button combination and, where possible, a startup option.

The diagnostic overlay should display information reported by the Foundation, not guesses made by the game.

###🌈 Required diagnostic fields

| Field | Meaning |
|---|---|
| Target / platform | Compiled target and runtime platform identity, where known |
| Active backend | Actual selected windowing/rendering/input backend, not merely a requested preference |
| Renderer / presentation path | Renderer or framebuffer/presentation adapter in use |
| Resolution | Logical game resolution and physical drawable/display resolution when both exist |
| Scaling | Active scaling mode, viewport, and letterbox/pillarbox state |
| FPS | Measured frames per second |
| Frame time | Current and rolling frame-time measurements; optionally min/average/max |
| Input | Detected input sources and recently observed device classes |
| Audio | Whether audio is available, selected audio backend, and whether output initialization succeeded |
| Capabilities | Supported, unavailable, emulated, degraded, or unknown capability states |
| Runtime warnings | Non-fatal adapter warnings and fallback decisions |
| Build identity | Build/profile identifier and optional reproducible-build metadata when available |

The overlay should distinguish **available**, **unavailable**, **emulated/fallback**, **degraded**, and **unknown**. It must not report a capability as supported merely because an API call exists.

###🌈 Diagnostic behavior

- Diagnostics must be usable without disrupting gameplay.
- A minimal overlay should work even when optional audio or advanced rendering is unavailable.
- The game must not crash just because a diagnostic field is unknown.
- A text/log export or copyable diagnostic report is desirable for bug reports.
- The overlay should show the reason for a fallback when the Foundation exposes one.
- Diagnostic reporting itself must not bypass the Foundation abstraction to inspect backend internals.
- Production builds may keep the overlay disabled by default, but it must remain available through a documented switch.

##🌈 Systems exercised by multiPong

| Foundation subsystem | multiPong validation |
|---|---|
| Rendering | Primitive shapes, text, clear/present, resize, viewport and scaling |
| Input | Keyboard, gamepad/controller, pointer/touch where supported, focus loss and reconnect |
| Audio | Short sound effects, initialization failure, unavailable-device behavior, independent audio backend |
| Timing | Fixed or controlled simulation step, frame pacing, pause/resume, delta-time handling |
| Window/display | Window creation, fullscreen/windowed mode, display size and orientation where relevant |
| Assets | Optional font/sound assets, target variants, missing-asset fallback |
| Capabilities | Honest reporting of supported and unavailable features |
| Lifecycle | Startup, shutdown, focus changes, device loss and recovery |
| Packaging | Build outputs and packages for supported targets |
| Reproducible builds | Stable assets/configuration and repeatable build inputs |
| WASM | Browser startup, canvas sizing, browser input and audio policy |
| Redox/Orbital | Native Orbital path and keyboard/pointer behavior, with audio tested independently |
| Legacy targets | Only where the toolchain/runtime path is explicitly supported; no compatibility claims by association |

##🌈 Gameplay and architecture boundaries

The core simulation should be independent from platform APIs. It should be possible to test ball movement, paddle movement, collision, scoring, match state, and deterministic reset behavior without creating a window or audio device.

Suggested conceptual layers:

1. **multiPong core:** rules, state, collision, scoring, deterministic simulation.
2. **Foundation-facing game adapter:** translates input actions into game commands and submits render/audio intents through public Foundation APIs.
3. **Diagnostics model:** consumes structured Foundation-reported platform/capability data and game performance counters.
4. **Presentation:** draws the game and optional diagnostic overlay using the selected Foundation rendering path.

Avoid coupling gameplay logic to a specific ECS or engine. A small game does not need a heavyweight engine merely to move a rectangle and a ball. Existing projects can inform the implementation, but multiPong must fit the Foundation's architecture rather than importing another project's platform assumptions.

##🌈 Conformance criteria

A target is not considered validated merely because the crate compiles.

For each target, record:
- target triple and toolchain;
- backend and renderer actually selected;
- build and packaging result;
- startup and clean shutdown;
- rendering and scaling result;
- keyboard/controller/pointer paths applicable to the target;
- audio initialization and playback result, or explicit unavailability;
- frame-time/FPS observations;
- diagnostic overlay output;
- known limitations and evidence (logs, screenshots, CI run, device test).

A target may be marked **compiled**, **launch-tested**, **gameplay-tested**, or **fully validated**. These statuses must not be conflated.

##🌈 Research: existing Rust Pong repositories

###🌈 Scope and caveats

This is a research snapshot, not a claim that every Rust Pong repository on GitHub has been found. GitHub repository search for `pong language:Rust`, code search, framework project lists, and direct repository inspection surfaced the projects below. Search ranking and indexing are incomplete; repositories may be renamed, deleted, archived, or poorly tagged. Some results containing “ping-pong” are networking demonstrations rather than the arcade game and are excluded from the gameplay shortlist.

The links below point to source repositories. Their inclusion is a research reference, not an endorsement, license grant, or decision to copy code. Before reusing any code or assets, inspect the exact revision, license, dependency versions, asset provenance, and attribution requirements.

###🌈 Strong candidates to study

| Repository | Stack / focus | Why inspect it | Caution |
|---|---|---|---|
| [bzar/wasm-pong-rs](https://github.com/bzar/wasm-pong-rs) | Rust + wasm-bindgen + web-sys + WebGL/canvas/browser APIs | A direct browser/WASM Pong; useful for browser lifecycle, canvas, WebGL and browser audio/input integration | Browser-specific implementation is not the cross-platform architecture we want |
| [yotam5/pong_ggez](https://github.com/yotam5/pong_ggez) | Rust + ggez | Organized game modules for ball, paddle, entities and game state; useful small-game structure | ggez-specific event and rendering model |
| [j0of/pong-macroquad-hecs](https://github.com/j0of/pong-macroquad-hecs) | Rust + Macroquad + hecs | Small 2-player implementation; useful comparison of immediate-mode drawing and lightweight ECS | Macroquad/hecs are implementation choices, not Foundation requirements |
| [0xAlcibiades/rusty-pong](https://github.com/0xAlcibiades/rusty-pong) | Rust + Bevy 0.15 + Rapier2D | More complete gameplay loop: AI, scoring/deuce, pause, audio, game states and physics | Heavier engine/physics stack than multiPong needs; study boundaries, not dependency weight |
| [FSMaxB/bevy-pong-clone](https://github.com/FSMaxB/bevy-pong-clone) | Rust + Bevy | Componentized ball, paddle, goal, wall and score modules | Bevy-specific architecture |
| [SuperiorJT/bevy_pong](https://github.com/SuperiorJT/bevy_pong) | Rust + Bevy | A simple Bevy implementation with a browser demo noted in related search results | Verify current build instructions and target status |
| [TanTanDev/rusty_pong](https://github.com/TanTanDev/rusty_pong) | Rust Pong tutorial project | A compact baseline worth inspecting for minimal implementation choices | Tutorial-oriented; assess code quality and dependencies |
| [coeuvre/rust-pong](https://github.com/coeuvre/rust-pong) | Rust desktop Pong | Separate modules for AABB, AI, app, ball and player; useful for gameplay decomposition | Older stack and dependency/API age need review |
| [Vaporjawn/pong](https://github.com/Vaporjawn/pong) | Rust + Macroquad, modern feature-rich implementation | Search result describes clean architecture, tests and CI; potentially useful for testing conventions | Inspect exact repository revision and portability before adopting |
| [amethyst/pong_wasm](https://github.com/amethyst/pong_wasm) | Amethyst + renderer/audio; native and attempted WASM | Historical example of a game targeting multiple graphics backends and attempting WASM | README explicitly reports a broken/incomplete WASM state and nightly requirement; historical research only |

###🌈 Specialized ports worth examining

| Repository | Target / approach | Research value |
|---|---|---|
| [marikka/micropong](https://github.com/marikka/micropong) | Embedded / microcontroller-oriented Rust | Constrained hardware, memory layout and no-desktop assumptions |
| [hiroshimorowaka/esp32-pong-game](https://github.com/hiroshimorowaka/esp32-pong-game) | ESP32 | Embedded display/input and target-specific runtime constraints |
| [mov-rax/psp-pong](https://github.com/mov-rax/psp-pong) | PlayStation Portable | Particularly relevant console-style target research; inspect SDK/toolchain and license |
| [Noah2610/pong-cli](https://github.com/Noah2610/pong-cli) | Terminal / CLI Pong | Useful alternate rendering model and testable game-state separation |
| [you-win/godot-pong-rust](https://github.com/you-win/godot-pong-rust) | Godot + Rust GDNative | Useful example of Rust game logic hosted by an external engine; not a direct Foundation backend candidate |

###🌈 Additional search-discovered repositories

These were surfaced by repository search as Rust Pong or Pong-named game projects. They are leads for further triage, not all individually audited or recommended:

- [trytryty1/PongButRust](https://github.com/trytryty1/PongButRust) — ggez Pong.
- [Plonq/bevy-pong](https://github.com/Plonq/bevy-pong) — Bevy Pong with a published WASM demo.
- [IdoKendo/good-game-rusty-pong](https://github.com/IdoKendo/good-game-rusty-pong) — Macroquad + GGRS rollback-netcode multiplayer Pong.
- [yotam5/pong_ggez](https://github.com/yotam5/pong_ggez) — also listed above as a primary candidate.
- [PhaestusFox/ZeroToPong](https://github.com/PhaestusFox/ZeroToPong) — Pong learning project.
- [devcherchecollegue-org/rust-pong](https://github.com/devcherchecollegue-org/rust-pong).
- [AllanHasegawa/rust-pong](https://github.com/AllanHasegawa/rust-pong).
- [zokier/pong-rs](https://github.com/zokier/pong-rs).
- [NJHTR/pong](https://github.com/NJHTR/pong).
- [TrisH0x2A/pong.rs](https://github.com/TrisH0x2A/pong.rs).
- [Yasin13852007/PONG-game](https://github.com/Yasin13852007/PONG-game).
- [richardanaya/pong](https://github.com/richardanaya/pong).
- [HectorPulido/rust-pong](https://github.com/HectorPulido/rust-pong).
- [harshvishu/pong](https://github.com/harshvishu/pong).
- [acmpesuecc/Pong](https://github.com/acmpesuecc/Pong).
- [Herschel/rusty-pong](https://github.com/Herschel/rusty-pong).
- [marikka/micropong](https://github.com/marikka/micropong) — also listed above as a specialized target.
- [ipeternella/pong](https://github.com/ipeternella/pong).
- [StephanDollberg/PingPong](https://github.com/StephanDollberg/PingPong).
- [IshmamR/terminal.pong](https://github.com/IshmamR/terminal.pong).
- [tensor-programming/rust_pong_game](https://github.com/tensor-programming/rust_pong_game).
- [nolantait/pong-tutorial](https://github.com/nolantait/pong-tutorial).
- [FSMaxB/bevy-pong-clone](https://github.com/FSMaxB/bevy-pong-clone) — also listed above as a primary candidate.
- [SuperiorJT/bevy_pong](https://github.com/SuperiorJT/bevy_pong) — also listed above as a primary candidate.
- [TanTanDev/rusty_pong](https://github.com/TanTanDev/rusty_pong) — also listed above as a primary candidate.
- [coeuvre/rust-pong](https://github.com/coeuvre/rust-pong) — also listed above as a primary candidate.
- [bzar/wasm-pong-rs](https://github.com/bzar/wasm-pong-rs) — also listed above as a primary candidate.
- [amethyst/pong_wasm](https://github.com/amethyst/pong_wasm) — also listed above as a primary candidate.
- [you-win/godot-pong-rust](https://github.com/you-win/godot-pong-rust) — also listed above as a specialized example.

Repository search also surfaced names such as `multiversx/mx-ping-pong-sc`, `0xprames/tonic-ping-pong`, and `comit-network/ping-pong`; these are not presumed to be arcade Pong games and are not part of the gameplay shortlist.

###🌈 Framework-maintained reference

The [ggez Projects list](https://github.com/ggez/ggez/blob/master/docs/Projects.md) explicitly lists [yotam5/pong_ggez](https://github.com/yotam5/pong_ggez) and [rust-game-ports' Boing/ggez port](https://github.com/rust-gamedev/rust-game-ports/tree/master/boing-ggez). The [rust-game-ports repository](https://github.com/rust-gamedev/rust-game-ports) is an educational collection of Rust game-library ports and includes Boing/ggez among its examples. It was archived in March 2026, so treat it as a historical reference rather than an actively maintained upstream.

##🌈 Initial recommendation: study multiple projects, do not adopt an engine

The first inspection pass should prioritize:

1. **yotam5/pong_ggez** for a modular small-game structure and state separation.
2. **j0of/pong-macroquad-hecs** for a compact immediate-mode 2D implementation.
3. **bzar/wasm-pong-rs** for browser/WASM-specific constraints and integration.
4. **0xAlcibiades/rusty-pong** for a richer gameplay loop, audio, pause and testable state boundaries.
5. **marikka/micropong**, **esp32-pong-game**, and **psp-pong** for constrained/console-style platform research.

No one repository should dictate multiPong's architecture. The likely best result is an original Foundation-native implementation that borrows only well-understood, license-compatible ideas and keeps its simulation independent of the presentation backend.

##🌈 License and provenance checklist

Before copying code, assets, tests, or substantial structure from any project:

- inspect the repository's license file at the exact commit being studied;
- identify licenses for fonts, music, sound effects, sprites, and other bundled assets separately;
- preserve copyright and attribution notices where required;
- check whether dependencies impose additional distribution obligations;
- do not assume a public repository or a visible license badge grants unrestricted reuse;
- prefer learning from architecture and independently implementing the same simple mechanics when license or provenance is unclear.

##🌈 Proposed implementation sequence

1. Inspect the primary candidates' source files, dependency graphs, licenses, and test coverage.
2. Write a short comparison record identifying ideas accepted, rejected, and why.
3. Define a small platform-neutral multiPong simulation API and deterministic unit tests.
4. Connect it to the Foundation's public rendering, input, timing, audio, lifecycle, and capability interfaces.
5. Add the diagnostic overlay using Foundation-reported runtime information.
6. Run native desktop first, then WASM and Redox/Orbital when their adapters are ready.
7. Add specialized console/embedded targets only when their toolchains and runtime adapters are independently validated.
8. Publish a per-target conformance report; never infer a successful port from compilation alone.

##🌈 References

- [Rust Game Ports: educational collection of game-library ports](https://github.com/rust-gamedev/rust-game-ports)
- [ggez Projects: framework-maintained project list](https://github.com/ggez/ggez/blob/master/docs/Projects.md)
- [wasm-bindgen](https://github.com/rustwasm/wasm-bindgen)
- [Macroquad](https://github.com/not-fl3/macroquad)
- [Bevy](https://github.com/bevyengine/bevy)
- [ggez](https://github.com/ggez/ggez)
