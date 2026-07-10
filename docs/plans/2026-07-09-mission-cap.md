# TI-5 Concurrent-Mission Cap Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Cap concurrent missions at 3, raised +1 per level of a new War Room building; enforce at dispatch and surface in party select + mission board.

**Architecture:** `BuildingType::WarRoom` joins the existing buildings system; `GuildBuildings::mission_cap()` (= 3 + War Room level) and `can_dispatch(active)` are the single source of truth. `active_mission_count()` counts live `Mission` entities still `InProgress` (mission entities despawn on resolve, so live count = active count). The authoritative gate is inside `dispatch_mission`; party-select and mission-board UI are advisory displays that rebuild when the active count changes.

**Tech Stack:** Bevy 0.18 (binary crate — test with `cargo test <module>`, not `--lib`), bevy_declarative UI, ron data files. Design doc: `docs/plans/2026-07-09-mission-cap-design.md`.

**Test style (house rules):** `#[cfg(test)] mod tests` at the bottom of the source file; build a bare `World`, use `world.run_system_once(...)` with closures for query-taking helpers. See `src/ui/banner.rs` tests for reference.

---

### Task 1: War Room building + `mission_cap()` / `can_dispatch()`

**Files:**
- Modify: `src/buildings.rs` (enum ~line 9–50, `GuildBuildings` impl ~line 92–104, tests at end)
- Modify: `assets/data/buildings.ron`

**Step 1: Write the failing tests**

At the bottom of `src/buildings.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mission_cap_grows_with_war_room() {
        let mut buildings = GuildBuildings::default();
        assert_eq!(buildings.mission_cap(), 3);
        buildings.0.insert(BuildingType::WarRoom, 2);
        assert_eq!(buildings.mission_cap(), 5);
    }

    #[test]
    fn can_dispatch_boundaries() {
        let buildings = GuildBuildings::default(); // cap 3
        assert!(buildings.can_dispatch(0));
        assert!(buildings.can_dispatch(2));
        assert!(!buildings.can_dispatch(3));
        assert!(!buildings.can_dispatch(4));
    }

    #[test]
    fn building_database_includes_war_room() {
        let data: BuildingsData =
            ron::from_str(include_str!("../assets/data/buildings.ron")).unwrap();
        let db = BuildingDatabase(data.buildings);
        let def = db.get(BuildingType::WarRoom).expect("War Room in buildings.ron");
        assert_eq!(def.max_level, 3);
        assert_eq!(def.level_costs.len(), 3);
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test buildings::`
Expected: compile FAIL — `WarRoom` variant, `mission_cap`, `can_dispatch` don't exist.

**Step 3: Minimal implementation**

In `src/buildings.rs`, add the variant (end of enum) and to `ALL`:

```rust
    Tavern,
    WarRoom,
```

```rust
        BuildingType::Tavern,
        BuildingType::WarRoom,
```

In `name()`: `Self::WarRoom => "War Room",`
In `description()`: `Self::WarRoom => "Coordinate more expeditions in the field at once.",`

In `impl GuildBuildings`, after `max_applicants`:

```rust
    /// How many missions may run concurrently. War Room raises the ceiling.
    pub fn mission_cap(&self) -> u32 {
        3 + self.level(BuildingType::WarRoom)
    }

    pub fn can_dispatch(&self, active_missions: usize) -> bool {
        active_missions < self.mission_cap() as usize
    }
```

In `assets/data/buildings.ron`, after the Tavern entry:

```ron
        (
            id: WarRoom,
            max_level: 3,
            level_costs: [
                (gold: 150, materials: [(Wood, 6), (IronOre, 4)]),
                (gold: 450, materials: [(Lumber, 5), (SteelIngot, 4)]),
                (gold: 900, materials: [(ArcaneWood, 4), (EnchantedSteel, 3)]),
            ],
        ),
```

**Step 4: Run tests to verify they pass**

Run: `cargo test buildings::`
Expected: 3 passed. Also run `cargo test save::` — save round-trip must still pass (saves store `HashMap<BuildingType, u32>`; a missing WarRoom key reads as level 0).

**Step 5: Commit**

```bash
git add src/buildings.rs assets/data/buildings.ron
git commit -m "feat(ti5): War Room building with mission_cap/can_dispatch"
```

---

### Task 2: `active_mission_count` helper

