# 🌈 Reproducible builds: architecture and implementation plan

## 🌈 1. Purpose and terminology

RustMultiConsoleFoundation should make supported game builds reproducible: another developer or CI worker should be able to obtain the declared source inputs, use the declared toolchain and build environment, and produce an equivalent artifact with documented provenance.

Use these terms precisely:

- **Repeatable build:** the same environment and inputs produce the same result in repeated runs.
- **Reproducible build:** independent environments, following the same declared process and inputs, can independently produce equivalent outputs.
- **Bit-for-bit reproducible build:** independent builds produce byte-identical artifacts. This is the strongest target, but it must be demonstrated per target and artifact type.
- **Hermetic build:** the build is constrained to declared inputs and does not silently depend on undeclared host state or network resources.

Do not label a target reproducible merely because Cargo succeeds twice on one workstation. State the achieved level and evidence for each build profile.

This document is a plan. It does not claim that reproducible builds, hermetic builds, artifact signing, or binary comparison are already implemented.

## 🌈 2. Goals

The build system should:

1. Pin the Rust toolchain and all build-relevant tools.
2. Resolve Rust dependencies from committed lockfiles and verified sources.
3. Declare native libraries, SDKs, linkers, compilers, and target-specific inputs.
4. Make asset conversion deterministic.
5. Control timestamps, locale, timezone, filesystem ordering, environment variables, and other sources of nondeterminism where practical.
6. Build offline or from a controlled dependency mirror after inputs have been acquired.
7. Produce build manifests, checksums, and provenance alongside artifacts.
8. Compare independent build outputs and explain any expected differences.
9. Keep platform-specific exceptions explicit, especially for consoles, legacy Windows, Redox, WASM, and Emscripten.
10. Keep the normal game-developer workflow simple: select a documented profile and invoke the Foundation build command.

## 🌈 3. Scope and non-goals

The initial scope is reproducibility of source-based Foundation builds and generated game artifacts. It includes Rust crates, native dependencies, asset pipelines, packaging, and CI environments.

It does not promise that every vendor SDK, console toolchain, firmware image, driver, or proprietary compiler can be redistributed or made hermetic. Some targets may require locally installed, licensed SDKs. In those cases, document the exact version, acquisition procedure, license constraints, expected hashes where legally possible, and validation boundary.

Reproducibility is not the same as supply-chain security. Checksums detect unexpected changes; signatures authenticate a publisher; provenance explains how an artifact was produced. The plan should eventually support all three without conflating them.

## 🌈 4. Source and dependency pinning

### Rust toolchain

- Commit a repository-root rust-toolchain.toml when a workspace-wide toolchain is appropriate.
- Pin an exact Rust release or a reviewed nightly date. Avoid floating stable or nightly in release workflows.
- Record installed components and targets, such as rustfmt, clippy, and target standard libraries.
- Pin or document Cargo and rustup versions when their behavior affects the build.
- Treat legacy targets that require historical/custom toolchains as separate profiles with their own pinned toolchain and maintenance policy.

### Cargo dependencies

- Commit Cargo.lock for every application/workspace whose output is intended to be reproducible.
- Use --locked in CI and release builds so dependency resolution cannot silently rewrite the lockfile.
- Review dependency updates as source changes; record the resolved versions and checksums.
- Prefer crates.io packages with verified registry checksums. Pin Git dependencies to immutable commit revisions, not moving branches or tags.
- Avoid unpinned path dependencies that refer outside the repository.
- Consider cargo vendor for controlled/offline builds. If vendoring is adopted, commit or archive the vendor output and its Cargo source configuration deliberately; do not maintain a second dependency truth accidentally.
- Document build scripts (build.rs) and procedural macros as executable build inputs, not harmless metadata.

### Native dependencies and SDKs

For each target profile, declare the native compiler, linker, C/C++ toolchain, SDK, system libraries, headers, import libraries, binary utilities, and relevant environment variables.

Do not assume that a Cargo lockfile pins native dependencies. Record their exact versions and acquisition method separately. Where redistribution is permitted, prefer immutable archives or controlled mirrors with verified hashes. Where licensing prevents redistribution, provide a reproducible acquisition and verification procedure.

