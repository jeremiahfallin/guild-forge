# Favorite & PersonallyManaged Flags — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add two orthogonal per-hero flags — `Favorite` (UI prominence) and `PersonallyManaged` (will eventually exclude from auto-assign) — with roster UI to toggle them, visual indicators, save/load support, and sidebar highlighting. This is the smallest useful slice of the 2026-04-21 scale-and-automation design.

**Architecture:** Two marker components on hero entities. Save/load gains two boolean DTO fields with serde defaults so existing saves continue to work. Roster UI gets star and pin toggle buttons per hero. Favorites are sorted to the top of the roster list and highlighted in the sidebar mission list. `PersonallyManaged` has no behavioral effect yet (it will when the Dispatcher lands in a later slice); for now it's a displayed flag only.

**Tech Stack:** Bevy 0.18 (ECS), bevy_declarative (UI), Rust, serde + ron (save format).

---

## Conventions

- All file paths in this plan are relative to `C:\Users\bullf\dev\games\guild-forge`.
- Build/check commands assume the repo root as working directory.
- We use `cargo test` for unit tests. UI is validated by visual inspection after `cargo run`.
- Commit at the end of each task. Commit message format: lowercase imperative, prefixed with `feat:` / `test:` / `refactor:` as appropriate.
- Follow existing patterns in `src/hero/mod.rs` and `src/save.rs` for component/DTO style.
- Marker components (no data) use `#[derive(Component, Debug, Reflect)]` and `#[reflect(Component)]` to match existing patterns (e.g., `Hero`).

## Related skills

- @superpowers:test-driven-development — use for the unit-testable tasks (sort helper, save/load round-trip).
- @superpowers:verification-before-completion — run `cargo check` before each commit.

---

## Task 1: Add `Favorite` and `PersonallyManaged` marker components

**Files:**
- Modify: `src/hero/mod.rs` (add components near the existing `Hero` marker, around line 21)

**Step 1: Add the component definitions**

Insert after the `Hero` struct in `src/hero/mod.rs`:

```rust
/// UI-prominence flag. Favorited heroes are pinned at the top of lists,
/// highlighted in mission feeds, and surfaced as priority events in the
/// eventual Field Report dashboard. Purely cosmetic — does not affect
/// game rules.
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct Favorite;

/// Opt-in flag indicating the player wants to manage this hero by hand
/// rather than let the (future) Dispatcher auto-assign them. When the
/// Dispatcher lands, it will skip heroes with this component. Has no
/// behavioral effect yet — displayed only.
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct PersonallyManaged;
```

**Step 2: Register reflection in the plugin**

Find the `plugin` function in `src/hero/mod.rs` (around line 11) and add two `register_type` calls alongside the existing ones:

```rust
pub(super) fn plugin(app: &mut App) {
    app.register_type::<HeroGrowth>();
    app.register_type::<HeroStatProgress>();
    app.register_type::<Favorite>();
    app.register_type::<PersonallyManaged>();
    app.add_systems(Startup, load_hero_databases);
    app.add_systems(OnEnter(Screen::Gameplay), spawn_starter_heroes);
}
```

**Step 3: Verify compile**

Run: `cargo check`
Expected: clean compile, no new warnings besides the existing three (data.rs fields, DispatchButton).

**Step 4: Commit**

```bash
git add src/hero/mod.rs
git commit -m "feat: add Favorite and PersonallyManaged marker components"
```

---

## Task 2: Save/load — extend `HeroSaveDto` with `favorite` and `personally_managed`

**Files:**
- Modify: `src/save.rs` (HeroSaveDto struct, handle_save query, load_save spawn block)

**Step 1: Add fields to the DTO**

In `src/save.rs`, find `HeroSaveDto` (around line 607) and add two fields with serde defaults so legacy saves deserialize cleanly:

```rust
#[derive(Serialize, Deserialize)]
pub struct HeroSaveDto {
    pub name: String,
    pub class: HeroClass,
    pub level: u32,
    pub xp: u32,
    pub xp_to_next: u32,
    pub stats: HeroStatsSave,
    pub traits: Vec<HeroTrait>,
    pub equipment: HeroEquipmentSave,
    pub on_mission: bool,
    #[serde(default)]
    pub growth: HeroGrowthSave,
    #[serde(default)]
    pub progress: HeroStatProgressSave,
    #[serde(default)]
    pub favorite: bool,
    #[serde(default)]
    pub personally_managed: bool,
}
```

