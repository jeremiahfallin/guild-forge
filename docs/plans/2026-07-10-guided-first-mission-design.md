# FT-1 · Guided First Mission — Design

**Date:** 2026-07-10
**Chunk:** FT-1 (steam-release-chunks.md) — a lightly scripted first session: recruit a starter, dispatch, watch with contextual prompts. Skippable.

## Decisions

- **Fresh games start with 2 heroes + 60 gold** (user-approved). Today's fresh save (3 heroes, cap 3, 0 gold) makes the chunk's "recruit a starter" beat impossible. The first seeded applicant's hire cost clamps to ≤50 so the hire is always affordable. CT-7 owns any retuning.
- **Pinned coach-mark panel** (user-approved): one persistent panel top-center under the header, never blocks input, advances automatically, Skip Tutorial button on every step.

## Architecture

**`src/tutorial.rs`** — `TutorialState { step: u32, done: bool }`. Primitives only in the persisted form (no enums — see the ron::Value migration incident, memory `ron-value-lossy-enums`). Five beats, each advancing off observable state via a pure `target_step(...)` decision fn; no bespoke event plumbing:

| Step | Prompt | Advances when |
|---|---|---|
| 0 | Welcome; hire a third hero at Recruiting | roster count ≥ 3 |
| 1 | Open the Mission Board, pick a contract | GameTab::PartySelect entered |
| 2 | Add all three heroes, Dispatch | any mission active |
| 3 | Watch — combat slows time, log narrates | active missions return to 0 |
| 4 | Graduation + **Done** button | Done clicked (or Skip) |

**Panel** — own absolute overlay (GlobalZIndex above content, body `Pickable::IGNORE`), rebuilt on step change, visible on all gameplay tabs, `DespawnOnExit(Screen::Gameplay)`. Skip sets `done`.

**Fresh-game hooks** — `spawn_starter_heroes` 3 → 2; starting gold 60 inserted under the same fresh-game gate; `seed_applicant_board` clamps `applicants[0].hire_cost` to ≤50.

**Persistence** — `SaveData` gains `tutorial_done: bool` (serde default **true** so all existing saves skip) and `tutorial_step: u32` (default 0). Fresh games never read a save, so the in-code `Default` (not done, step 0) activates the tutorial. Autosave carries both.

## Testing

Unit: `target_step` per beat; fresh default = active; old-save deserialization (missing fields) = done; skip; save round-trip of the new fields. Hand-verify the full flow on a fresh save.

## Out of scope

Button highlighting/arrows, per-step art, localization (TI-6), re-running the tutorial from settings.
