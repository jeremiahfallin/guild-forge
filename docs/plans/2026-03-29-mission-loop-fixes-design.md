# Mission Loop Fixes — Design

## Problem

The mission system has three functional bugs that break the game loop, plus no reward feedback.

1. **Tick timing** — AI and combat gate on a fragile float check (`timer.0 > TICK_INTERVAL * 0.1`) that rarely evaluates true, so heroes don't move or fight.
2. **OnMission cleanup** — Heroes keep `OnMission` permanently after a mission ends, making them unavailable forever.
3. **No rewards or feedback** — Missions end silently with a log message. No gold, XP, or player-visible result.

## Design

### 1. Tick Timing Fix

Replace float-sniffing with an explicit `TickJustFired` event.

- `simulation_tick` sends `EventWriter<TickJustFired>` when it fires a tick.
- AI (`hero_ai_system`) and combat systems (`hero_combat_system`, `enemy_combat_system`, `update_room_status`, `check_mission_completion`) consume `EventReader<TickJustFired>` instead of checking `timer.0`.

### 2. OnMission Cleanup

Add `cleanup_completed_mission` on `OnExit(GameTab::MissionView)`:

- Remove `OnMission` from all heroes in the mission party.
- Despawn the mission entity.
- Remove the `ActiveDungeon` resource.

Runs regardless of outcome (complete, fail, retreat).

### 3. Toast Notification System

New module `src/ui/toast.rs`:

- `ToastEvent { title, body, kind }` where `ToastKind` is `Success | Failure | Info`.
- On event: spawn a bevy_declarative UI node anchored bottom-right, styled by kind (green/red/neutral border).
- Auto-dismiss after 5s via `ToastTimer` component.
- Multiple toasts stack vertically, newest on bottom.

### 4. Rewards

New `Gold` resource initialized to 0 on gameplay enter.

On mission **success**:
- Roll gold in template's `min..=max`, add to `Gold`.
- Award `xp_bonus` + sum of killed enemy `xp_reward` to each surviving hero.
- Level up: if `xp >= xp_to_next` then level++, reset xp, `xp_to_next *= 1.5`.
- Toast: `"Goblin Cave — Complete!" / "+45g, +80xp - 1 casualty"`

On mission **failure**:
- No gold, no XP.
- Toast: `"Goblin Cave — Failed!" / "Party wiped - no rewards"`

## Approach

Approach B: bug fixes + reusable toast module. The event-driven toast generalizes to future notifications (level up, new recruit, etc.) with minimal extra work over a one-off.