**Files:**
- Modify: `src/mission/mod.rs` (helper near `MissionProgress` ~line 122; tests at end of file — check whether a `#[cfg(test)]` mod already exists and extend it if so)

**Step 1: Write the failing test**

```rust
#[cfg(test)]
mod cap_tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    #[test]
    fn active_mission_count_ignores_resolved() {
        let mut world = World::new();
        world.spawn((Mission, MissionProgress::InProgress));
        world.spawn((Mission, MissionProgress::InProgress));
        world.spawn((Mission, MissionProgress::Complete));
        world.spawn((Mission, MissionProgress::Failed));
        world.spawn(MissionProgress::InProgress); // no Mission marker — ignored

        let count = world
            .run_system_once(|q: Query<&MissionProgress, With<Mission>>| {
                active_mission_count(&q)
            })
            .unwrap();
        assert_eq!(count, 2);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test mission::cap_tests`
Expected: compile FAIL — `active_mission_count` not defined.

**Step 3: Minimal implementation**

In `src/mission/mod.rs`, after the `MissionProgress` enum:

```rust
/// Missions currently underway — the number the War Room cap is checked against.
/// Resolved missions despawn, but count only `InProgress` to be safe against
/// same-frame resolution.
pub fn active_mission_count(missions: &Query<&MissionProgress, With<Mission>>) -> usize {
    missions
        .iter()
        .filter(|p| **p == MissionProgress::InProgress)
        .count()
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test mission::cap_tests`
Expected: 1 passed.

**Step 5: Commit**

```bash
git add src/mission/mod.rs
git commit -m "feat(ti5): active_mission_count helper"
```

---

### Task 3: Enforce the cap in `dispatch_mission`

The guard composes the two tested pieces; no new unit test — hand-verified in Task 6.

**Files:**
- Modify: `src/screens/party_select.rs` (`dispatch_mission` ~line 376)

**Step 1: Add parameters**

To `dispatch_mission`'s signature (it already imports `Mission` and `MissionProgress`):

```rust
    buildings: Res<crate::buildings::GuildBuildings>,
    mission_q: Query<&MissionProgress, With<Mission>>,
```

**Step 2: Add the guard**

Immediately after the `if party.0.is_empty()` check (~line 413):

```rust
    let active = crate::mission::active_mission_count(&mission_q);
    if !buildings.can_dispatch(active) {
        let cap = buildings.mission_cap();
        warn!("Dispatch refused: War Room at capacity ({active}/{cap})");
        commands.trigger(crate::ui::toast::ToastEvent {
            title: "War Room at capacity".into(),
            body: format!("{active}/{cap} missions underway. Wait for one to finish or upgrade the War Room."),
            kind: crate::ui::toast::ToastKind::Failure,
            action: None,
        });
        return;
    }
```

**Step 3: Verify build + suite**

Run: `cargo test`
Expected: all pass (92 + 4 new = 96). Run `cargo clippy --all-targets` — no new warnings.

**Step 4: Commit**

```bash
git add src/screens/party_select.rs
git commit -m "feat(ti5): refuse dispatch when War Room is at capacity"
```

---

### Task 4: Party-select dispatch button shows cap state

**Files:**
- Modify: `src/screens/party_select.rs` (`refresh_party_select` ~line 139–172 rebuild triggers, ~line 318–345 bottom bar)

**Step 1: Rebuild when the active count changes**

Add params to `refresh_party_select`:

```rust
    buildings: Res<crate::buildings::GuildBuildings>,
    mission_q: Query<&MissionProgress, With<Mission>>,
    mut last_active: Local<Option<usize>>,
```

After `let mut should_rebuild = ...` (~line 158):

```rust
    let active = crate::mission::active_mission_count(&mission_q);
    if *last_active != Some(active) {
        *last_active = Some(active);
        should_rebuild = true;
    }
```

