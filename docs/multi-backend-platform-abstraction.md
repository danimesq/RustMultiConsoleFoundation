# 🌈 Multi-backend platform abstraction and target-specific SDL selection

## 🌈 1. Architectural decision

RustMultiConsoleFoundation should use a **multi-backend platform abstraction** rather than binding game code directly to one SDL generation.

The game core talks to Foundation-owned platform contracts. A target-specific platform profile selects the concrete backend and its native dependencies:

- Modern desktop targets may use SDL 2 or SDL 3, according to the pinned and tested profile.
- Windows XP may use a specifically validated SDL 2 or SDL 3 build, subject to the selected Rust toolchain, ABI, runtime, and imported Windows APIs.
- Windows 98 / 98 SE uses SDL 1.2 as the relevant legacy SDL path, subject to a separately validated Win9x build and Rust integration strategy.
- Redox uses a native Orbital/Redox platform adapter rather than SDL; `orbclient` is a concrete reference for selecting an Orbital backend on Redox and SDL2 on other supported systems.
- Other console targets use their own SDK or hardware adapters. SDL is not treated as a universal console layer.

This is an architectural proposal, not a claim that these backends or legacy binaries are already implemented or validated.

## 🌈 2. Dependency direction

The dependency direction should be:

```text
                 Game Core
                     |
          Foundation Platform API
                     |
       +-------------+-------------+
       |             |             |
   SDL 1.2        SDL 2          SDL 3
   adapter        adapter        adapter
       |             |             |
   Win98 profile  XP/desktop    modern desktop
                  profiles       profiles

       Console SDK / bare-metal adapters
       remain separate platform backends.
```

The game core must not import SDL crates, SDL headers, SDL event types, SDL window objects, or SDL-specific pixel/audio structures.

The Foundation API owns the stable vocabulary used by games: input snapshots, window/display configuration, audio commands, timing, diagnostics, and presentation requests. Each backend translates between that vocabulary and its native library.

This keeps SDL migration and legacy support localized. It also prevents an SDL 1.2 limitation from becoming a limitation in the shared gameplay model.

## 🌈 3. Suggested workspace boundaries

A possible future Cargo workspace is:

| Crate | Responsibility |
|---|---|
| `game-core` | Gameplay and simulation; no SDL dependency |
| `platform-api` | Foundation-owned traits and platform-neutral data types |
| `platform-sdl12` | SDL 1.2 implementation for the validated Win98 path |
| `platform-sdl2` | SDL 2 implementation for selected desktop/XP profiles |
| `platform-sdl3` | SDL 3 implementation for selected modern/XP profiles |
| `platform-redox-orbital` | Redox display/window/event integration through Orbital APIs or a Foundation-owned wrapper around `orbclient` |
| `platform-console-*` | Independent console SDK or hardware adapters |
| `game-desktop` | Executable/bootstrap that selects and initializes one backend |

These names are illustrative. The repository should introduce crates only when an implementation slice needs them; documentation does not imply that this workspace already exists.

## 🌈 4. Keep the platform API narrower than SDL

The internal API should express game needs, not mirror any SDL version.

Candidate contracts include:

- `PlatformClock`: monotonic time and frame pacing.
- `InputSource`: normalized keyboard, controller, and pointer snapshots.
- `DisplayBackend`: dimensions, display mode, frame submission, and presentation.
- `AudioOutput`: normalized audio format and command submission.
- `PlatformDiagnostics`: logs, errors, and panic/reporting hooks.

Use Foundation-owned enums and structs for events and formats. Backend-specific handles should remain private to the adapter. Avoid exposing an SDL window pointer or SDL event union through a public game-facing trait.

The abstraction should not promise features every target cannot provide. Represent optional capabilities explicitly, and let a backend report unsupported operations rather than silently pretending they succeeded.

## 🌈 5. Select a backend by target profile

Backend selection should be explicit and reproducible. The selected target profile determines:

1. The platform adapter crate.
2. The SDL major/minor version, if SDL is used.
3. The Rust binding crate or local FFI layer.
4. The native SDL headers, library, DLL/import library, and build flags.
5. The Rust toolchain, linker, C runtime, architecture, and CPU baseline.
6. The validation environment and acceptance tests.

For Redox, the profile selects the Orbital backend and its Redox-native dependencies, not an SDL version. This is a compile-time target choice, not a runtime switch.

A conceptual feature-based build could look like:

```toml
[features]
default = ["backend-sdl2"]
backend-sdl12 = ["dep:sdl12"]
backend-sdl2 = ["dep:sdl2"]
backend-sdl3 = ["dep:sdl3"]
```

And a conceptual build invocation:

```sh
# Modern desktop profile
cargo build --release --no-default-features --features backend-sdl2

# Win98 candidate profile, only with a toolchain and binding proven to work
cargo build --release --no-default-features --features backend-sdl12
```

This is illustrative Cargo configuration, not verified crate metadata. The actual binding crate names, versions, feature graph, native linking behavior, and MSRV must be checked before implementation.

For a larger workspace, separate adapter packages and target-specific dependency declarations may be safer than enabling multiple SDL generations as features in one package. The build must never accidentally link two SDL implementations into one executable.

## 🌈 6. SDL 1.2 is a real backend, not an SDL 2 compatibility switch

SDL 1.2 and SDL 2 have materially different APIs and lifecycle models. For example, SDL 1.2 commonly uses `SDL_SetVideoMode`, while SDL 2 uses `SDL_CreateWindow` and a renderer/texture model. Their event, input, video, and audio APIs are not source-compatible.

Therefore, switching a target from SDL 2 to SDL 1.2 means compiling a different adapter implementation against SDL 1.2. It does not mean changing a runtime flag while reusing SDL 2 calls.

The SDL project's compatibility projects solve different migration directions:

- `sdl12-compat` exposes an SDL 1.2 API implemented over SDL 2.
- `sdl2-compat` exposes an SDL 2 API implemented over SDL 3.

These wrappers do not make SDL 2 or SDL 3 run on Windows 98. In particular, `sdl12-compat` still relies on SDL 2 underneath, so it is not a substitute for a native SDL 1.2 Win9x backend.

References:

- [SDL 2 Migration Guide](https://wiki.libsdl.org/SDL2/MigrationGuide)
- [SDL 3 SDL 1.2 Migration Guide](https://wiki.libsdl.org/SDL3/SDL12MigrationGuide)
- [SDL 3 source code and compatibility projects](https://wiki.libsdl.org/SDL3/SourceCode)
- [sdl12-compat project](https://github.com/libsdl-org/sdl12-compat)

## 🌈 7. Legacy Windows constraints remain separate

The abstraction solves source-level coupling. It does not solve OS/runtime compatibility by itself.

### 🌈 Windows 98 / 98 SE

- SDL 1.2.15 is the relevant official SDL line to investigate.
- SDL 2 removed Windows 95/98/ME support.
- Current Rust standard-library targets do not promise Win98 compatibility.
- A pinned historical/custom toolchain, constrained Rust static library, `no_std` integration, or C/assembly host shim may be required.
- Validate the actual executable, imported APIs, native dependencies, CPU baseline, drivers, and SDL build on Win98 SE.

### 🌈 Windows XP

- SDL 2 and current SDL 3 Windows documentation list XP support, but the exact build must be pinned and tested.
- Current Rust target baselines do not promise XP compatibility.
- Validate the Rust runtime, linker, C runtime, imported APIs, binding crate, SDL binaries, and target architecture independently.

See [Legacy Windows: Rust and SDL compatibility](legacy-windows-sdl-rust.md) for the detailed platform research and acceptance gates.

## 🌈 8. Build and test matrix

Maintain separate build profiles rather than scattering operating-system checks throughout gameplay code.

| Profile | Candidate backend | Required evidence |
|---|---|---|
| Modern desktop | SDL 2 or SDL 3 | Build, launch, input, audio, display, and shutdown tests on declared OS versions |
| Windows XP x86 | Pinned SDL 2 or SDL 3 | Clean XP SP3 x86 smoke test plus dependency/import audit |
| Windows 98 SE x86 | SDL 1.2.15 | Clean Win98 SE smoke test plus Win9x API, CPU, driver, and dependency audit |
| Redox | Orbital/Redox backend (for example, `orbclient`) | Redox-target build and window/event/input smoke tests; verify no SDL2 dependency is pulled in |
| Console target | Console-specific adapter | SDK/linker/package validation and emulator or hardware test |

Each successful build should record the target profile, backend, SDL version, Rust toolchain, binding/FFI revision, linker, native libraries, and test environment.

## 🌈 9. Implementation sequence

1. Define the minimal platform-neutral API from actual game requirements.
2. Implement one modern desktop adapter and prove that the game core has no SDL dependency.
3. Add a second SDL-generation adapter to validate that the contract is not accidentally SDL-shaped.
4. Build a minimal native SDL 1.2 C test for Win98 SE.
5. Separately validate the chosen Rust-to-SDL 1.2 integration strategy on Win98.
6. Implement the SDL 1.2 Foundation adapter only after the native and Rust integration spikes pass.
7. Keep XP validation independent, even if it uses the same platform API.
8. Add automated build checks where possible and retain real OS/emulator smoke tests for claims that CI cannot prove.

## 🌈 10. Acceptance criteria

The architecture is successful when:

- Gameplay code depends only on the Foundation platform API.
- SDL types and native handles do not leak into the game core.
- Exactly one intended backend is selected for a build profile.
- The same game-core tests run independently of the selected desktop SDL backend.
- Backend-specific limitations are reported explicitly.
- Win98, XP, and modern desktop claims are supported by separate reproducible build manifests and runtime tests.
- Console adapters remain independent of SDL and can implement the same relevant Foundation contracts without pretending to be desktop backends.

The key rule is simple: **the game chooses the Foundation contract; the target profile chooses the platform backend; the backend chooses the native SDL or console SDK implementation.**


## 🌈 11. Redox: native Orbital backend instead of SDL

Redox should be represented as its own platform backend, not as an SDL variant.

The [Redox OS `orbclient` project](https://github.com/redox-os/orbclient) is a useful architectural reference. Its source selects the system implementation at compile time:

- With `std` on `target_os = "redox"`, it compiles `sys/orbital.rs`.
- With `std` on non-Redox targets, it compiles `sys/sdl2.rs`.

See the [orbclient library entry point](https://github.com/redox-os/orbclient/blob/master/src/lib.rs). This demonstrates the same broad principle proposed here: a stable Rust-facing client API can route to a native Redox implementation or an SDL2 implementation depending on the target.

The current [orbclient Cargo.toml](https://github.com/redox-os/orbclient/blob/master/Cargo.toml) reinforces the distinction:

- `libredox` is a dependency under `cfg(target_os = "redox")`.
- `sdl2` is an optional dependency under `cfg(not(target_os = "redox"))`.
- The crate's `sdl` feature enables the SDL2 dependency on targets where that dependency exists.

For RustMultiConsoleFoundation, this suggests a dedicated `platform-redox-orbital` adapter. It may wrap `orbclient` behind Foundation-owned window, display, and input contracts, or use the same target-selection pattern with a narrower native integration.

Do not assume `orbclient` is a complete replacement for every platform service. Its documented role is the Orbital client/windowing and drawing surface; audio, storage, timing, diagnostics, and any advanced rendering needs still require explicit contracts and implementations. A 2D surface API is also not a universal 3D renderer.

### 🌈 Illustrative target-specific dependency declaration

The following is a design sketch, not a tested Foundation manifest:

```toml
[target.'cfg(target_os = "redox")'.dependencies]
orbclient = { version = "0.4.7", default-features = false, features = ["std", "unifont"] }

[target.'cfg(not(target_os = "redox"))'.dependencies]
sdl2 = { version = "0.38", optional = true }
```

The actual feature graph should be designed at workspace level. In particular, do not let a default SDL feature accidentally become a required Redox dependency. Prefer explicit target profiles and verify the resolved dependency graph with Cargo for each target.

### 🌈 Redox acceptance checks

1. Build for the actual Redox target with the intended Rust toolchain.
2. Confirm the resolved dependency graph uses the Orbital/Redox path and does not compile or link SDL2.
3. Open a window/surface and verify display dimensions and presentation.
4. Read and normalize supported input events through the Foundation API.
5. Verify focus, close, resize, and failure behavior as applicable to the Redox backend.
6. Test audio and other services independently; do not infer their availability from successful window creation.

## 🌈 12. SDL bundled builds: dependency packaging, not a platform backend

The `orbclient` `bundled` feature is a useful example of separating native dependency acquisition from platform selection.

According to its [README](https://github.com/redox-os/orbclient/blob/master/README.md) and [Cargo feature definitions](https://github.com/redox-os/orbclient/blob/master/Cargo.toml), `bundled` enables SDL2's `bundled` and `static-link` features. SDL2 is built from source as part of the crate build, so a preinstalled SDL2 development package is not required on the build machine.

This does **not** mean:

- SDL2 is transformed into a Redox-native backend.
- SDL2 becomes compatible with Windows 98.
- A Rust binding's MSRV or native compiler requirements disappear.
- No build tools are needed. Building bundled SDL2 still requires a compatible native build environment and toolchain.
- Every SDL2 platform dependency is automatically included or licensed for redistribution without review.

For the Foundation, model this as an orthogonal build option for SDL-based profiles:

- **Backend selection:** SDL 1.2, SDL 2, SDL 3, Orbital/Redox, or a console-specific adapter.
- **SDL acquisition/link mode:** system SDL, bundled source build, or another explicitly validated native artifact strategy.

A Redox profile should select Orbital and should not enable SDL2 merely to obtain a bundled library. A Linux/Windows/macOS SDL2 profile may choose bundled SDL2 when its toolchain and licensing requirements are satisfied.

This distinction keeps the platform architecture honest: bundling changes where a dependency comes from and how it is linked; it does not change which platform API or backend the game is using.
