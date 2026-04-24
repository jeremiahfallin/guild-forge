# Missing & Injured Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace permadeath on mission-wipe with a timed `Missing` state that transitions to a temporary `Injured` stat-penalty debuff, shown on roster cards with countdowns and announced via toasts.

**Architecture:** Two zero-cost marker-ish components — `Missing { expires_at }` and `Injured { expires_at }` — stored in game-time seconds (`Time<Virtual>::elapsed_secs_f64()` which already reflects `GameSpeed` scaling + offline time bank). On mission failure, `remove::<OnMission>()` stays but is now joined by `insert(Missing { expires_at: now + MISSING_DURATION })`. A tick system checks `Time<Virtual>`, removes `Missing` → inserts `Injured` and fires a return toast, and later removes `Injured` when its timer lapses. Combat stat calculation at `src/mission/entities.rs:125-128` multiplies STR/DEX/CON by 0.75 when the hero has `Injured`. Dispatch filter widens from `Without<OnMission>` to `Without<OnMission>, Without<Missing>`. Roster card replaces the "On Mission" status line with a priority-ordered banner (Missing > Injured > OnMission > idle) and formats a `m:ss` countdown. Save DTO gets two `Option<f64>` fields persisting *remaining* seconds (not absolute timestamps, because `Time<Virtual>::elapsed_secs_f64()` resets per run).

**Tech Stack:** Bevy 0.18 ECS, `Time<Virtual>`, observers, bevy_declarative UI, RON save format, `#[serde(default)]` for forward-compat.

**Design constants (final):**
- `MISSING_DURATION_SECS: f64 = 120.0` (game-seconds)
- `INJURED_DURATION_SECS: f64 = 300.0` (game-seconds)
- `INJURED_STAT_MULTIPLIER: f32 = 0.75` — applies to STR / DEX / CON only
- No permadeath in this slice. Combat already halts token movement at 0 HP; mission wipe still ends the run at `src/mission/combat.rs:258-272` but now returns heroes as Missing instead of dead.
- No XP reward change (failed missions still grant nothing).
- Favorite integration: failure toast that contains any favorited hero uses the hero's name in the title instead of the mission name; return toast for a favorited hero uses `ToastKind::Success` (regular return uses `ToastKind::Info`).

---

### Task 1: `Missing` and `Injured` components + time helper

**Files:**
- Modify: `src/hero/mod.rs` (add components below `PersonallyManaged` at ~line 39; register in `plugin` at ~line 11)
- Create: `src/hero/status.rs` (pure formatter helpers + constants + tests)
- Modify: `src/hero/mod.rs` add `pub mod status;`

**Step 1: Write failing test for `format_countdown`**

Create `src/hero/status.rs` with:

```rust
//! Hero status helpers: Missing / Injured lifecycle constants and pure
//! formatters used by both the tick system and the roster UI.

use bevy::prelude::*;

/// How long (in game-seconds) a hero stays Missing before returning Injured.
pub const MISSING_DURATION_SECS: f64 = 120.0;
/// How long (in game-seconds) the Injured stat penalty persists after return.
pub const INJURED_DURATION_SECS: f64 = 300.0;
/// Multiplier applied to STR/DEX/CON while Injured.
pub const INJURED_STAT_MULTIPLIER: f32 = 0.75;

/// Marks a hero as absent after a mission wipe. `expires_at` is in the
/// `Time<Virtual>` elapsed-seconds frame.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Missing {
    pub expires_at: f64,
}

/// Temporary stat-penalty state applied when a Missing hero returns.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Injured {
    pub expires_at: f64,
}

/// Format a remaining-seconds value as `m:ss`. Negative / zero → `"0:00"`.
pub fn format_countdown(remaining_secs: f64) -> String {
    let total = remaining_secs.max(0.0).ceil() as u64;
    let minutes = total / 60;
    let seconds = total % 60;
    format!("{minutes}:{seconds:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_countdown_formats_whole_minutes() {
        assert_eq!(format_countdown(120.0), "2:00");
    }

    #[test]
    fn format_countdown_pads_seconds() {
        assert_eq!(format_countdown(65.0), "1:05");
    }

    #[test]
    fn format_countdown_rounds_up_partial_second() {
        assert_eq!(format_countdown(59.2), "1:00");
    }

    #[test]
    fn format_countdown_clamps_negative_to_zero() {
        assert_eq!(format_countdown(-3.0), "0:00");
        assert_eq!(format_countdown(0.0), "0:00");
    }
}
```

