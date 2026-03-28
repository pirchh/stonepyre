# Stonepyre

<div align="center">

# Stonepyre
### Rust game project, engine workspace, and supporting server/runtime stack

![Status](https://img.shields.io/badge/Status-Active%20Development-orange?style=for-the-badge)
![Language](https://img.shields.io/badge/Language-Rust-blue?style=for-the-badge)
![Engine](https://img.shields.io/badge/Engine-Bevy-5B8CFF?style=for-the-badge)
![Workspace](https://img.shields.io/badge/Workspace-Multi--Crate-6f42c1?style=for-the-badge)
![Architecture](https://img.shields.io/badge/Architecture-Client%20%2B%20Server-2ea44f?style=for-the-badge)
![License](https://img.shields.io/badge/License-Proprietary-red?style=for-the-badge)

</div>

Stonepyre is my game project and engine workspace built in Rust.

The repo is split into focused crates for the client, engine/runtime layer, world model, content definitions, UI, and server-side systems. It is still an active build, but the structure is deliberate and the project is moving toward a solid long-term foundation instead of turning into one oversized experimental codebase.

This is the project itself, not a starter kit, not a framework release, and not something intended to be dropped into somebody else’s game.

---

## Repository layout

```text
.
├── game/   # gameplay, engine, client, and server crates
├── libs/   # shared libraries used across the wider workspace
└── tools/  # supporting utilities and development tooling
```

The `game` workspace is the center of the project right now.

---

## What lives in `game/`

The `game` workspace is organized as a set of crates with fairly clear boundaries so systems can grow without everything collapsing into a single binary.

### `stonepyre_app`
The main game application.

This crate boots the Bevy app, configures the window and asset pathing, wires the core plugins together, and handles the transition into the playable world. In practice, this is the client entry point and the place where the engine and UI are brought together into a running game.

### `stonepyre_engine`
The gameplay and runtime layer.

This crate owns the systems that make the world actually behave like a game: input handling, interaction flow, movement, animation, world syncing, skill logic, progression, and the update pipeline once the player is in-world. It is the part of the codebase that turns data and state into moment-to-moment gameplay.

### `stonepyre_world`
World structure and spatial foundations.

This crate defines the lower-level world model: tiles, chunks, world objects, placement data, coordinate helpers, and world sources. It exists to keep spatial logic and world representation separated from higher-level gameplay systems.

### `stonepyre_content`
Game data and content definitions.

This crate holds the content-side definitions for things like items, containers, recipes, skills, atlases, manifests, and object data. The goal here is to keep game content centralized and reusable instead of scattering definitions across runtime code.

### `stonepyre_ui`
Client-facing interface systems.

This crate manages the in-game UI layer, including the HUD, inventory panels, character panels, and related UI state. It is kept separate from the gameplay runtime so the interface can evolve without being tangled directly into engine logic.

### `stonepyre_market`
Market simulation and backend domain logic.

This crate handles the market-side backend pieces: API-facing modules, configuration, database integration, simulation state, and supporting types. It is effectively the market service layer for Stonepyre rather than a client gameplay crate.

### `stonepyre_server`
Server process and live runtime services.

This crate runs the HTTP server, owns server configuration and application state, connects to Postgres, starts simulation loops, and coordinates runtime services such as market ticking and game snapshot broadcasting. It is the operational side of the project and the main entry point for server execution.

---

## Current direction

Stonepyre is being built around a few priorities:

- a clean multi-crate workspace instead of one oversized binary
- a usable split between app, engine, content, world, and UI
- backend support for persistent systems and simulation
- room to expand without needing to constantly reorganize the entire repo
- a codebase that stays practical to work in as the project gets larger

Some systems are already taking shape cleanly. Others are still in motion. That is normal for where the project is at.

---

## Runtime shape

At the moment, the runtime is broadly split like this:

- `stonepyre_app` starts the client and wires the major plugins together
- `stonepyre_engine` drives in-world behavior and gameplay systems
- `stonepyre_world` provides the spatial model and world representation
- `stonepyre_content` supplies reusable definitions and content data
- `stonepyre_ui` handles the player-facing interface layer
- `stonepyre_server` runs backend services and server-side loops
- `stonepyre_market` supports market simulation and related backend logic

That split is intentional. I want the project to stay understandable as it grows, and separating these responsibilities early makes that a lot easier.

---

## Tech stack

Current core pieces include:

- Rust
- Bevy
- Axum
- Tokio
- SQLx
- PostgreSQL
- Serde
- Reqwest

That will probably continue to change as the project grows, but that is the current base the repo is built on.

---

## Building

You will need a current Rust toolchain installed.

From the repository root:

```bash
cargo build
```

To run a specific crate, move into the relevant workspace area and run the crate you want from there.

For example:

```bash
cd game
cargo run -p stonepyre_app
```

Server-side crates will also need the expected environment configuration in place.

---

## Project status

This is an active development repository.

That means a few things up front:

- systems will change
- crate boundaries may get adjusted
- unfinished work will exist in the open
- some areas will be cleaner than others depending on when they were last touched

I am not trying to make the repo look more finished than it is. The goal is to build it properly, not pretend it is done.

---

## Notes

This repo is the place where Stonepyre gets built.

Some of it is engine work. Some of it is gameplay work. Some of it is server and simulation work. The common thread is that it all exists to support the same project, and the structure is there to keep that work manageable over time.

---

## Ownership and license

Stonepyre is proprietary.

Unless I have explicitly given written permission, you may not copy, redistribute, republish, sell, sublicense, or use this code or its assets in your own project, whether commercial or non-commercial. All rights are reserved.

If you want to discuss usage, licensing, or permission, contact me first.