**Step 2: Update `handle_save` to include the flags**

Update the import line at the top of `src/save.rs` to pull in the new components:

```rust
use crate::hero::{
    Favorite, Hero, HeroGrowth, HeroInfo, HeroStatProgress, HeroStats, HeroTraits,
    PersonallyManaged, roll_growth,
};
```

Then update the `handle_save` function's `heroes` query signature (around line 302) to include `Has<Favorite>` and `Has<PersonallyManaged>`:

```rust
heroes: Query<
    (
        Entity,
        &HeroInfo,
        &HeroStats,
        &HeroTraits,
        &HeroEquipment,
        &HeroGrowth,
        &HeroStatProgress,
        Option<&OnMission>,
        Has<Favorite>,
        Has<PersonallyManaged>,
    ),
    With<Hero>,
>,
```

Update the iteration (around line 346) to destructure the new fields and include them in the DTO:

```rust
for (entity, info, stats, traits, equipment, growth, progress, on_mission, is_favorite, is_managed) in &heroes {
    let idx = hero_dtos.len();
    entity_to_index.insert(entity, idx);

    hero_dtos.push(HeroSaveDto {
        // ... existing fields unchanged ...
        favorite: is_favorite,
        personally_managed: is_managed,
    });
}
```

Keep all existing fields in the struct literal — just add the two new ones at the bottom.

**Step 3: Update `load_save` to restore the flags**

In `load_save` (around line 139), after the existing hero spawn, conditionally insert the components. Replace the spawn block with:

```rust
let mut entity_commands = commands.spawn((
    Name::new(dto.name.clone()),
    Hero,
    HeroInfo {
        name: dto.name.clone(),
        class: dto.class,
        level: dto.level,
        xp: dto.xp,
        xp_to_next: dto.xp_to_next,
    },
    HeroStats {
        strength: dto.stats.strength,
        dexterity: dto.stats.dexterity,
        constitution: dto.stats.constitution,
        intelligence: dto.stats.intelligence,
        wisdom: dto.stats.wisdom,
        charisma: dto.stats.charisma,
    },
    HeroTraits(dto.traits.clone()),
    HeroEquipment {
        weapon_tier: dto.equipment.weapon_tier,
        armor_tier: dto.equipment.armor_tier,
        accessory_tier: dto.equipment.accessory_tier,
    },
    restore_growth(&dto.growth, dto.class, &class_db),
    HeroStatProgress {
        strength: dto.progress.strength,
        dexterity: dto.progress.dexterity,
        constitution: dto.progress.constitution,
        intelligence: dto.progress.intelligence,
        wisdom: dto.progress.wisdom,
        charisma: dto.progress.charisma,
    },
));
if dto.favorite {
    entity_commands.insert(Favorite);
}
if dto.personally_managed {
    entity_commands.insert(PersonallyManaged);
}
let entity = entity_commands.id();
hero_entities.push(entity);
```

**Step 4: Verify compile**

Run: `cargo check`
Expected: clean compile.

**Step 5: Commit**

```bash
git add src/save.rs
git commit -m "feat: persist Favorite and PersonallyManaged in save data"
```

---

## Task 3: Test — `HeroSaveDto` round-trips the new flags

**Files:**
- Modify: `src/save.rs` (tests module, around line 717)

**Step 1: Write the failing test**

Add to the `tests` module in `src/save.rs`:

```rust
#[test]
fn hero_save_dto_round_trips_favorite_flags() {
    let dto = HeroSaveDto {
        name: "F".into(),
        class: HeroClass::Warrior,
        level: 1,
        xp: 0,
        xp_to_next: 100,
        stats: HeroStatsSave {
            strength: 10, dexterity: 10, constitution: 10,
            intelligence: 10, wisdom: 10, charisma: 10,
        },
        traits: vec![],
        equipment: HeroEquipmentSave {
            weapon_tier: 0, armor_tier: 0, accessory_tier: 0,
        },
        on_mission: false,
        growth: HeroGrowthSave::default(),
        progress: HeroStatProgressSave::default(),
        favorite: true,
        personally_managed: true,
    };
    let s = ron::ser::to_string(&dto).unwrap();
    let back: HeroSaveDto = ron::from_str(&s).unwrap();
    assert!(back.favorite);
    assert!(back.personally_managed);
}
```

**Step 2: Run the test**