**Step 2: Run tests — expect "file not in crate"**

Run: `cargo test -p guild-forge hero::status --lib`
Expected: fails to compile because `status` isn't declared in `src/hero/mod.rs`.

**Step 3: Register module & components**

Edit `src/hero/mod.rs`:
- After `pub mod data;` (line 3) add: `pub mod status;`
- In `plugin` (line 11-18), register the new types. Insert after `app.register_type::<PersonallyManaged>();`:
  ```rust
  app.register_type::<status::Missing>();
  app.register_type::<status::Injured>();
  ```

**Step 4: Run tests again**

Run: `cargo test -p guild-forge hero::status --lib`
Expected: 4 passing.

**Step 5: Commit**

```bash
git add src/hero/mod.rs src/hero/status.rs
git commit -m "feat(hero): add Missing/Injured components and countdown formatter"
```

---

### Task 2: Missing-state tick system (Missing → Injured → cleared)

**Files:**
- Create: `src/hero/status_tick.rs`
- Modify: `src/hero/mod.rs` (declare module, add system to `plugin`)

**Step 1: Write the tick system**

Create `src/hero/status_tick.rs`:

```rust
//! Per-frame lifecycle for Missing and Injured components.
//!
//! Runs in `Update` and reads `Time<Virtual>` so that `GameSpeed` scaling
//! and the offline time bank both flow through naturally. Transitions:
//! Missing expires → insert Injured (return toast) → Injured expires →
//! component removed silently.

use bevy::prelude::*;

use super::status::{Injured, Missing, INJURED_DURATION_SECS};
use super::{Favorite, HeroInfo};
use crate::ui::toast::{ToastEvent, ToastKind};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Update, (tick_missing, tick_injured).chain());
}

fn tick_missing(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    q: Query<(Entity, &Missing, &HeroInfo, Has<Favorite>)>,
) {
    let now = time.elapsed_secs_f64();
    for (entity, missing, info, is_favorite) in &q {
        if now < missing.expires_at {
            continue;
        }
        commands
            .entity(entity)
            .remove::<Missing>()
            .insert(Injured { expires_at: now + INJURED_DURATION_SECS });

        let kind = if is_favorite { ToastKind::Success } else { ToastKind::Info };
        commands.trigger(ToastEvent {
            title: format!("{} has returned", info.name),
            body: "Injured — stats reduced while they recover.".to_string(),
            kind,
        });
    }
}

fn tick_injured(
    mut commands: Commands,
    time: Res<Time<Virtual>>,
    q: Query<(Entity, &Injured)>,
) {
    let now = time.elapsed_secs_f64();
    for (entity, injured) in &q {
        if now >= injured.expires_at {
            commands.entity(entity).remove::<Injured>();
        }
    }
}
```

**Step 2: Wire it up**

In `src/hero/mod.rs`:
- Add `pub mod status_tick;` below `pub mod status;`
- In `plugin`, add `app.add_plugins(status_tick::plugin);` after the register calls.

**Step 3: Verify it compiles**

Run: `cargo check --all-targets`
Expected: no errors. (`ToastEvent`, `ToastKind` import path matches `src/ui/toast.rs:19-25`.)

**Step 4: Commit**

```bash
git add src/hero/mod.rs src/hero/status_tick.rs
git commit -m "feat(hero): tick Missing→Injured→cleared each frame"
```

---

### Task 3: Apply Missing on mission wipe (replace party-wipe cleanup)

**Files:**
- Modify: `src/mission/combat.rs:258-272` (the `all_dead` branch of `check_mission_completion`)

**Step 1: Read the current block**

Confirm current code:
```rust
let all_dead = !mission_heroes.is_empty() && mission_heroes.iter().all(|(_, c)| c.hp <= 0);
if all_dead {
    *progress = MissionProgress::Failed;
    commands.trigger(ToastEvent {
        title: format!("{} — Failed!", info.name),
        body: "Party wiped — no rewards".to_string(),
        kind: ToastKind::Failure,
    });
    for &hero_entity in &party.0 {
        commands.entity(hero_entity).remove::<super::OnMission>();
    }
    commands.entity(mission_entity).despawn();
    info!("Mission '{}' failed — all heroes fell!", info.name);
    continue;
}
```

