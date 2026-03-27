<div align="center">

# 🪨🔥 STONEPYRE

![Project](https://img.shields.io/badge/Project-STONEPYRE-2e7d32?style=for-the-badge)
![Language](https://img.shields.io/badge/Language-Rust-b7410e?style=for-the-badge)
![Engine](https://img.shields.io/badge/Engine-Bevy-1f6feb?style=for-the-badge)
![Architecture](https://img.shields.io/badge/Architecture-Multi--crate-7b61ff?style=for-the-badge)
![Status](https://img.shields.io/badge/Status-Active%20Development-f59e0b?style=for-the-badge)

**A modular Rust game project with a Bevy client, shared game crates, backend services, and custom content tooling.**

</div>

---

## Overview

**Stonepyre** is structured as a workspace-driven game project rather than a single binary crate.  
At the center is a **Bevy-powered game client**, supported by separate crates for engine logic, world/domain data, content definitions, UI, a market simulation service, and a dedicated backend server.

The project also includes **asset-generation and management tooling** for sprite workflows, palette baking, and action completeness tracking.

---

## Highlights

- **Rust + Bevy** game client
- **Multi-crate workspace** for clean separation of concerns
- **Dedicated backend server** built with Axum + Tokio
- **Market simulation crate** with Postgres/SQLx integration
- **Shared world/content crates** used across client and server
- **Custom Python asset tooling** for palette baking and animation management
- **Configurable asset-root loading** for local development

---

## Workspace Layout

```text
stonepyre/
├─ game/
│  ├─ Cargo.toml
│  └─ crates/
│     ├─ stonepyre_app/
│     ├─ stonepyre_engine/
│     ├─ stonepyre_world/
│     ├─ stonepyre_content/
│     ├─ stonepyre_ui/
│     ├─ stonepyre_market/
│     └─ stonepyre_server/
├─ tools/
└─ libs/
```

---

## Core Architecture

### `stonepyre_app`
The top-level game application crate.

Responsibilities:
- boots the Bevy app
- configures windowing and asset loading
- wires together engine and UI plugins
- handles boot flow / screen transitions
- enters the in-world experience and spawns the demo world

### `stonepyre_engine`
The gameplay and runtime systems layer.

Responsibilities:
- core engine/plugin orchestration
- world spawning hooks
- gameplay logic and runtime systems
- integration with content/world definitions

### `stonepyre_world`
Shared world and domain-state structures.

Responsibilities:
- world-facing shared types
- serializable data used across systems
- common model/state definitions

### `stonepyre_content`
Shared content definitions and serialized game data.

Responsibilities:
- game data structures
- serialization support
- lightweight shared content layer for client/engine use

### `stonepyre_ui`
The Bevy UI layer.

Responsibilities:
- in-game UI
- text/UI rendering integration
- UI state that interacts with engine/content systems

### `stonepyre_market`
A backend/service crate for market simulation.

Responsibilities:
- async market simulation
- API/data-layer support
- Postgres access via SQLx
- config/state for simulation workloads

### `stonepyre_server`
The backend game server.

Responsibilities:
- HTTP server and app state
- database-backed runtime services
- simulation loop ownership / locking
- snapshot broadcasting and server-side runtime orchestration

---

## Dependency Flow

```text
stonepyre_app
 ├─ stonepyre_engine
 ├─ stonepyre_content
 └─ stonepyre_ui

stonepyre_engine
 ├─ stonepyre_world
 └─ stonepyre_content

stonepyre_ui
 ├─ stonepyre_engine
 └─ stonepyre_content

stonepyre_server
 ├─ stonepyre_market
 └─ stonepyre_world
```

This layout gives Stonepyre a nice split between:
- **client app composition**
- **shared gameplay/domain crates**
- **backend services and simulation**

---

## Tech Stack

### Game / Client
- **Rust**
- **Bevy 0.18**
- Bevy UI / text / sprite / render / winit support

### Backend / Services
- **Axum**
- **Tokio**
- **SQLx**
- **PostgreSQL**
- **Serde / Serde JSON**
- **UUID / Chrono**
- **Tracing**

### Tooling / Content Pipeline
- **Python**
- **Pygame**
- **Pillow**

---

## Running the Client

From the workspace root inside `game/`:

```bash
cd game
cargo run -p stonepyre_app
```

The app configures a **1920x1080** Bevy window titled **Stonepyre**.

### Asset Root
The client resolves assets from:

- `STONEPYRE_ASSET_ROOT` if set
- otherwise `../../assets` relative to the app crate

Example:

```bash
set STONEPYRE_ASSET_ROOT=C:\path\to\Stonepyre\assets
cargo run -p stonepyre_app
```

On macOS/Linux:

```bash
export STONEPYRE_ASSET_ROOT=/path/to/Stonepyre/assets
cargo run -p stonepyre_app
```

---

## Running the Server

From `game/`:

```bash
cd game
cargo run -p stonepyre_server
```

The server uses:
- **Axum** for routing
- **Tokio** for async runtime
- **SQLx** for Postgres connectivity
- **environment-based configuration**
- **advisory locks** to avoid duplicate market/game simulation loops

This is a strong sign that Stonepyre is being built with both **gameplay runtime concerns** and **persistent server-side systems** in mind.

---

## Running the Market Service / Simulation

Depending on how you’re using the project, the market simulation logic is available through:

```bash
cd game
cargo run -p stonepyre_market
```

Even when launched through the main server flow, the market crate is clearly treated as a first-class subsystem.

---

## Asset Tooling

Stonepyre includes a substantial Python-based asset workflow in `tools/animations.py`.

### What it does
- views animation actions in multiple directions
- bakes palette variants from greyscale source sprites
- manages action completeness by direction and frame slot
- supports grouped actions like:
  - base
  - skills
  - combat
- provides a UI for creating new action folder structures
- includes a manager mode and a viewer mode

### Why it matters
This is not “misc helper script” tooling — it suggests Stonepyre has a real internal content pipeline for:
- character sprites
- palette-swapped variants
- directional animation sets
- structured action authoring

---

## `libs/` Purpose

From the linked structure and tooling references, `libs/` appears to hold reusable non-Rust project assets and templates, including things like:

- palettes
- humanoid templates
- generated outputs
- font/template resources

The animation tooling references paths for:
- greyscale humanoid templates
- humanoid palette bundles
- generated palette outputs

So `libs/` looks like the art/content support layer that feeds the game’s broader asset pipeline.

---

## Project Direction

Stonepyre already reads like a project aiming for:

- a modular client/game architecture
- a persistent backend or online-capable runtime
- simulation-backed systems
- content-driven workflows
- structured internal tooling for art production

That combination makes it feel less like a throwaway prototype and more like the early shape of a **full game platform**.

---

## Suggested Dev Workflow

### Client-only work
```bash
cd game
cargo run -p stonepyre_app
```

### Server work
```bash
cd game
cargo run -p stonepyre_server
```

### Build the whole workspace
```bash
cd game
cargo build
```

### Check everything
```bash
cd game
cargo check --workspace
```

---

## Roadmap Ideas

A few natural README roadmap directions for this repo:

- [ ] document gameplay systems crate-by-crate
- [ ] add screenshots / GIFs of the Bevy client
- [ ] document backend endpoints and server config
- [ ] document database setup and migrations
- [ ] add content pipeline docs for palettes/actions
- [ ] add architecture diagrams for client ↔ server ↔ sim flow

---

## License

Workspace crates declare:

**MIT OR Apache-2.0**

If that is intended project-wide, you may want to add top-level license files to match.

---

## Notes

This README is based on the currently visible workspace/crate structure, the main app/server entrypoints, and the linked tooling/layout in the public repo. It should be a strong foundation README, and it can be expanded further with:

- screenshots
- gameplay feature breakdowns
- backend API docs
- database setup
- deployment notes
- contributor instructions

---

<div align="center">

**Built with Rust, Bevy, and a suspicious amount of ambition.**

</div>
