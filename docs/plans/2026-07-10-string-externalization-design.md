# TI-6 · String Externalization — Design

**Date:** 2026-07-10
**Chunk:** TI-6 (steam-release-chunks.md) — move user-facing strings to a lookup table now; EFIGS localization is an EA-era decision but retrofitting strings is the painful part.

## Approach decision

Three options weighed:

- **A. Custom RON string table + `tr()`/`trf()` helpers** — ~100 LOC, zero new deps, matches the RON-everything house style (`abilities.ron`, `narrative_templates.ron`, …). **Chosen.**
- **B. `rust-i18n` crate** — proven, but YAML locale files and macro magic cut against house style for a need we can meet in one small module.
- **C. `fluent-rs`** — ICU-grade plurals/genders. The EFIGS decision itself is EA-era; committing to Fluent now is premature. If EA-era localization demands it, keyed strings migrate to Fluent mechanically — the painful part (keying every call site) is what this chunk does.

## Architecture

**`src/localization.rs`** — `static STRINGS: LazyLock<StringTable>` parsed from `include_str!("../assets/locales/en-US.ron")` (a flat `HashMap<String, String>`).

- `pub fn tr(key: &'static str) -> &'static str` — lookup; missing key returns the key itself (loud in UI, never panics) and `warn!`s.
- `pub fn trf(key: &'static str, args: &[(&str, &str)]) -> String` — `tr` + `{placeholder}` replacement, same convention the feed templates already use.

**Global static, not a Bevy Resource** — threading `Res<Strings>` through every widget helper and enum impl would be enormous churn for no benefit today. Locale switching (when it comes) happens at boot: an `OnceLock` init reading `assets/locales/{locale}.ron` from settings, embedded en-US as per-key fallback. Restart-to-switch-language is standard for games.

**Keys** are dotted, namespaced by module: `common.back`, `guild.header`, `guild.upgrade`, `material.iron_ore`, `building.war_room.desc`, `tutorial.step0`. The en-US.ron file is kept sorted and section-commented.

**Enum `name()`/`description()` impls** (materials, buildings, perks, epithets, equipment slots, …) keep their `match` but each arm becomes `tr("material.iron_ore")` — return type stays `&'static str` (the table is static), and the coverage test can see every key literal.

**`format!` strings feeding UI** become `trf` templates: `format!("Lv {} / {}", a, b)` → `trf("guild.level", &[("cur", …), ("max", …)])` with `guild.level: "Lv {cur} / {max}"`.

## Coverage enforcement

A unit test reads `src/**/*.rs` via `CARGO_MANIFEST_DIR`, regex-extracts every `tr("…")` / `trf("…")` key literal, and asserts **set equality both ways** against en-US.ron: no key used in code that the table lacks, no orphan table entries. Dynamically built keys are disallowed (nothing needs them today). A second test exercises `trf` interpolation and the missing-key fallback.

## Scope

**In:** every string literal a player sees — screens, menus, ui widgets (toast/banner/feed chrome), tutorial copy, sidebar, settings labels, enum `name()`/`description()` impls, UI-bound `format!` templates.

**Out (with the EFIGS hook noted):**
- `narrative_templates.ron` and RON-data `name`/`description` fields (buildings, classes, abilities, equipment, enemies, events, mission templates) — already externalized data files; EFIGS-era work is per-locale variants of those files, not code changes.
- Hero/guild proper names (`names.ron`) — proper nouns.
- Log/dev/debug strings, crash reporting, `Name::new(…)` entity labels — not player-facing.
- Plural rules — templates are phrased to avoid pluralization where possible; `.one`/`.many` key pairs are the escape hatch later.

## Testing

Unit: table parses, coverage set-equality test, `trf` interpolation, missing-key fallback. Hand-verify: launch, walk every screen (guild, missions, party select, mission view, roster, armory, recruiting, settings, pause, credits, tutorial on a fresh save) looking for leaked `some.key` text or blanks.