**Step 2: Add imports**

At the top of `src/mission/combat.rs`, add to the existing `use crate::hero::...` import group (find the existing hero import and extend it):

```rust
use crate::hero::status::{Missing, MISSING_DURATION_SECS};
use crate::hero::Favorite;
```

Also add a `Time<Virtual>` resource parameter to `check_mission_completion`. Find the `fn check_mission_completion(` signature and add `time: Res<Time<Virtual>>,` and `favorite_q: Query<&crate::hero::HeroInfo, With<Favorite>>,` to its parameters.

**Step 3: Rewrite the wipe branch**

Replace the `if all_dead { ... }` block with:

```rust
if all_dead {
    *progress = MissionProgress::Failed;
    let expires_at = time.elapsed_secs_f64() + MISSING_DURATION_SECS;

    // Favorite-aware toast title.
    let favorited_name = party
        .0
        .iter()
        .find_map(|e| favorite_q.get(*e).ok().map(|i| i.name.clone()));
    let title = match favorited_name {
        Some(name) => format!("{name} is missing!"),
        None => format!("{} — Failed!", info.name),
    };
    commands.trigger(ToastEvent {
        title,
        body: "Party wiped — heroes are missing.".to_string(),
        kind: ToastKind::Failure,
    });

    for &hero_entity in &party.0 {
        commands
            .entity(hero_entity)
            .remove::<super::OnMission>()
            .insert(Missing { expires_at });
    }
    commands.entity(mission_entity).despawn();
    info!("Mission '{}' failed — heroes missing for {MISSING_DURATION_SECS}s", info.name);
    continue;
}
```

**Step 4: Verify**

Run: `cargo check --all-targets`
Expected: clean.

Run: `cargo test -p guild-forge`
Expected: all existing tests still pass.

**Step 5: Commit**

```bash
git add src/mission/combat.rs
git commit -m "feat(mission): mark wiped party as Missing instead of despawning silently"
```

---

### Task 4: Injured stat penalty at combat-stat spawn site

**Files:**
- Modify: `src/mission/entities.rs:98-150` — add `injured_q` param, apply 0.75× to STR/DEX/CON before derived-stat math.

**Step 1: Add the query parameter**

In `spawn_tokens_for_mission` (line 98), add a new parameter:

```rust
injured_q: &Query<(), With<crate::hero::status::Injured>>,
```

And import at top of file:
```rust
use crate::hero::status::INJURED_STAT_MULTIPLIER;
```

**Step 2: Apply the penalty**

Replace lines 125-128:
```rust
let mut hp = stats.constitution * 3 + info.level as i32 * 5;
let mut attack = (stats.strength + stats.dexterity) / 2;
let mut defense = (stats.constitution + stats.dexterity) / 2;
```

with:
```rust
let is_injured = injured_q.get(hero_entity).is_ok();
let mul = |v: i32| -> i32 {
    if is_injured {
        (v as f32 * INJURED_STAT_MULTIPLIER).floor() as i32
    } else {
        v
    }
};
let str_eff = mul(stats.strength);
let dex_eff = mul(stats.dexterity);
let con_eff = mul(stats.constitution);

let mut hp = con_eff * 3 + info.level as i32 * 5;
let mut attack = (str_eff + dex_eff) / 2;
let mut defense = (con_eff + dex_eff) / 2;
```

**Step 3: Pass the new query from callers**

Find all call sites of `spawn_tokens_for_mission`:

Run: `grep -n 'spawn_tokens_for_mission' src/mission/*.rs src/screens/*.rs`

For each caller (likely in `src/mission/` dispatch and `src/screens/party_select.rs:~469` area), thread through an `injured_q: Query<(), With<Injured>>` parameter and pass `&injured_q`.

**Step 4: Verify**

Run: `cargo check --all-targets`
Expected: clean.

**Step 5: Commit**

```bash
git add src/mission/entities.rs src/mission/*.rs src/screens/*.rs
git commit -m "feat(mission): apply -25% STR/DEX/CON to Injured heroes at combat spawn"
```

---

### Task 5: Filter Missing heroes out of dispatch

**Files:**
- Modify: `src/screens/party_select.rs:64, :152, :236`

**Step 1: Widen the query**