Run: `cargo test --lib save::tests::hero_save_dto_round_trips_favorite_flags -- --nocapture`
Expected: PASS (the fields already exist from Task 2 — this is a regression test locking in the behavior).

**Step 3: Commit**

```bash
git add src/save.rs
git commit -m "test: lock in Favorite/PersonallyManaged save round-trip"
```

---

## Task 4: Test — legacy save without the new fields deserializes with defaults

**Files:**
- Modify: `src/save.rs` (tests module)

**Step 1: Write the failing test**

Add to the `tests` module:

```rust
#[test]
fn legacy_hero_save_dto_without_favorite_flags_defaults_false() {
    // A RON string missing `favorite` and `personally_managed`.
    let legacy = r#"(
        name: "L",
        class: Warrior,
        level: 1, xp: 0, xp_to_next: 100,
        stats: (strength: 10, dexterity: 10, constitution: 10,
                intelligence: 10, wisdom: 10, charisma: 10),
        traits: [],
        equipment: (weapon_tier: 0, armor_tier: 0, accessory_tier: 0),
        on_mission: false,
    )"#;
    let dto: HeroSaveDto = ron::from_str(legacy).unwrap();
    assert!(!dto.favorite);
    assert!(!dto.personally_managed);
}
```

**Step 2: Run the test**

Run: `cargo test --lib save::tests::legacy_hero_save_dto_without_favorite_flags_defaults_false -- --nocapture`
Expected: PASS — `#[serde(default)]` on the new fields means missing fields deserialize to `false`.

**Step 3: Commit**

```bash
git add src/save.rs
git commit -m "test: legacy saves without favorite flags default to false"
```

---

## Task 5: Add a pure sort helper for the roster list

**Files:**
- Modify: `src/screens/roster.rs` (add helper above `build_hero_list`, around line 79)

**Step 1: Write the failing test**

Add a `tests` module at the bottom of `src/screens/roster.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_with_favorites_first_puts_favorite_entries_before_non_favorites() {
        // Using plain numbers as a stand-in for entities; the helper operates
        // on the `(is_favorite, original_index)` tuple.
        let input: Vec<(bool, usize)> = vec![
            (false, 0),
            (true, 1),
            (false, 2),
            (true, 3),
            (false, 4),
        ];
        let sorted = sort_favorites_first(&input);
        // Favorites (index 1, 3) come first in their original order;
        // non-favorites (0, 2, 4) follow in their original order.
        assert_eq!(sorted, vec![1, 3, 0, 2, 4]);
    }

    #[test]
    fn sort_with_no_favorites_preserves_input_order() {
        let input: Vec<(bool, usize)> = vec![
            (false, 0),
            (false, 1),
            (false, 2),
        ];
        let sorted = sort_favorites_first(&input);
        assert_eq!(sorted, vec![0, 1, 2]);
    }

    #[test]
    fn sort_with_all_favorites_preserves_input_order() {
        let input: Vec<(bool, usize)> = vec![
            (true, 0),
            (true, 1),
            (true, 2),
        ];
        let sorted = sort_favorites_first(&input);
        assert_eq!(sorted, vec![0, 1, 2]);
    }
}
```

**Step 2: Run the test — expect compile error**

Run: `cargo test --lib screens::roster::tests -- --nocapture`
Expected: FAIL with "cannot find function `sort_favorites_first`" — this confirms the test exists and the function is missing.

**Step 3: Implement the helper**

Add to `src/screens/roster.rs` just above `build_hero_list`:

```rust
/// Stable-sort helper: return the input indices reordered so favorites come first,
/// preserving original order within each group. The input is `(is_favorite, original_index)`.
fn sort_favorites_first(entries: &[(bool, usize)]) -> Vec<usize> {
    let mut indexed: Vec<(bool, usize)> = entries.to_vec();
    // Stable sort: `true` (favorite) should come before `false`. Rust bool
    // sorts false-before-true naturally, so invert with `!`.
    indexed.sort_by_key(|(is_fav, _)| !*is_fav);
    indexed.into_iter().map(|(_, idx)| idx).collect()
}
```

**Step 4: Run the test again**

Run: `cargo test --lib screens::roster::tests -- --nocapture`
Expected: PASS (all three tests).

**Step 5: Commit**

```bash
git add src/screens/roster.rs
git commit -m "feat: add sort_favorites_first helper for roster list ordering"
```

---

## Task 6: Apply favorite sort to the roster hero list

