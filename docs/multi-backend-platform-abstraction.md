

## 🌈 13. StreetChavesRust as a Redox validation consumer

StreetChavesRust is a useful first 2D consumer for validating the Orbital backend. Its retro presentation requirements make it a more focused initial test than a renderer designed around modern GPU features.

The current evidence supports treating Orbital/orbclient as a **viable candidate for windowing, 2D drawing/surface presentation, and input integration**. It does not yet establish complete game-platform support or production readiness.

| Capability | Current architectural assessment | Required validation |
|---|---|---|
| Window and 2D surface presentation | Available through the Orbital client path | Create, update, present, resize, close |
| Keyboard input | Input events are part of the client integration | Normalize key state/events and test focus transitions |
| Mouse input | Pointer input is part of the client integration | Validate movement, buttons, and coordinate scaling |
| GPU-accelerated rendering | Must not be assumed from a 2D surface API | Determine whether a separate accelerated renderer is available and worthwhile |
| Audio output | Separate from orbclient | Integrate with Redox audio services, including the audiod path, and test latency/format support |
| Gamepad/controller | Not established by the current orbclient evidence | Investigate Redox input/device support and define a fallback |
| Real hardware performance | Unknown | Benchmark on declared Redox hardware/emulator configurations |

The Redox audio service is a separate platform concern. A successful Orbital window or framebuffer test must never be reported as proof that audio works. Keep audio behind the Foundation's independent AudioOutput contract and validate its Redox implementation separately.

References:

- [orbclient README](https://github.com/redox-os/orbclient)
- [Redox OS source organization](https://github.com/redox-os)
- [Redox OS book: project goals](https://github.com/redox-os/book/blob/master/src/our-goals.md)

## 🌈 14. The Foundation chooses; the game declares

The consumer-facing design goal is that a game declares its needs through Foundation-owned APIs. It should not select or import orbclient, SDL 1.2, SDL 2, SDL 3, or a platform windowing library directly.

A conceptual consumer API could look like this:

~~~rust
use rust_multi_console_foundation::prelude::*;

fn main() -> Result<()> {
    // The Foundation resolves the backend from the build target/profile.
    let mut game = Game::builder()
        .title("Street Chaves")
        .resolution(320, 240)
        .pixel_art(true)
        .audio(true)
        .input(InputConfig::GAMEPAD_AND_KEYBOARD)
        .build()?;

    while game.is_running() {
        let input = game.input();
        game.update(input);
        game.render()?;
    }

    Ok(())
}
~~~

This is **illustrative pseudocode**, not an implemented API or a promise that these exact types and method signatures will be adopted. Its purpose is to communicate the intended developer experience.

The intended dependency flow is:

~~~text
StreetChavesRust
  gameplay · assets · audio intent · input intent
                 |
                 v
RustMultiConsoleFoundation
  stable API · capability detection · asset adaptation · runtime
                 |
       target profile selects backend
          /          |           \
         v           v            v
      Redox       Windows        Linux
    orbclient      SDL 1.2/2/3   SDL / other
    + Orbital     by profile     validated adapter

  Other profiles: WASM · Emscripten · consoles · UEFI,
  only where the required contracts and capabilities exist.
~~~

The game core should express what it needs. The Foundation resolves how to provide it for the selected target. Platform-specific code belongs inside adapters or narrowly scoped platform integrations, not in every consumer project.

## 🌈 15. Minimum-friction consumer contract

The Foundation should own a stable, platform-neutral API for the services needed by games, including:

- Window/display configuration and framebuffer or frame-presentation requests.
- Normalized keyboard, pointer, and controller input.
- Audio output through an independent audio contract.
- Monotonic time, frame pacing, and timers.
- Storage and other platform services as the supported target set grows.
- Diagnostics, structured errors, and useful failure reporting.
- Asset adaptation for sprites, tiles, palettes, textures, and audio formats.

The game-facing API must not expose SDL event unions, SDL window handles, Orbital handles, or backend-specific pixel/audio structures.

### Capabilities and graceful degradation

Do not reduce the shared API to the least capable backend. A game may request advanced features while a target supports only a subset.

Represent optional features explicitly through capability queries or a build/runtime capability report. When a capability is unavailable:

1. Report it clearly and identify the affected feature.
2. Use a documented fallback when one exists and preserves acceptable behavior.
3. Otherwise return a typed, actionable unsupported-capability error.
4. Never silently claim that an unsupported operation succeeded.

The Foundation should distinguish at least between a feature that is unavailable on the target, unavailable in the selected backend/build, and temporarily unavailable at runtime.

### Backend selection is not consumer configuration

For normal use, the consumer should select target profiles and declare game requirements. The Foundation should choose the compatible backend and dependency graph at build time. Manual backend overrides may exist for diagnostics, testing, or unusual deployments, but should not be required for the standard workflow.

## 🌈 16. Alternative Redox integration research

Keep orbclient as the primary concrete Orbital reference for the initial Redox 2D path. Investigate other combinations, including winit and softbuffer, as alternatives only after verifying their actual Redox/Orbital support, event coverage, maintenance status, and dependency requirements.

Do not describe these alternatives as interchangeable or already validated on Redox merely because they are used in other Rust desktop environments. Record a small comparison spike before selecting one:

- Can it create and manage an Orbital window on the current Redox release?
- Does it provide the input events StreetChavesRust needs?
- What presentation model and pixel formats does it expose?
- Does it add a dependency or abstraction that duplicates orbclient?
- Can it coexist with the Foundation's target-specific dependency policy?
- What are the maintenance, licensing, and toolchain constraints?

The initial implementation should remain narrow: prove a Redox-target build, window/surface presentation, keyboard and pointer input, and clean shutdown. Add audio, controller input, performance claims, and advanced rendering only after independent tests pass.

## 🌈 17. Developer-experience acceptance criteria

The minimum-friction goal is met when:

- A consumer can depend on the Foundation without directly depending on SDL or orbclient.
- A consumer declares platform-independent requirements rather than choosing a windowing library.
- The target profile selects exactly one compatible platform backend.
- The same game-core code builds against multiple supported profiles without platform conditionals spreading through gameplay.
- Capability gaps are discoverable and produce documented fallbacks or actionable errors.
- Asset conversion and platform-specific packaging are handled by Foundation tooling or build profiles.
- Redox/Orbital windowing, input, audio, and performance are tracked as separate validation results.
- StreetChavesRust can serve as a real consumer test without becoming coupled to the Orbital implementation.

The guiding rule is: **write the game against the Foundation; declare the targets and capabilities; let the Foundation handle platform-specific integration.** This is an architectural objective, not a claim that the API, adapters, or target builds already exist.
