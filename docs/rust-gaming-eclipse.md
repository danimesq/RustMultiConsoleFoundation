
## 🌘 11. Consumer boundary clarified: own the host, not the game

The Foundation provides reusable host/platform mechanisms behind stable contracts. It must not absorb consumer-specific simulation or decide what a game means.

For a StreetChaves-style consumer, the boundary is:

| Concern | Owner |
|---|---|
| Window creation, event loop, physical device polling | Selected Foundation host adapter (or Bevy in Bevy-hosted mode) |
| Physical-to-logical input normalization and capability reporting | Foundation contract/adapter |
| Mapping logical actions such as jump, attack, and movement to game behavior | Consumer game |
| Fixed-step simulation, combat, RNG, collision rules and replay | Consumer game |
| Renderer-neutral presentation intent: sprites, positions, animation, effects and draw order | Consumer game |
| Actual rendering/presentation and platform resource handling | Foundation adapter, or selected engine host |
| Semantic audio requests such as “play hit sound” | Consumer game |
| Audio device/backend playback and output capability | Foundation adapter, or selected engine host |

SDL is one possible implementation of a Foundation desktop adapter, not the public API and not a dependency that should leak into consumer game-domain crates. Replacing SDL with another host must not require rewriting a game's simulation. A game retains game-specific visual/audio intent while delegating physical presentation and playback to the host.

Do not remove a consumer's existing SDL/window/input/audio code merely because the contract is documented. Migration is gated on a real adapter and a minimal compiling consumer that exercises it. Until then, existing host code remains the working integration path.

## 🌘 12. Runtime scaffold status and fixed-step ownership

The follow-up [Foundation runtime API PR #9](https://github.com/danimesq/RustMultiConsoleFoundation/pull/9) introduces an initial foundation-runtime scaffold and mock-host example. It is the first implementation step, not evidence of a production runtime or validated platform adapter.

At this stage, the scaffold does not provide a production SDL adapter, real window/event loop, physical input polling, renderer, audio backend, storage implementation, or validated desktop/Web/console support. Its frame coordinator and variable frame delta must not replace a consumer's deterministic simulation clock. In particular, StreetChaves' source-derived 28 ms logical tick remains owned by StreetChaves and must be advanced explicitly by its fixed-step policy; host frame pacing merely supplies opportunities to run that policy.

A consumer integration is ready only when the selected adapter implements the required contract, reports unavailable/degraded capabilities honestly, and passes a compiling end-to-end example plus the consumer's deterministic tests.