**Files:**
- Modify: `src/screens/roster.rs` (`build_hero_list`, `spawn_roster`, `refresh_roster_on_selection_change`)

**Step 1: Update the `build_hero_list` query type**

The function currently takes `Query<(Entity, &HeroInfo, Option<&OnMission>), With<Hero>>`. Change it to include favorite-ness. Update the signature and both call sites (in `spawn_roster` and `refresh_roster_on_selection_change`) so the query is:

```rust
Query<(Entity, &HeroInfo, Option<&OnMission>, Has<Favorite>), With<Hero>>
```

Add to the imports at the top of `src/screens/roster.rs`:

```rust
use crate::hero::{Favorite, Hero, HeroInfo, HeroStats, HeroTraits, data::*};
```
(replace the existing hero import line; add `Favorite` and keep the rest.)

**Step 2: Sort favorites to the top in `build_hero_list`**

Replace the `for (entity, info, on_mission) in heroes.iter() { ... }` loop with:

```rust
// Collect hero iteration with favorite flag, then sort favorites to the top.
let hero_vec: Vec<(Entity, &HeroInfo, Option<&OnMission>, bool)> = heroes
    .iter()
    .map(|(e, i, om, is_fav)| (e, i, om, is_fav))
    .collect();
let indexed: Vec<(bool, usize)> = hero_vec
    .iter()
    .enumerate()
    .map(|(i, (_, _, _, is_fav))| (*is_fav, i))
    .collect();
let order = sort_favorites_first(&indexed);

for i in order {
    let (entity, info, on_mission, is_favorite) = hero_vec[i];
    let is_selected = selected.0 == Some(entity);
    let is_on_mission = on_mission.is_some();

    // ... existing bg_color, name_color, class_text logic unchanged ...
```

Keep the rest of the loop body (the `list = list.child(...)` block) as-is for now; we'll add the star indicator in Task 7.

**Step 3: Verify compile**

Run: `cargo check`
Expected: clean compile.

**Step 4: Visual check**

Run: `cargo run`
Expected: the roster screen still works, shows all heroes. Favorites aren't distinguishable yet (we'll add visual indicators next), and no heroes have the `Favorite` component yet so ordering is unchanged. Close the game.

**Step 5: Commit**

```bash
git add src/screens/roster.rs
git commit -m "feat: sort roster by favorites first"
```

---

## Task 7: Add star and pin indicators to roster hero cards

**Files:**
- Modify: `src/screens/roster.rs` (hero card rendering)

**Step 1: Extend `build_hero_list` query to include `PersonallyManaged`**

Update the query in both call sites to include `Has<PersonallyManaged>`:

```rust
Query<(Entity, &HeroInfo, Option<&OnMission>, Has<Favorite>, Has<PersonallyManaged>), With<Hero>>
```

Add `PersonallyManaged` to the hero module import at the top of the file.

Update the `hero_vec` collection and the loop destructure to carry the new boolean.

**Step 2: Add a right-side icon column to each hero card**

In the `list = list.child(...)` block inside the hero-card loop, change the row to include an icon column at the end. The existing row:

```rust
list = list.child(
    div()
        .row()
        .w_full()
        .p(px(12.0))
        .gap(px(12.0))
        .items_center()
        .bg(bg_color)
        .rounded(px(6.0))
        .insert(SelectHeroButton(entity))
        .on_click(select_hero)
        .child(
            div()
                .col()
                .flex_1()
                .child(text(&info.name).font_size(22.0).color(name_color))
                .child(text(class_text).font_size(16.0).color(LABEL_TEXT)),
        ),
);
```

Becomes:

```rust
let star_glyph = if is_favorite { "★" } else { "☆" };
let star_color = if is_favorite {
    Color::srgb(1.0, 0.85, 0.2)
} else {
    Color::srgba(0.5, 0.5, 0.5, 0.7)
};
let pin_glyph = if is_managed { "📌" } else { "·" };
let pin_color = if is_managed {
    Color::srgb(0.5, 0.8, 1.0)
} else {
    Color::srgba(0.5, 0.5, 0.5, 0.5)
};

list = list.child(
    div()
        .row()
        .w_full()
        .p(px(12.0))
        .gap(px(12.0))
        .items_center()
        .bg(bg_color)
        .rounded(px(6.0))
        .insert(SelectHeroButton(entity))
        .on_click(select_hero)
        .child(
            div()
                .col()
                .flex_1()
                .child(text(&info.name).font_size(22.0).color(name_color))
                .child(text(class_text).font_size(16.0).color(LABEL_TEXT)),
        )
        .child(
            div()
                .col()
                .gap(px(4.0))
                .items_center()
                .child(
                    text(star_glyph)
                        .font_size(20.0)
                        .color(star_color)
                        .insert(ToggleFavoriteButton(entity))
                        .on_click(toggle_favorite),
                )
                .child(
                    text(pin_glyph)
                        .font_size(16.0)
                        .color(pin_color)
                        .insert(ToggleManagedButton(entity))
                        .on_click(toggle_managed),
                ),
        ),
);
```

