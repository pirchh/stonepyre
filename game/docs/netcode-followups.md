# Netcode & server-architecture follow-ups

Deferred work from the `update/server-architecture` branch, with the concrete
**trigger** for each. That branch built client prediction/reconciliation, off-tick
persistence (#7), per-client AOI snapshots (#8), and the zone-sharding seam (#9).
The items below are deliberately *not* built — each is obviated by current work,
untestable without content that doesn't exist yet, or premature for the current
scale. They're captured here so they're ready when the trigger fires.

## UDP transport (#10)

**Status:** documented, not built.

**Why deferred.** WebSocket/TCP is in-order, so a dropped packet head-of-line (HOL)
blocks everything behind it — a movement hitch at 10Hz. UDP avoids this (drop a
snapshot, the next just arrives). But the client prediction + reconciliation
(`stonepyre_app/.../game_net/reconciliation.rs`) and remote-player interpolation
(`remote_players.rs`) already mask that jitter: the local player renders its own
prediction, and remotes render ~150 ms behind with two-sample interpolation, both
tolerant of a late/dropped snapshot. The pain that motivated UDP is largely gone.
There is also no retrofit penalty for waiting — the reliable/unreliable message
split is a small additive change whenever it's needed.

**Design when built.**
- UDP socket both ends; the Bevy client replaces its WebSocket networking.
- Split `ServerMsg`/`ClientMsg` into **unreliable** (snapshots, `MoveDir` —
  drop-tolerant, newest-wins) and **reliable** (inventory, equip, bank,
  action-state, notices — must arrive, ordered per stream).
- A reliability layer over UDP for the reliable class (sequence numbers, acks,
  retransmit, ordering). Prefer an existing crate — `renet`, `laminar`, or
  `quinn`/QUIC — over hand-rolling; hand-rolled reliability is a classic footgun.
- New handshake/auth: today the JWT rides the WS upgrade request; UDP needs an
  explicit authenticated handshake before the session opens.

**Trigger.** Movement goes twitch / real-time PvP where sub-100 ms matters, **or**
real-world packet loss is measured to cause HOL stalls the prediction can't hide.

## Cross-zone handoff + boundary AOI (#9)

**Status:** seam built, handoff stubbed (`game/zone.rs` → `ZoneManager::handoff`).

**Why deferred.** With a single zone there's no boundary to cross, and the handoff +
boundary-merge logic can't be written or tested without a second zone and
adjacent-map data. Building it blind would ship untested, probably-wrong code.

**Design when built.**
- **Handoff:** when a player crosses a zone boundary, lift their full sim entity
  (position, inventory cache, in-flight action) out of the source `GameSim`, insert
  into the destination, then `player_zone.insert(player, dst)`. ws command routing
  already reads the connection's zone, so repointing it completes the move.
- **Boundary AOI:** the per-zone snapshot loop currently sends only its own zone's
  entities. Near a boundary a player must also see the adjacent zone — merge
  neighbor-zone entities within AOI into that player's snapshot (read-only
  cross-zone query). This is the cross-zone extension of #8.

**Trigger.** A second zone exists with real adjacent-map / boundary data.

## Command-stream input replay (movement)

**Status:** input sequencing plumbed (Phase B — `MoveDir.seq`, echoed as
`last_input_seq`); per-input replay not built. The reconciler uses the lighter
trust-prediction + eased-in correction model.

**Why deferred.** The current reconciler renders the local prediction and converges
only genuine divergences (deadzone + eased-in, collision-aware ramp). Its one
limitation is a residual sub-tile divergence after adversarial input-mashing at a
collision edge, which heals smoothly rather than instantly. For OSRS/WoW-style
movement that's imperceptible; only twitch/PvP would need exact per-input replay.

**Design when built.** The client keeps a ring of unacked `(seq, input)`; on each
snapshot it snaps to the authoritative position at `last_input_seq` and
re-simulates the unacked inputs forward through the shared `slide_move` integrator.
The `seq` fields already on the wire are the foundation.

**Trigger.** Movement becomes competitive/twitch, or the residual divergence is
observed to matter in real play.

## Scaling knobs already seamed

- **Add zones (#9)** when the single-sim tick or single sim-lock is the profiled
  bottleneck under real population, or the map grows past one logical region. The
  seam (`ZoneManager`, per-zone tick/snapshot loops) is in place.
- **AOI tuning (#8)** — `AOI_TILES` in `stonepyre_server/src/main.rs`: a no-op at
  current scale (everything in range). Lower it and/or add a spatial grid when
  player + node counts per zone make the O(n²) per-tick filter or the snapshot size
  significant.
