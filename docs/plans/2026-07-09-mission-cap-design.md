# TI-5 · Concurrent-Mission Cap — Design

**Date:** 2026-07-09
**Chunk:** TI-5 (steam-release-chunks.md) — introduce the cap (~3 for the demo; dispatch is unbounded today), surface it in party select/mission board, leave a hook for War-Room-tier growth.

## Decisions

- **Gating building:** a new **War Room** building (user-approved). This is the scale doc's concept made real (`docs/plans/2026-04-21-scale-and-automation-design.md` §6) — the raw concurrent-mission ceiling, upgraded with gold + materials.
- **Cap formula:** `mission_cap() = 3 + level(WarRoom)`. Demo max level 3, so the cap grows 3 → 6.
- **Rescues count against the cap.** No exemption; revisit if playtests say otherwise.

## Architecture

**Cap source** — `BuildingType::WarRoom` added to the enum and `buildings.ron` (max level 3, ops-tier costs between Barracks and Armory). `GuildBuildings::mission_cap() -> u32` mirrors `roster_cap()`. Save-compatible: saves store `HashMap<BuildingType, u32>`; missing key → `level()` returns 0 → cap 3. The guild screen iterates `BuildingType::ALL`, so the War Room appears there with no screen changes.

**Enforcement** — the authoritative check lives in `dispatch_mission` (`src/screens/party_select.rs`): count live `Mission` entities with `MissionProgress::InProgress`; refuse dispatch at cap with a `warn!` and an error toast ("War Room at capacity (3/3)"). Mission entities despawn on completion/failure, so live count *is* the active count. UI states are advisory; the handler is the gate, so stale UI can never over-dispatch.

**Surfacing**
- *Party select:* at cap, the dispatch button renders in the existing disabled style, reading "War Room full (3/3)". The screen rebuilds when the active-mission count changes, so capacity freeing up while the screen is open re-enables the button.
- *Mission board:* header counter "Underway: 2/3", amber/red at cap. Browsing and party prep stay available; only dispatch is gated.

## Testing

TDD throughout. Unit tests: `mission_cap()` at levels 0/2; active-mission counting (ignores Complete/Failed, counts InProgress); dispatch refusal at cap via `run_system_once` (mission count unchanged, no `OnMission` inserted, error toast fired); dispatch success below cap.

## Out of scope

Dispatcher/auto-management (scale doc §6.1's second cap), cap growth past level 3, rescue exemptions.
