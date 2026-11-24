# RustMultiConsoleFoundation

A foundation for exploring Rust-powered game development across classic consoles, including GBA, PS1, Nintendo DS, 3DS, and N64.

## Research directions

- [Byte-perfect ROM reconstruction and the C-to-Rust experiment](docs/byte-perfect-rom-reconstruction.md)
- [Impossible Engine compiler architecture](docs/impossible-engine-architecture.md)

The initial reverse-engineering case studies are **Pokémon FireRed/LeafGreen** and **Pokémon Emerald**. The goal is to understand the original C/assembly implementations, port their C portions to Rust, and investigate how closely the Rust implementation can reproduce the original executable bytes.

This repository is currently a research and architecture foundation. The documents describe goals and experimental methodology, not completed ports or verified byte-perfect Rust builds.
