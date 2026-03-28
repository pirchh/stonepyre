# 🪨 Stonepyre

Stonepyre is a personal game project built in Rust. This repo is where
I'm working through engine systems, experimenting with structure, and
figuring things out as I go.

It's not a framework, not a template, and definitely not stable.

------------------------------------------------------------------------

## What this repo is

This is an active, evolving project. Things will change, break, get
rewritten, or disappear entirely.

The goal is simple: - build a solid foundation for a game - explore
engine architecture in Rust - keep things modular enough to not hate
myself later

Some parts are clean. Some parts are held together with duct tape.
That's intentional.

------------------------------------------------------------------------

## Structure

    game/   → main game code and crates
    libs/   → shared libraries
    tools/  → development tools and utilities

There isn't a strict long-term structure yet. I reorganize things when
it makes sense.

------------------------------------------------------------------------

## What I'm working on

This shifts pretty often, but generally:

-   core engine structure
-   ECS / game state management
-   rendering pipeline experiments
-   internal tooling to make iteration easier

------------------------------------------------------------------------

## Building

You'll need Rust (latest stable).

From the root of the repo:

``` bash
cargo build
```

To run something specific, go into the relevant crate and use:

``` bash
cargo run
```

------------------------------------------------------------------------

## Notes

-   Expect rough edges
-   Expect breaking changes
-   Expect unfinished systems

This repo is more of a workspace than a product.

------------------------------------------------------------------------

## Why this exists

I wanted a place to build something from the ground up without
over-planning it.

This is that place.