**Step 3: Add the toggle button components and click handlers**

At the bottom of `src/screens/roster.rs` (near `SelectHeroButton`), add:

```rust
/// Component on the star icon inside a hero row; toggles `Favorite` on click.
#[derive(Component)]
struct ToggleFavoriteButton(Entity);

/// Component on the pin icon inside a hero row; toggles `PersonallyManaged` on click.
#[derive(Component)]
struct ToggleManagedButton(Entity);

fn toggle_favorite(
    click: On<Pointer<Click>>,
    buttons: Query<&ToggleFavoriteButton>,
    favorites: Query<(), With<Favorite>>,
    mut commands: Commands,
    mut selected: ResMut<SelectedHero>,
) {
    let Ok(button) = buttons.get(click.event_target()) else { return };
    if favorites.get(button.0).is_ok() {
        commands.entity(button.0).remove::<Favorite>();
    } else {
        commands.entity(button.0).insert(Favorite);
    }
    // Force a roster rebuild so the sort and icon state update.
    selected.set_changed();
}

fn toggle_managed(
    click: On<Pointer<Click>>,
    buttons: Query<&ToggleManagedButton>,
    managed: Query<(), With<PersonallyManaged>>,
    mut commands: Commands,
    mut selected: ResMut<SelectedHero>,
) {
    let Ok(button) = buttons.get(click.event_target()) else { return };
    if managed.get(button.0).is_ok() {
        commands.entity(button.0).remove::<PersonallyManaged>();
    } else {
        commands.entity(button.0).insert(PersonallyManaged);
    }
    selected.set_changed();
}
```

**Step 4: Suppress click-propagation**

The icons sit inside a parent row that has a `SelectHeroButton` click handler — clicking the star would also select the hero. This is fine (probably desirable — clicking the star also gives a "focus" feel), but if it turns out to feel bad in playtest, the fix is to attach `Pickable::IGNORE` to the text children and handle the toggle on a wrapper div with its own click observer. For this first pass, accept both effects: clicking a star both toggles favorite and selects the hero.

**Step 5: Verify compile**

Run: `cargo check`
Expected: clean compile.

**Step 6: Visual check**

Run: `cargo run`, navigate to the Roster screen.
Expected:
- Each hero shows a star (☆) and pin (·) on the right.
- Clicking a star toggles it to ★ (yellow) and pins that hero to the top.
- Clicking the pin toggles it to 📌 (blue) for that hero.
- Both flags persist through screen navigation.

Close the game. If anything visibly wrong, fix before committing.

**Step 7: Commit**

```bash
git add src/screens/roster.rs
git commit -m "feat: star and pin toggles on roster hero cards"
```

---

## Task 8: Verify save/load persists the flags end-to-end

**Files:** No code changes. Manual verification.

**Step 1: Play-test save/load**

Run: `cargo run`
1. Navigate to Roster, favorite one hero (click the star — it turns ★).
2. Mark a different hero as personally managed (click the pin — it turns 📌).
3. Wait for an autosave toast ("Game Saved") or force one by letting the timer elapse — or close the game cleanly (on-close autosave if implemented; otherwise just wait). Alternatively, trigger `SaveGame` manually if there's a keybind; otherwise let autosave tick.
4. Fully close the game.
5. Relaunch: `cargo run`
6. Navigate to Roster.

Expected:
- The previously-favorited hero still has a ★ and is at the top.
- The previously-managed hero still has 📌.

If either flag doesn't persist, the bug is in Task 2's save/load wiring — re-open those files and confirm the `Has<_>` queries and the `insert` calls in `load_save` are present and correct.

**Step 2: Verify legacy save compatibility**

