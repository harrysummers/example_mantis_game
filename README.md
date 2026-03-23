# Example Mantis Game

A minimal example game built with the [Mantis](../mantis) game engine. Launches a fullscreen window displaying "Hello World" centered on screen.

## Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain, targeting `aarch64-apple-darwin` for Apple Silicon Macs)
- The `mantis` crate cloned alongside this project (expected at `../mantis`)

Install Rust if you haven't already:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

## Local Development Setup

Clone both repositories into the same parent directory:

```sh
mkdir game_engine && cd game_engine
git clone <your-mantis-repo-url>
git clone <your-example-mantis-game-repo-url>
```

Your directory structure should look like:

```
game_engine/
├── mantis/
└── example_mantis_game/
```

## Running

```sh
cd example_mantis_game
cargo run
```

This will open a fullscreen borderless window with "Hello World" displayed in the center. Press `Escape` to exit.

## Building a Release Binary

```sh
cargo build --release
```

The compiled binary will be at `target/release/example_mantis_game`.
