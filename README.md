# RustMultiConsoleFoundation

A foundation for exploring Rust-powered game development across classic consoles, including GBA, PS1, Nintendo DS, 3DS, and N64.

## Research directions

- [Byte-perfect ROM reconstruction and the C-to-Rust experiment](docs/byte-perfect-rom-reconstruction.md)
- [Impossible Engine compiler architecture](docs/impossible-engine-architecture.md)

The first byte-matching experiment is **Pokémon Red/Blue**: compare the mature pret/pokered disassembly against the Rust reimplementations open-pokered and pokerust. This is followed by an ambitious shared-source investigation linking Red and FireRed. FireRed/LeafGreen and Emerald remain major GBA case studies.

This repository is currently a research and architecture foundation. The documents describe goals and experimental methodology, not completed ports or verified byte-perfect Rust builds.