## 🌈 5. Build environment and isolation

Provide a versioned build environment for each supported profile. The implementation may use containers, Nix, a pinned VM image, or another maintainable mechanism; choose based on target compatibility rather than imposing one technology on every platform.

The environment specification should include:

- Base image or environment revision, pinned by immutable digest/revision.
- Host OS and architecture.
- Rust toolchain and installed components.
- Native compilers, linkers, SDKs, and system packages.
- Environment variables and build flags.
- Locale, timezone, and deterministic time policy.
- Required filesystem layout and permissions.
- Network policy and the source of downloaded dependencies.

Builds should support a two-stage workflow:

1. **Acquire:** fetch declared inputs from approved sources and verify them.
2. **Build:** run from the acquired inputs, ideally with network access disabled.

A container alone does not guarantee reproducibility. Host kernel behavior, mounted files, timestamps, CPU features, external SDKs, and network access may still leak into the output. Record and test the actual boundary.

## 🌈 6. Deterministic Rust build invocation

The exact command belongs to each target profile, but the common release path should use locked dependencies and a clean environment. A conceptual baseline is:

~~~sh
cargo build --release --locked --offline --target <TARGET>
~~~

--offline is appropriate only after every required crate and source has been acquired into the configured cache or vendor directory. It is not a substitute for pinning.

Use clean workspaces and isolated target directories for independent builds. Record relevant variables such as RUSTFLAGS, CARGO_ENCODED_RUSTFLAGS, linker selection, codegen settings, and profile overrides. Do not set flags globally without documenting their effect.

Avoid build scripts that embed the current wall-clock time, absolute workspace path, username, hostname, random values, or untracked Git state. If version metadata is embedded, derive it from an explicit source revision and declared release metadata.

## 🌈 7. Deterministic assets and generated files

The Foundation's asset pipeline is part of the build and must be reproducible too.

- Pin image/audio/font conversion tools and their versions.
- Define canonical input ordering and stable output naming.
- Normalize metadata and timestamps in generated archives.
- Fix compression parameters and tool versions where byte identity matters.
- Define color-space, pixel-format, palette, resampling, audio sample-rate, and quantization rules explicitly.
- Avoid embedding machine-specific paths or locale-dependent strings.
- Make generated manifests stable: sorted entries, normalized separators, and canonical numeric formatting.
- Keep original source assets distinguishable from generated target-specific outputs.
- Record source asset hashes and transformation parameters in a machine-readable manifest.

A target-specific conversion may legitimately produce different bytes from another target. Reproducibility means independently repeating the same target profile yields the same declared output, not that all targets share one binary asset representation.

## 🌈 8. Timestamps, archives, and binary nondeterminism

Investigate and control common sources of nondeterminism:

- File modification times and archive entry timestamps.
- File ordering and directory traversal order.
- Absolute paths in debug information and generated metadata.
- Build IDs, linker UUIDs, timestamps, and random seeds.
- Locale, timezone, and line-ending behavior.
- Parallel build race conditions and unordered collection iteration.
- CPU-specific code generation and host feature detection.
- Compression and archive tool versions.
- Generated source files and code-generation ordering.

Use SOURCE_DATE_EPOCH where supported by the relevant tools, with a documented source such as the release commit timestamp. Do not assume every compiler, linker, SDK, or packager honors it. Normalize archive metadata with pinned tooling and test the final artifacts.

For debug information, use supported path remapping where appropriate. Do not strip useful diagnostics merely to force identical bytes without recording that trade-off.

## 🌈 9. Target profiles and compatibility boundaries

Reproducibility must be defined per target profile, not as one universal command.