Change every occurrence of:
```rust
Query<(Entity, &HeroInfo), (With<Hero>, Without<OnMission>)>
```

to:
```rust
Query<(Entity, &HeroInfo), (With<Hero>, Without<OnMission>, Without<crate::hero::status::Missing>)>
```

(Three hits: line 64, 152, 236.)

**Step 2: Verify**

Run: `cargo check --all-targets`
Expected: clean. Missing heroes now won't appear in the available pool; Injured heroes still do (they dispatch but with reduced stats).

**Step 3: Commit**

```bash
git add src/screens/party_select.rs
git commit -m "feat(party): hide Missing heroes from the dispatchable pool"
```

---

### Task 6: Roster card countdown banner

**Files:**
- Modify: `src/screens/roster.rs:120-143`

**Step 1: Extend the query**

Find `build_hero_list` (contains the loop at line 120). Add `Option<&Missing>`, `Option<&Injured>`, and `Res<Time<Virtual>>` access to that function's signature and the upstream query feeding `hero_vec`. (Search upward from line 120 for where `hero_vec` is built — extend its tuple to include the two Options.)

Add imports at the top of `roster.rs`:
```rust
use crate::hero::status::{format_countdown, Injured, Missing};
```

**Step 2: Replace the class-text block**

Replace lines 139-143:
```rust
let class_text = if is_on_mission {
    format!("Lv.{} {} (On Mission)", info.level, info.class)
} else {
    format!("Lv.{} {}", info.level, info.class)
};
```

with (priority order: Missing > Injured > OnMission > idle):

```rust
let now = time.elapsed_secs_f64();
let class_text = if let Some(m) = missing {
    format!("Lv.{} {} — MISSING {}", info.level, info.class,
        format_countdown(m.expires_at - now))
} else if let Some(inj) = injured {
    format!("Lv.{} {} — INJURED {}", info.level, info.class,
        format_countdown(inj.expires_at - now))
} else if is_on_mission {
    format!("Lv.{} {} (On Mission)", info.level, info.class)
} else {
    format!("Lv.{} {}", info.level, info.class)
};
```

And tint `name_color` / `bg_color` for Missing (same gray as on-mission) — extend the existing conditionals:

```rust
let bg_color = if missing.is_some() {
    Color::srgba(0.35, 0.2, 0.2, 0.5)   // dim red-gray
} else if injured.is_some() {
    Color::srgba(0.3, 0.25, 0.15, 0.5)  // dim amber
} else if is_on_mission {
    Color::srgba(0.3, 0.3, 0.3, 0.4)
} else if is_selected {
    Color::srgba(0.275, 0.400, 0.750, 0.8)
} else {
    Color::srgba(0.2, 0.2, 0.3, 0.6)
};
```

**Step 3: Verify build**

Run: `cargo check --all-targets`
Expected: clean.

**Step 4: Commit**

```bash
git add src/screens/roster.rs
git commit -m "feat(roster): show MISSING/INJURED countdown on hero cards"
```

---

### Task 7: Save/load persistence

**Files:**
- Modify: `src/save.rs:619-638` (DTO), `src/save.rs:300-400` (handle_save query + assembly), `src/save.rs:79-184` (load_save insertion).

**Rationale for `remaining` not `expires_at`:** `Time<Virtual>::elapsed_secs_f64()` starts at 0 each run. Persisting an absolute `expires_at` would make every Missing hero return "now" on first tick after load. Persist *remaining seconds* and reconstruct `expires_at = current_elapsed + remaining` on load.

**Step 1: Write a round-trip test first**

Append to `src/save.rs`'s existing `#[cfg(test)]` block:

```rust
#[test]
fn hero_save_dto_round_trips_with_missing_and_injured() {
    let dto = HeroSaveDto {
        name: "A".into(),
        class: HeroClass::Warrior,
        level: 1,
        xp: 0,
        xp_to_next: 100,
        stats: HeroStatsSave { strength: 10, dexterity: 10, constitution: 10,
            intelligence: 10, wisdom: 10, charisma: 10 },
        traits: vec![],
        equipment: HeroEquipmentSave { weapon_tier: 0, armor_tier: 0, accessory_tier: 0 },
        on_mission: false,
        growth: HeroGrowthSave::default(),
        progress: HeroStatProgressSave::default(),
        favorite: false,
        personally_managed: false,
        missing_remaining: Some(42.0),
        injured_remaining: Some(200.0),
    };
    let s = ron::to_string(&dto).unwrap();
    let back: HeroSaveDto = ron::from_str(&s).unwrap();
    assert_eq!(back.missing_remaining, Some(42.0));
    assert_eq!(back.injured_remaining, Some(200.0));
}

#[test]
fn hero_save_dto_defaults_missing_and_injured_to_none() {
    // Old-format save (no fields) should deserialize with None.
    let old = r#"(name:"A",class:Warrior,level:1,xp:0,xp_to_next:100,
        stats:(strength:10,dexterity:10,constitution:10,intelligence:10,wisdom:10,charisma:10),
        traits:[],equipment:(weapon_tier:0,armor_tier:0,accessory_tier:0),on_mission:false)"#;
    let back: HeroSaveDto = ron::from_str(old).unwrap();
    assert_eq!(back.missing_remaining, None);
    assert_eq!(back.injured_remaining, None);
}
```

**Step 2: Run — expect field-missing compile error**

Run: `cargo test -p guild-forge save:: --lib`
Expected: compile errors on `missing_remaining` / `injured_remaining`.

**Step 3: Extend the DTO**

In `src/save.rs:619-638`, add two fields to `HeroSaveDto`:

```rust
#[serde(default)]
pub missing_remaining: Option<f64>,
#[serde(default)]
pub injured_remaining: Option<f64>,
```

**Step 4: Populate in `handle_save`**

In `handle_save` (line 300), extend the `heroes` query tuple with `Option<&Missing>, Option<&Injured>`, and add `time: Res<Time<Virtual>>,` parameter. In the DTO assembly (around line 397), compute:

```rust
missing_remaining: missing.map(|m| (m.expires_at - time.elapsed_secs_f64()).max(0.0)),
injured_remaining: injured.map(|i| (i.expires_at - time.elapsed_secs_f64()).max(0.0)),
```

(Use `use crate::hero::status::{Missing, Injured};` at the module top.)

**Step 5: Restore in `load_save`**

In `load_save` (line 141-184), pass `time: Res<Time<Virtual>>` in the signature. After the `entity_commands.insert(PersonallyManaged)` block (around line 181), add:

```rust
let now = time.elapsed_secs_f64();
if let Some(rem) = dto.missing_remaining {
    entity_commands.insert(Missing { expires_at: now + rem });
}
if let Some(rem) = dto.injured_remaining {
    entity_commands.insert(Injured { expires_at: now + rem });
}
```

**Step 6: Verify tests pass**

Run: `cargo test -p guild-forge save:: --lib`
Expected: both new tests pass, existing `hero_save_dto_round_trips_with_growth` still passes (add the two new fields initialized to `None` to its struct literal).

**Step 7: Commit**

```bash
git add src/save.rs
git commit -m "feat(save): persist Missing/Injured as remaining-seconds"
```

---

### Task 8: Full build + manual smoke

**Step 1: Full build**

Run: `cargo build`
Expected: clean.

**Step 2: Full test suite**

Run: `cargo test`
Expected: all passing.

**Step 3: Manual smoke (user)**

1. Start game, dispatch a clearly-doomed low-level party into a hard mission.
2. Wait for party wipe. Observe: failure toast appears, heroes vanish from dispatch list, roster cards show "MISSING 2:00" countdown (colored red-ish).
3. Wait 2 minutes of game-time (bump speed if tedious). Observe: return toast fires per hero; card switches to "INJURED 5:00" (amber).
4. Dispatch an Injured hero on an easy mission. Verify they appear in dispatch list and their in-mission HP/attack are visibly lower than their healthy self.
5. Save the game while a hero is Missing and another is Injured. Quit. Relaunch. Verify both countdowns resume from roughly where they left off.
6. Favorite a hero, send them to wipe. Verify the failure toast uses their name in the title.

**Step 4: Final commit (if any polish needed)**

Only commit if the smoke surfaced a bug fix.

---

## Checklist of skill references

- **superpowers:test-driven-development** — Task 1 (`format_countdown`) and Task 7 (DTO round-trip) are pure-function tests and must go red-then-green.
- **superpowers:executing-plans** — Orchestrator for this plan.
- **superpowers:requesting-code-review** — Run the code-reviewer after Task 8.
