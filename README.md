# Stonepyre

![Status](https://img.shields.io/badge/status-in%20active%20development-orange)
![Language](https://img.shields.io/badge/language-Rust-blue)
![Workspace](https://img.shields.io/badge/workspace-multi--crate-6f42c1)
![License](https://img.shields.io/badge/license-MIT%20%7C%20Apache--2.0-green)

Stonepyre is my game project and engine workspace built in Rust.

The repo is organized around a few clear pieces: the playable client, the engine/runtime layer, shared game data, UI, world foundations, and server-side systems that support simulation and persistence. It is still very much an active build, but the structure is intentional and the code is moving toward a real long-term foundation rather than a pile of disconnected experiments.

This is not a framework, starter kit, or polished engine release. It is the working project itself.

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

The `game` workspace is split into focused crates so systems can evolve without everything collapsing into one giant binary.

### `stonepyre_app`
The main game application.

This crate boots the Bevy app, configures the window and asset pathing, wires the core plugins together, and handles the transition into the playable world. In practice, this is the entry point for the client and the place where the engine and UI are brought together into a running game.

### `stonepyre_engine`
The gameplay/runtime layer.

This crate owns the systems that make the world actually behave like a game: input handling, interaction flow, movement, animation, world syncing, skill logic, progression, and the general update pipeline once the player is in-world. It is the part of the codebase that turns data and state into moment-to-moment gameplay.

### `stonepyre_world`
World structure and spatial foundations.

This crate defines the lower-level world model: tiles, chunks, world objects, placement data, coordinate helpers, and world sources. It exists to keep spatial logic and world representation cleanly separated from higher-level gameplay systems.

### `stonepyre_content`
Game data and content definitions.

This crate holds the content-side definitions for things like items, containers, recipes, skills, atlases, manifests, and object data. The goal here is to keep game content centralized and reusable instead of scattering definitions across runtime code.

### `stonepyre_ui`
Client-facing interface systems.

This crate manages the in-game UI layer, including the HUD, inventory panels, character panels, and related UI state. It is built to stay separate from the gameplay runtime so the interface can be iterated on without tangling it directly into engine logic.

### `stonepyre_market`
Market simulation and backend domain logic.

This crate handles the market-side backend pieces: API-facing modules, configuration, database integration, simulation state, and supporting types. It is effectively the market service layer for Stonepyre rather than a client gameplay crate.

### `stonepyre_server`
Server process and live runtime services.

This crate runs the HTTP server, owns server configuration and application state, connects to Postgres, starts simulation loops, and coordinates runtime services such as market ticking and game snapshot broadcasting. It is the operational side of the project and the main entry point for server execution.

---

## Current direction

At a high level, Stonepyre is being built around a few priorities:

- a clean multi-crate game workspace instead of a single oversized binary
- a usable client/runtime split between app, engine, content, world, and UI
- backend support for persistent systems and simulation
- room to keep expanding without rewriting the repo structure every other week

That means some parts are further along than others. Some systems are already clearly separated. Others are still being reshaped as the project grows.

---

## Building

You will need a recent stable Rust toolchain.

From the repository root:

```bash
cargo build
```

To work specifically in the game workspace:

```bash
cd game
cargo build
```

To run the client application:

```bash
cd game
cargo run -p stonepyre_app
```

To run the server:

```bash
cd game
cargo run -p stonepyre_server
```

Some server-side crates expect environment configuration for things like database access and service settings.

---

## Project status

Stonepyre is under active development.

That means a few things are normal here:

- systems will continue to move between crates when it improves the structure
- APIs and internal boundaries may change
- some features are represented by foundations first and polish later
- parts of the repo are farther along than others

I care more about getting the structure right than pretending everything is finished.

---

## Why this repo is organized this way

I wanted the project to stay practical as it grows.

A lot of game repos start clean and then slowly turn into one crate doing everything. I have been trying very deliberately not to let that happen here. The split between app, engine, world, content, UI, market, and server is there so each part can carry its own responsibility and be rewritten or expanded without dragging the entire project down with it.

That separation also makes it easier to test ideas, replace weak spots, and keep long-term work from turning into cleanup work.

---

## Notes

This repo is a working project, not a finished product.

Expect rough edges, incomplete systems, and occasional rewrites. But the direction is real, the structure is deliberate, and every crate here exists for a reason.