| Profile | Important pinned inputs | Reproducibility boundary |
|---|---|---|
| Modern Linux | Rust, linker, libc/sysroot, native libraries, environment image | Aim for independent byte-identical builds |
| Windows modern | Rust MSVC/GNU toolchain, linker, Windows SDK, runtime and native libraries | Pin the full native toolchain; test PE metadata and signing separately |
| Windows XP | Historical/pinned Rust strategy, SDL version, compiler, linker, CRT and XP-compatible SDK inputs | Separate legacy profile; do not infer from modern Windows |
| Windows 98 / 98 SE | SDL 1.2.15, legacy compiler/linker strategy, C runtime, CPU baseline and compatibility shims | Experimental until independently buildable and testable |
| Redox / Orbital | Rust target/toolchain, Redox sysroot, Orbital dependencies, linker and image/runtime inputs | Pin Redox SDK/image and test separately from SDL profiles |
| WASM | Rust target, wasm linker/tooling, WASI or browser ABI, optimization and packaging tools | Distinguish WASI, browser, and Emscripten profiles |
| Emscripten | Exact Emscripten SDK, LLVM/Clang, sysroot, ports and flags | Pin the SDK release and all ports |
| Console targets | Vendor SDK, compiler, linker, packaging/signing tools and firmware/emulator versions | Respect licensing; publish hashes/manifests where redistribution is restricted |
| UEFI / bare metal | Target toolchain, linker scripts, firmware interfaces and image tooling | Pin toolchain and image-generation pipeline |

A profile is not reproducible until its declared inputs are sufficiently pinned and independent-build evidence exists. Some proprietary or hardware-bound targets may only achieve documented repeatability within a licensed environment.

## 🌈 10. Build manifest and artifact provenance

Every release build should emit a machine-readable manifest next to its artifacts. A versioned JSON schema should eventually record:

- Repository URL and exact source commit.
- Dirty-tree status; release builds should normally reject uncommitted changes.
- Target triple and named target profile.
- Rust toolchain, Cargo version, and installed components.
- Dependency lockfile hash and vendor/source snapshot identity.
- Native compiler, linker, SDK, sysroot, and system dependency versions.
- Build command, profile, feature selection, and relevant environment variables.
- Asset source hashes and conversion tool/parameter versions.
- Build environment image digest or environment revision.
- Artifact filenames, sizes, and cryptographic checksums.
- Reproducibility test result and comparison method.
- Known nondeterministic fields or documented exclusions.

Do not place secrets, access tokens, private paths, or personal machine identifiers in public manifests.

Generate checksums using a stable algorithm such as SHA-256. Signing and attestations should be a later, separately reviewed layer, with key management and verification policy documented before release use.

## 🌈 11. Independent rebuild verification

A reproducibility test must build the same source revision independently, not merely invoke the same build twice in one mutable workspace.

Proposed verification process:

1. Check out the exact source commit in two clean workspaces.
2. Provision two independent instances of the same declared target environment.
3. Acquire and verify the same pinned inputs.
4. Build with the same target profile and declared environment.
5. Compare artifact hashes.
6. If hashes differ, compare archive contents and inspect binary differences.
7. Identify whether the difference is a real undeclared input, an expected platform-specific field, or a toolchain limitation.
8. Record the result and any exclusions in the build manifest.

Use byte-for-byte comparison as the default acceptance test. If a target cannot produce identical bytes, define a narrower structural comparison and explain exactly which fields are excluded. Never silently normalize away differences.

A useful CI layout is:

- Pull request: locked dependency check, formatting/lint/tests, build-profile smoke tests, asset-manifest stability.
- Scheduled verification: independent clean rebuilds for selected profiles and artifact comparison.
- Release: clean source checkout, pinned environment, offline build after acquisition, manifest/checksum generation, and reproducibility gate.
- Periodic maintenance: rebuild old supported profiles and report toolchain or dependency availability regressions.

## 🌈 12. CI, caching, and offline operation

CI caches are performance optimizations, not trusted sources of truth. Cache keys should include the target profile, toolchain, lockfile, environment revision, and relevant build flags. A cache miss must not change the output.

For release verification, support a clean build path that does not rely on mutable shared build artifacts. Verify downloaded archives before use. Keep dependency acquisition logs and hashes.

Where practical, test the build phase with network access disabled. If a build script attempts to download undeclared resources, fail the build and add the resource to the declared acquisition process.

## 🌈 13. Security, licensing, and supply-chain controls