If you have a pre-existing save file from before this branch, launch the game once with the new build. Expected: the game loads without error, no heroes are favorited or managed (because the old save lacks those fields, and `#[serde(default)]` makes them `false`).

**Step 3: No commit needed** — this is validation only.

---

## Task 9: Highlight missions containing favorites in the sidebar mission list

**Files:**
- Modify: `src/screens/sidebar.rs` (`update_mission_list`)

**Step 1: Extend the mission list query to know which heroes are favorites**

In `update_mission_list` (around line 283), add a query parameter for favorite heroes. Update the signature:

```rust
fn update_mission_list(
    mut commands: Commands,
    list_q: Query<Entity, With<SidebarMissionList>>,
    missions: Query<(Entity, &MissionInfo, &MissionProgress, &crate::mission::MissionParty), With<Mission>>,
    favorite_heroes: Query<(), (With<crate::hero::Hero>, With<crate::hero::Favorite>)>,
    children_q: Query<&Children>,
    mut last_snapshot: Local<Vec<(Entity, MissionProgress, bool)>>,
) {
```

Note the addition of `&MissionParty` to the missions query and the new `favorite_heroes` filter query. The `last_snapshot` now carries a `bool` for "party contains favorite" so we detect changes when favoriting happens.

**Step 2: Compute favorite-presence and use it in sorting and rendering**

Replace the snapshot-building and render section. Update the snapshot to include favorite-presence:

```rust
let mut snapshot: Vec<(Entity, MissionProgress, bool)> = missions
    .iter()
    .map(|(e, _, p, party)| {
        let has_favorite = party.0.iter().any(|h| favorite_heroes.get(*h).is_ok());
        (e, *p, has_favorite)
    })
    .collect();
snapshot.sort_by_key(|(e, _, _)| *e);
```

Then in the "rebuild mission entries" loop, read favorite-presence per mission and tint the entry:

```rust
for (mission_entity, info, progress, party) in &missions {
    let has_favorite = party.0.iter().any(|h| favorite_heroes.get(*h).is_ok());

    let status_text = match progress { /* unchanged */ };

    let base_bg = match progress {
        MissionProgress::InProgress => Color::srgba(0.2, 0.25, 0.35, 0.8),
        MissionProgress::Complete => Color::srgba(0.15, 0.35, 0.15, 0.8),
        MissionProgress::Failed => Color::srgba(0.35, 0.15, 0.15, 0.8),
    };
    let bg_color = if has_favorite {
        // Warm tint for missions containing a favorite.
        Color::srgba(base_bg.to_srgba().red + 0.15, base_bg.to_srgba().green + 0.10, base_bg.to_srgba().blue, 0.9)
    } else {
        base_bg
    };

    let name_text = if has_favorite {
        format!("★ {}", info.name)
    } else {
        info.name.clone()
    };

    let entry = div()
        // ... existing .col()/.w_full()/etc ...
        .bg(bg_color)
        .rounded(px(4.0))
        .insert(WatchMissionButton(mission_entity))
        .on_click(watch_mission)
        .child(
            text(name_text)
                .font_size(14.0)
                .color(HEADER_TEXT),
        )
        .child(
            text(status_text)
                .font_size(12.0)
                .color(LABEL_TEXT),
        );

    entry.spawn_as_child_of(&mut commands, list_entity);
}
```

The tint is a slight warm shift. If it's too subtle, bump the +0.15/+0.10 values; if it clashes, swap to a simple border or a prefix-only approach (drop the bg change, keep the ★ prefix).

**Step 3: Make sure imports are correct**

At the top of `src/screens/sidebar.rs`, add to the existing `crate::` imports:

```rust
use crate::{
    // ... existing ...
    hero::{Favorite, Hero},
    mission::{Mission, MissionInfo, MissionParty, MissionProgress, ViewedMission},
    // ... existing ...
};
```
(the `mission::MissionParty` path may need adjustment — check the existing `MissionParty` location in `src/mission/mod.rs`.)

**Step 4: Verify compile**

Run: `cargo check`
Expected: clean compile. If `Color::to_srgba` or arithmetic is wrong, fall back to the simpler approach: keep the same bg, only apply the ★ prefix.

**Step 5: Visual check**

Run: `cargo run`
1. Favorite a hero in the Roster.
2. Dispatch a mission with that hero.
3. Observe the sidebar: the mission entry for this party shows "★ <mission name>" and a warm-tinted background.
4. Dispatch a second mission without a favorite: that entry looks normal.

