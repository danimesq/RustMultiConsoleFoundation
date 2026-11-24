# 🌈 Legacy Windows: Rust and SDL compatibility

## 🌈 1. Goal

Windows 98 / 98 SE and Windows XP should be treated as explicit legacy desktop targets for games built with RustMultiConsoleFoundation.

They are not the same compatibility problem:

- **Windows 98:** the relevant official SDL path is SDL 1.2. SDL 2 removed Windows 95/98/ME support. Rust itself has no maintained official Win98 target/runtime.
- **Windows XP:** SDL 2 documents XP support, and the current SDL 3 Windows README also lists desktop Windows back to XP. Current Rust target baselines are newer than XP, so the SDL compatibility statement does not establish Rust binary compatibility.

The build machine should be a modern supported operating system. The legacy machine is a deployment/test target, not a requirement to run current rustc, rustup, Cargo, or modern development tools on Win98/XP.

## 🌈 2. SDL version matrix

| SDL line | Windows 98 / 98 SE | Windows XP | Recommendation |
|---|---|---|---|
| SDL 1.2.15 | Supported by the SDL 1.2 Win9x support line | Supported | Required candidate for Win98; deprecated legacy dependency |
| SDL 2.x | Win9x support was removed in SDL 2.0 | Supported by SDL's Windows documentation | Candidate for XP; pin and test the exact SDL build |
| SDL 3.x | No Win9x support is documented | Current Windows README says desktop Windows back to XP | Candidate for XP only; verify the exact SDL version, compiler, and dependency set |

Sources:

- [SDL 1.2 README](https://github.com/libsdl-org/SDL-1.2/blob/main/README.md) lists Windows 95/98/ME/XP support and marks the 1.2 branch deprecated, with no formal release since 2012.
- [SDL 2 installation notes](https://wiki.libsdl.org/SDL2/Installation) state that Windows 95/98/ME support was removed in SDL 2.0 and that SDL 1.2 still supports Win9x.
- [SDL 2 Windows FAQ](https://wiki.libsdl.org/SDL2/FAQWindows) documents SDL 2.0.0 support for XP and newer desktop releases.
- [SDL 3 Windows README](https://wiki.libsdl.org/SDL3/README-windows) currently states that desktop Windows versions back to XP are supported.

SDL is a C library. A Rust game would use a Rust binding or a carefully scoped FFI layer, but the binding crate, native DLL/import library, compiler ABI, and Rust runtime must each be compatible with the selected Windows version.

SDL support does not automatically make every Rust application or Rust binding compatible with that operating system.

## 🌈 3. Rust compiler and runtime reality

The official Rust platform-support policy is a moving target and must be checked when toolchains are pinned.

The Rust project announced that, beginning with Rust 1.78, the baseline for Tier 1 Windows toolchains and binaries would become Windows 10. It introduced separate Tier 3 Windows 7 targets. The current platform-support table lists the regular Windows GNU/MSVC targets with modern baselines and the dedicated Windows 7 targets separately.

Sources:

- [Rust: Updated baseline standards for Windows targets](https://blog.rust-lang.org/2024/02/26/Windows-7/)
- [Current rustc platform support](https://doc.rust-lang.org/rustc/platform-support.html)

Neither the regular current Windows targets nor the Windows 7 compatibility targets promise Windows XP or Windows 98 support.

Older Rust releases have sometimes been able to produce programs that run on older Windows versions with restrictions, but that is not equivalent to a supported target. A version pin alone does not guarantee compatibility: the standard library, compiler-generated code, linker, C runtime, dependency graph, and imported Win32 APIs all matter.

The following distinctions are mandatory:

- The Rust compiler running on the developer's workstation.
- The Rust standard library linked into the game.
- Rust code compiled as a static library or object and linked into a native host.
- A no_std program with a custom entry point and platform FFI.
- The SDL native library and its own dependencies.

A modern toolchain can cross-compile from Linux or a modern Windows host without being able to run its resulting binary on XP or Win98.

## 🌈 4. Windows XP strategy

Windows XP is the more realistic legacy Rust target.

### 🌈 Candidate stack

- x86 32-bit build profile.
- SDL 2 or SDL 3, pinned to a tested version.
- Rust toolchain pinned to a tested historical or custom configuration rather than assumed to be supported by current default targets.
- A pinned MinGW/GNU or MSVC-compatible linker/runtime combination.
- Explicit control of imported Windows APIs and native DLL dependencies.
- Optional no_std Rust modules where std introduces incompatible runtime requirements.

SDL 2 and SDL 3 currently document XP support, but Rust's current standard targets do not promise it. A compatibility spike must determine whether a selected Rust version and dependency graph can produce a stable XP executable.

Do not choose a Rust compiler version solely because it predates the Windows 10 baseline change. The baseline change does not identify a guaranteed XP-compatible release. Test the actual binary and record the exact compiler, linker, SDL version, DLLs, and import table.

### 🌈 XP acceptance test

A candidate build is not XP-compatible until it:

1. Starts on a clean Windows XP SP3 x86 environment.
2. Loads all required DLLs without missing-entry-point errors.
3. Creates a window and selects a supported display mode.
4. Reads keyboard and controller input.
5. Initializes audio and plays a test sound.
6. Runs a deterministic gameplay sample for a fixed duration.
7. Handles focus loss and window close safely.
8. Saves and reloads test data if the game needs persistent storage.
9. Produces a clean log/build manifest that identifies every dependency.

Test a real XP machine where possible in addition to a virtual machine. Record whether the result also works on XP x64 separately; do not infer x64 compatibility from x86.

## 🌈 5. Windows 98 / 98 SE strategy

Windows 98 is a substantially harder target for Rust.

### 🌈 Candidate SDL stack

- SDL 1.2.15 as the initial video/audio/input layer.
- 32-bit x86 executable and a Win9x-compatible native toolchain.
- No assumption that SDL 2 or SDL 3 can be used.
- A small C or assembly startup/compatibility shim if required.

SDL 1.2's Win9x support makes it a plausible native multimedia layer, but SDL 1.2 is deprecated. Pin the exact version and build; do not depend on a moving fork without reviewing its changes.

### 🌈 Candidate Rust integration modes

Investigate these options in increasing order of difficulty:

1. **Rust host tools only:** use modern Rust to convert assets, validate game data, generate tables, and emit C/assembly/binary resources consumed by a Win98-compatible executable.
2. **Rust static library / object integration:** compile a constrained Rust logic module and link it into a Win98-compatible C/assembly program using a tested 32-bit ABI.
3. **no_std Rust runtime:** use core-only Rust with a custom entry point, panic handler, memory setup, and Win32 API imports limited to APIs available on Win98.
4. **Custom compiler target/toolchain:** if the previous paths are not reliable, investigate a custom target specification, a compatible linker/import library, and a build-std configuration.

The no_std route is not automatically easy. It removes the standard library dependency but does not provide process startup, memory allocation, system services, thread support, panic behavior, ABI compatibility, or old-CPU code generation.

A modern i686 binary may also use CPU instructions unavailable on some Windows 98-era machines. CPU baseline and instruction selection must be recorded and tested independently from OS compatibility.

### 🌈 Win98 acceptance test

A candidate build is not Win98-compatible until it:

1. Starts on a clean Windows 98 SE environment with the declared CPU baseline.
2. Loads only APIs and DLLs available on the target system.
3. Initializes SDL 1.2 video and creates the intended display mode.
4. Reads keyboard and any explicitly supported controller input.
5. Initializes audio using a tested Win9x driver path.
6. Runs the gameplay sample without stack, heap, or timing failures.
7. Closes cleanly and does not rely on newer Windows APIs.
8. Is tested on a second configuration or physical machine where practical.

Document whether the tested result requires Windows 98 SE, service packs, unofficial updates, specific drivers, or additional redistributables.

## 🌈 6. Keep separate build profiles

Do not turn legacy compatibility into conditional compilation scattered across the entire game.

Use separate profiles, for example:

- desktop-modern: current stable Rust, modern SDL/native adapters.
- desktop-xp-x86: pinned Rust/linker/runtime configuration and SDL 2 or SDL 3.
- desktop-win98-x86: SDL 1.2.15 plus the least complex validated Rust/C/assembly integration.

The names are illustrative. A profile must pin toolchain versions and dependency artifacts rather than merely set an OS flag.

Keep the shared game core free of SDL types. The desktop adapter translates platform events into the common input model and submits the renderer's output to SDL.

## 🌈 7. Rust SDL binding policy

A binding crate may impose a newer MSRV, generate bindings that refer to unavailable APIs, or pull in native dependencies that do not exist on Win98/XP.

For each legacy profile:

- Pin the exact Rust binding crate version or use a small local FFI module.
- Pin SDL headers, DLL/import library, and build configuration.
- Audit the final executable's imported DLLs and functions.
- Avoid optional dependencies that silently raise the OS minimum.
- Prefer explicit feature flags and static build manifests.
- Preserve upstream licenses and redistribution requirements for SDL and other native libraries.

Do not claim Rust 1.78+ binaries support XP simply because SDL 3 does. Do not claim Win98 support merely because SDL 1.2 does.

## 🌈 8. Implementation order

1. Build a minimal SDL 1.2.15 C test application for Win98 SE.
2. Build the equivalent SDL 2 or SDL 3 C test application for XP.
3. Determine the last viable Rust toolchain/configuration for an XP x86 application and record all constraints.
4. Test a Rust no_std static library called from the Win98 SDL 1.2 application.
5. Only then test a standalone Rust executable on Win98.
6. Integrate the validated adapters into the Foundation and add CI artifacts/build manifests.

This isolates SDL/OS issues from Rust runtime issues and avoids debugging the entire game before knowing whether the platform foundation can launch.
