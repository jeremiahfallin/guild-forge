# Event Banners (UX-3) — Design

> **Date:** 2026-07-06 · chunk UX-3 in `docs/steam-release-chunks.md`
> **Goal:** floating banners in the mission view ("BOSS ENCOUNTER", "RARE DROP!",
> "RESCUE WINDOW CLOSING") that interrupt the eye reliably and clear themselves.

## Decisions (settled 2026-07-06)

- **Architecture:** bridge from the existing UX-1 `MissionLogEvent` stream plus a
  small rescue-timer watcher. No new emit sites inside the sim.
- **Boss trigger:** first boss combat engagement — a boss-type enemy token enters
  combat-range overlap with a hero (the same overlap test
  `update_simulation_tempo` uses). One-shot per mission.
- **Rare-drop bar:** Legendary only. Epic and below stay feed-only, keeping the
  banner rare enough to stay exciting (matches CT-6's "legendary drops announced
  as events").
- **Rescue threshold:** under 30 game-seconds remaining on the Missing window
  (the window is 120s — this is the last quarter). One-shot per mission.

## Architecture

New module `src/ui/banner.rs`, registered next to the feed in `src/ui/mod.rs`.

### Data

```rust
enum BannerKind { Boss, RareDrop, RescueClosing }   // drives color/styling

struct BannerRequest { text: String, subtitle: Option<String>, kind: BannerKind }

#[derive(Resource, Default)]
struct BannerQueue {
    pending: VecDeque<BannerRequest>,
    // active banner + phase timer live here or on the spawned UI node
}
```

One-shot state lives on the mission entity as a marker component
(`BannersFired { boss: bool, rescue: bool }`) so it despawns with the mission
and never leaks across missions.

### Producers (all scoped to the viewed mission)

1. **Rare drop** — a bridge system reads `MessageReader<MissionLogEvent>` and
   enqueues on `GearDrop { rarity: Legendary, .. }` for the viewed mission:
   text "RARE DROP!", subtitle = item name.
2. **Boss encounter** — a detection system checks the viewed mission's enemy
   tokens for a boss-type enemy within combat-range overlap of any hero
   (reuse the tempo-split overlap logic). Enqueues "BOSS ENCOUNTER" once.
3. **Rescue window** — if the viewed mission has `RescueMission`, compute the
   soonest `Missing.expires_at` among `rescue_heroes`; when remaining
   game-time < 30s, enqueue "RESCUE WINDOW CLOSING" once.

### Renderer

A system in the mission-view overlay (`run_if(in_state(GameTab::MissionView))`)
shows one banner at a time: slide in top-center, hold ~2.5s, fade ~0.5s, then
pop the next request. Kind-based color: boss red, drop gold, rescue amber.
Queue clears on view change / view exit so stale banners never show.

## Error handling

- No viewed mission / mission despawned mid-banner: producers no-op; the
  renderer finishes or clears the active banner.
- Multiple simultaneous triggers: queue serializes them; nothing is dropped.

## Testing (TDD, `run_system_once`)

- Legendary `GearDrop` enqueues; Epic does not.
- Boss overlap enqueues exactly once (marker prevents refire).
- Rescue watcher fires under 30s remaining, not above, and only once.
- Queue pops sequentially as banner lifetimes expire.
- View-change clears pending queue.

Render visuals verified by hand in a watched mission (chunk's *done when*).