- Review dependency licenses and native SDK redistribution rights.
- Pin Git dependencies to commit hashes and review source changes.
- Verify downloaded toolchains and SDK archives against trusted checksums or signatures.
- Keep signing keys and credentials outside build logs and manifests.
- Generate an SBOM in a later implementation phase, covering Rust and native dependencies.
- Treat build scripts, procedural macros, asset converters, and packaging tools as code execution in the supply chain.
- Document how to verify release checksums, signatures, and provenance once those mechanisms exist.

Reproducible output is useful for auditing, but it does not prove that source code is benign or that a compiler/toolchain is trustworthy.

## 🌈 14. Proposed implementation phases

### Phase 0 — Inventory and baseline

- Inventory current manifests, lockfiles, build scripts, asset tools, native dependencies, and CI workflows.
- Identify every target profile and its undocumented host dependencies.
- Choose one modern, well-supported target as the first reproducibility pilot.
- Capture baseline artifact hashes and investigate existing nondeterminism.

### Phase 1 — Pin the Rust workspace

- Add or verify the pinned toolchain configuration.
- Commit the applicable lockfile.
- Enforce cargo build --locked and locked dependency checks in CI.
- Pin Git dependencies and document build scripts.
- Add a clean-checkout build smoke test.

### Phase 2 — Pin the build environment

- Add a versioned environment definition for the pilot target.
- Pin native compiler/linker/system dependencies.
- Separate dependency acquisition from offline build.
- Record the environment revision and tool versions.

### Phase 3 — Deterministic asset pipeline

- Pin conversion tools and parameters.
- Add stable asset manifests and source hashes.
- Normalize generated metadata and archive ordering.
- Add tests that regenerate assets twice and compare outputs.

### Phase 4 — Independent rebuilds

- Build the pilot target in two independent clean environments.
- Compare final artifacts byte-for-byte.
- Fix nondeterminism or document a narrowly justified comparison boundary.
- Publish a reproducibility report with the artifact.

### Phase 5 — Expand target profiles

- Add modern Windows, Redox, WASM, and Emscripten profiles as their toolchains are pinned.
- Add XP and Win98 only with separately documented historical toolchain and runtime constraints.
- Add console profiles only within vendor SDK licensing and distribution boundaries.
- Track each profile's reproducibility level independently.

### Phase 6 — Release provenance and supply-chain hardening

- Emit versioned build manifests and checksums.
- Add SBOM generation.
- Define artifact signing, attestation, and verification policy.
- Make release publication depend on the declared reproducibility gates.

## 🌈 15. Acceptance criteria

The plan is successfully implemented for a target profile when:

- The exact source revision and all build-relevant inputs are declared.
- Rust and Cargo dependencies are locked and CI refuses unintended lockfile changes.
- Native tools, SDKs, and system libraries are pinned or their unavoidable external boundary is documented.
- A clean build can run from verified acquired inputs without undeclared network access.
- Asset generation is deterministic and covered by repeatability tests.
- Two independent clean builds produce byte-identical artifacts, or a documented and testable narrower comparison is approved.
- The build manifest records source, target, toolchain, dependencies, environment, asset inputs, and artifact hashes.
- The profile's reproducibility level and known limitations are published accurately.
- A developer can invoke the supported profile without manually reconstructing hidden machine-specific steps.

## 🌈 16. Developer experience

The desired user-facing workflow is intentionally small. Exact command names are implementation decisions, but the future experience should resemble:

~~~sh
# Inspect the declared profile and its pinned inputs
foundation build doctor --profile redox

# Acquire and verify required dependencies/toolchains
foundation build fetch --profile redox

# Build using the declared profile
foundation build --profile redox --release

# Verify an independent rebuild or compare supplied artifacts
foundation build verify --profile redox
~~~

These commands are proposed CLI examples, not existing commands. The Foundation should provide actionable diagnostics when a required SDK, toolchain, license acceptance, or environment image is missing. It should not make the developer discover undocumented linker flags or host packages through trial and error.

The guiding rule: **a build should be a function of declared source, declared inputs, and a declared target profile—not of whoever happened to run it.**