(First frame: `None != Some(x)` forces one rebuild — harmless, UI doesn't exist yet.)

**Step 2: Three-state bottom bar**

Replace the `let bottom = if selected_party.0.is_empty() { ... } else { ... }` block (~line 325–345). Keep the existing empty-party arm, add a cap arm reusing the same disabled style:

```rust
    let cap = buildings.mission_cap();
    let at_cap = !buildings.can_dispatch(active);

    let bottom = if selected_party.0.is_empty() || at_cap {
        let msg = if at_cap {
            format!("War Room full ({active}/{cap})")
        } else {
            "Select at least 1 hero".to_string()
        };
        bottom.child(
            div()
                .w(px(380.0))
                .h(px(80.0))
                .items_center()
                .justify_center()
                .bg(Color::srgba(0.3, 0.3, 0.3, 0.5))
                .border_radius(BorderRadius::MAX)
                .child(
                    text(msg)
                        .font_size(28.0)
                        .color(Color::srgba(0.6, 0.6, 0.6, 0.8)),
                ),
        )
    } else {
        bottom.child(widgets::game_button(
            format!("Dispatch! ({})", selected_party.0.len()),
            dispatch_mission,
        ))
    };
```

(At-cap wins over empty-party: show the cap message even with no heroes picked, since it's the harder blocker.When both, cap message shows — acceptable.)

**Step 3: Verify**

Run: `cargo test && cargo clippy --all-targets`
Expected: green, no new warnings. Visual check happens in Task 6.

**Step 4: Commit**

```bash
git add src/screens/party_select.rs
git commit -m "feat(ti5): dispatch button shows War Room full state"
```

---

### Task 5: Mission-board "Underway: n/cap" counter

**Files:**
- Modify: `src/screens/missions.rs` (`update_mission_board` ~line 63–97 triggers, top bar ~line 129–137)

**Step 1: Rebuild when the active count changes**

Add params to `update_mission_board`:

```rust
    buildings: Res<crate::buildings::GuildBuildings>,
    mission_q: Query<&crate::mission::MissionProgress, With<crate::mission::Mission>>,
    mut last_active: Local<Option<usize>>,
```

After `let mut should_rebuild = !has_ui;` (~line 75):

```rust
    let active = crate::mission::active_mission_count(&mission_q);
    if *last_active != Some(active) {
        *last_active = Some(active);
        should_rebuild = true;
    }
```

**Step 2: Counter in the top bar**

Replace the top bar (~line 130–135):

```rust
    let cap = buildings.mission_cap();
    let counter_color = if buildings.can_dispatch(active) {
        LABEL_TEXT
    } else {
        Color::srgb(0.9, 0.35, 0.25)
    };
    let top_bar = div()
        .row()
        .w_full()
        .justify_between()
        .items_center()
        .p(px(16.0))
        .child(widgets::header("Mission Board"))
        .child(
            text(format!("Underway: {active}/{cap}"))
                .font_size(22.0)
                .color(counter_color),
        );
```

**Step 3: Verify**

Run: `cargo test && cargo clippy --all-targets`
Expected: green, no new warnings.

**Step 4: Commit**

```bash
git add src/screens/missions.rs
git commit -m "feat(ti5): mission board shows underway/cap counter"
```

---

### Task 6: Final gate — hand verification, tick chunk, merge

**Step 1: Full gate**

Run: `cargo test` (expect 96 passing) and `cargo clippy --all-targets` (no new warnings).

**Step 2: Hand-verify in the running game** (see memory `driving-game-for-verification`)

The demo roster (3 heroes) can't naturally reach 3 concurrent missions AND attempt a 4th, so use the temp-hack pattern: temporarily change `mission_cap()` to return `1 + self.level(BuildingType::WarRoom)`, then `cargo run`:

1. Dispatch one hero to a mission → back to Missions tab: header shows "Underway: 1/1" in red.
2. Select another mission, pick a hero → bottom bar shows disabled "War Room full (1/1)" instead of Dispatch.
3. Guild screen shows the War Room card with costs; upgrade it (dev gold if needed) → cap 2, dispatch re-enabled.
4. Let the first mission finish while sitting on party select → button flips back to Dispatch without leaving the screen.

Revert the temp hack (`git diff` must show only intended changes), re-run `cargo test`.

**Step 3: Tick the chunk**

In `docs/steam-release-chunks.md` line 164, mark TI-5 `[x]` with `*(✓ 2026-07-09 — War Room building gates the cap, 3 + level; design: docs/plans/2026-07-09-mission-cap-design.md)*`.

**Step 4: Merge**

```bash
git add docs/steam-release-chunks.md
git commit -m "docs: tick TI-5 — concurrent-mission cap shipped"
git checkout main
git merge --no-ff feat/ti5-mission-cap -m "Merge branch 'feat/ti5-mission-cap' — TI-5 concurrent-mission cap"
cargo test   # re-verify on main
git branch -d feat/ti5-mission-cap
```
