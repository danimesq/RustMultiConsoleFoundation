# foundation-runtime: initial public API

This is the first concrete consumer-facing runtime crate in RustMultiConsoleFoundation. It is backend-neutral and has no external dependencies in this first slice.

## Included API

- `RuntimeConfig`: requested logical display configuration.
- `Capabilities` and `CapabilityStatus`: explicit supported, unavailable, emulated, degraded, and unknown reporting.
- `InputSnapshot`: normalized directional, confirm/cancel, pointer, and quit state.
- `DrawCommand` and `FrameOutput`: minimal renderer-neutral frame output.
- `AudioRequest`: semantic audio request values.
- `RuntimeHost`: trait implemented by platform adapters.
- `Runtime`: frame coordinator that polls input, invokes consumer update, dispatches audio, and presents output.
- `RuntimeError`: typed error cases.

## Consumer example

See `examples/mock_host.rs`. It demonstrates one frame through a mock host without requiring a native window.

## Architecture boundaries

- Does not initialize SDL, Bevy, Orbital, browser APIs, or console SDKs.
- Does not yet implement a production renderer, audio device, storage, or host event loop.
- Adapter owns host integration. Avoid duplicate event loop/window/input ownership.
- Consumer owns game state and simulation timing. Frame delta is not a replacement for fixed-step deterministic simulation.
- Capability support is explicit; unsupported operations must not silently succeed.
- Bevy remains optional. A later Bevy adapter can bridge this API only after host-loop ownership is validated.

## Current status

Initial `0.1.0` API scaffold, not yet backed by a validated native, web, or console adapter. Before consumer integration, implement and validate one host adapter, a target matrix, feature/MSRV policy, and adapter-level tests. This API may evolve before `1.0`.

## Validate

```sh
cargo test -p foundation-runtime
cargo run -p foundation-runtime --example mock_host
cargo doc -p foundation-runtime --no-deps
```

Cargo features and optional backend dependencies should remain additive and documented. See the Cargo feature guide: https://doc.rust-lang.org/cargo/reference/features.html