Expected: visual distinction between "a favorite is on this mission" and "no favorite on this mission."

**Step 6: Commit**

```bash
git add src/screens/sidebar.rs
git commit -m "feat: highlight sidebar missions containing favorite heroes"
```

---

## Task 10: Display favorite/managed status on the detail panel

**Files:**
- Modify: `src/screens/roster.rs` (`build_detail_panel`)

**Step 1: Extend the detail panel query to include the flags**

Update the `hero_query` type in `spawn_roster` and `refresh_roster_on_selection_change` to include favorite and managed:

```rust
hero_query: Query<
    (&HeroInfo, &HeroStats, &HeroTraits, Has<Favorite>, Has<PersonallyManaged>),
    With<Hero>,
>,
```

Update `build_detail_panel`'s signature to match, and destructure:

```rust
let Ok((info, stats, traits, is_favorite, is_managed)) = hero_query.get(entity) else {
    // ... unchanged ...
};
```

**Step 2: Add a status line to the detail panel header**

In `build_detail_panel`, add a status line below the level/class line:

```rust
let header = div()
    .col()
    .gap(px(4.0))
    .child(text(&info.name).font_size(36.0).color(HEADER_TEXT))
    .child(
        text(format!("Level {} {}", info.level, info.class))
            .font_size(20.0)
            .color(LABEL_TEXT),
    )
    .child(
        text(format!("XP: {} / {}", info.xp, info.xp_to_next))
            .font_size(16.0)
            .color(LABEL_TEXT),
    )
    .child({
        let status_parts: Vec<&str> = [
            is_favorite.then_some("★ Favorite"),
            is_managed.then_some("📌 Personally Managed"),
        ]
        .into_iter()
        .flatten()
        .collect();
        let status_text = if status_parts.is_empty() {
            String::new()
        } else {
            status_parts.join("   ")
        };
        text(status_text).font_size(14.0).color(Color::srgb(0.9, 0.85, 0.4))
    });
```

When neither flag is set, the line is empty (zero-height text). This is acceptable; if the empty gap looks ugly, gate the child behind `if !status_parts.is_empty()`.

**Step 3: Verify compile**

Run: `cargo check`
Expected: clean.

**Step 4: Visual check**

Run: `cargo run`, navigate to Roster, select a favorited hero.
Expected: the detail panel shows "★ Favorite" (and "📌 Personally Managed" if both are set) in a warm-gold color under the XP line.

**Step 5: Commit**

```bash
git add src/screens/roster.rs
git commit -m "feat: show favorite and managed status on hero detail panel"
```

---

## Task 11: Final verification and cleanup

**Files:** No changes. Run a full check.

**Step 1: Run all tests**

Run: `cargo test --lib`
Expected: all tests pass, including the new round-trip and legacy tests from Tasks 3–4 and the sort helper tests from Task 5.

**Step 2: Full build**

Run: `cargo check --all-targets`
Expected: clean compile, no new warnings.

**Step 3: Smoke test**

Run: `cargo run` and perform the full happy-path:
1. Favorite a hero → they pin to the top of the roster.
2. Personally-manage a different hero → 📌 shows on their card.
3. Dispatch the favorite on a mission → sidebar mission entry is highlighted.
4. Unfavorite the hero mid-mission → highlight disappears on the next `update_mission_list` tick.
5. Let the autosave fire (or play long enough for the 5-minute timer).
6. Close and relaunch. Flags persist.

**Step 4: Commit message cleanup**

If any Task commits have fix-ups, squash with interactive rebase only if requested by the user. Default: leave the commit history as-is (one commit per task).

---

## Open Follow-ups (Not In This Plan)

These belong to future slices, not this one:

- The **Dispatcher NPC** that actually honors `PersonallyManaged` by skipping those heroes during auto-assignment. Until the Dispatcher exists, the flag is purely informational.
- **Exception-queue events** for favorite-hero level-ups, rare loot, and Missing status. Requires the event system and Field Report dashboard.
- **Bulk toggle** (select multiple heroes and favorite them all) — not needed until the roster grows large enough to matter.
- **Filtering roster by favorite/managed** — an explicit "show favorites only" toggle on the roster screen. Likely wanted once the guild has 20+ heroes.
- **Sidebar sort by favorite** — right now missions in the sidebar are rebuilt in arbitrary ECS iteration order. A later polish pass could sort favorite-containing missions to the top of the sidebar list as well.
