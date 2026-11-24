# RustMultiConsoleFoundation

A reusable Rust game-development foundation for exploring native, web, legacy desktop, and classic-console targets. The project focuses on shared game logic with target-specific runtimes, renderers, assets, and build pipelines.

## 🌈 Documentation

- [Multi-console foundation: architecture and scope](docs/multi-console-foundation.md)
- [Multi-backend platform abstraction and target-specific SDL selection](docs/multi-backend-platform-abstraction.md)
- [Target platform and toolchain matrix](docs/target-platform-matrix.md)
- [Asset pipeline and target capability profiles](docs/asset-and-capability-pipeline.md)
- [Legacy Windows: Rust and SDL compatibility](docs/legacy-windows-sdl-rust.md)
- [Multi-console implementation roadmap](docs/multiconsole-roadmap.md)
- [Byte-perfect ROM reconstruction and C-to-Rust research](docs/byte-perfect-rom-reconstruction.md)
- [Impossible Engine compiler architecture](docs/impossible-engine-architecture.md)

## 🌈 Research status

The documents describe research findings, architectural proposals, and validation plans. They do not imply that every listed console, operating system, renderer, or SDK is already implemented or tested by this repository.

The first intended game consumers are StreetChavesRust and PointBlankRust. The Foundation is designed to remain reusable by other games and tools as well.
