# TI-6 String Externalization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Every player-visible string literal moves from code into `assets/locales/en-US.ron`, accessed via `tr()`/`trf()`, with a set-equality coverage test keeping code and table in lockstep.

**Architecture:** `src/localization.rs` holds a `LazyLock` static parsed from the embedded RON table. `tr(key)` returns `&'static str` (missing key → the key itself + warn). `trf(key, args)` interpolates `{placeholder}`s. Enum `name()`/`description()` match arms become `tr("…")` calls. See `docs/plans/2026-07-10-string-externalization-design.md`.

**Tech Stack:** Rust / Bevy 0.18, ron, regex (dev-dependency, coverage test only).

---

### Task 1: Localization core

**Files:**
- Create: `src/localization.rs`
- Create: `assets/locales/en-US.ron`
- Modify: `src/main.rs` (declare `mod localization;`)
- Modify: `Cargo.toml` (add `regex` to `[dev-dependencies]`)

**Step 1: Write the failing tests** (inside `src/localization.rs` `#[cfg(test)]`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_parses_and_lookup_works() {
        assert_eq!(tr("test.probe"), "probe value");
    }

    #[test]
    fn missing_key_returns_key() {
        assert_eq!(tr("no.such.key"), "no.such.key");
    }

    #[test]
    fn trf_interpolates() {
        // test.greet = "Hello {name}, you have {n} quests"
        assert_eq!(
            trf("test.greet", &[("name", "Sera"), ("n", "3")]),
            "Hello Sera, you have 3 quests"
        );
    }

    /// Every tr()/trf() key literal in src/ must exist in en-US.ron and vice versa.
    #[test]
    fn coverage_set_equality() {
        let keys_in_code = scan_src_for_keys();
        let keys_in_table: std::collections::BTreeSet<String> =
            STRINGS.keys().cloned().collect();
        let missing: Vec<_> = keys_in_code.difference(&keys_in_table).collect();
        let orphaned: Vec<_> = keys_in_table
            .difference(&keys_in_code)
            .filter(|k| !k.starts_with("test."))
            .collect();
        assert!(missing.is_empty(), "keys used in code but absent from en-US.ron: {missing:?}");
        assert!(orphaned.is_empty(), "orphan keys in en-US.ron never used in code: {orphaned:?}");
    }

    fn scan_src_for_keys() -> std::collections::BTreeSet<String> {
        let re = regex::Regex::new(r#"\btrf?\(\s*"([a-z0-9_.]+)""#).unwrap();
        let mut keys = std::collections::BTreeSet::new();
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut stack = vec![src];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    let text = std::fs::read_to_string(&path).unwrap();
                    for cap in re.captures_iter(&text) {
                        keys.insert(cap[1].to_string());
                    }
                }
            }
        }
        keys.remove("test.probe"); // this test file's own literals
        keys.remove("test.greet");
        keys.remove("no.such.key");
        keys
    }
}
```

**Step 2: Run to verify failure** — `cargo test localization` → compile error (module/functions absent).

**Step 3: Minimal implementation** (top of `src/localization.rs`)

```rust
//! TI-6 string table. All player-visible strings live in
//! assets/locales/en-US.ron; code refers to them by dotted key.
//! EFIGS hook: swap the include for a boot-time locale file load.
use std::collections::HashMap;
use std::sync::LazyLock;

static STRINGS: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    ron::from_str(include_str!("../assets/locales/en-US.ron"))
        .expect("Failed to parse en-US.ron")
});

/// Look up a UI string. A missing key returns the key itself so the
/// gap is visible in-game instead of panicking.
pub fn tr(key: &'static str) -> &'static str {
    match STRINGS.get(key) {
        Some(s) => s.as_str(),
        None => {
            bevy::log::warn!("missing string key: {key}");
            key
        }
    }
}

/// `tr` plus `{placeholder}` interpolation.
pub fn trf(key: &'static str, args: &[(&str, &str)]) -> String {
    let mut s = tr(key).to_string();
    for (name, value) in args {
        s = s.replace(&format!("{{{name}}}"), value);
    }
    s
}
```

`assets/locales/en-US.ron` starts as:

```ron
{
    // test fixtures (allowed as orphans by the coverage test)
    "test.probe": "probe value",
    "test.greet": "Hello {name}, you have {n} quests",
}
```

Add `mod localization;` to `src/main.rs`; add `regex = "1"` under `[dev-dependencies]`.

**Step 4: Run** — `cargo test localization` → 4 passed.

**Step 5: Commit** — `feat(ti6): localization core — tr/trf + en-US.ron + coverage test`

---

### Tasks 2–8: Per-module externalization passes

Same recipe each task. **Recipe:**

1. Open every file in the group; find player-visible literals: `text("…")`, button labels, `format!` bound to UI text, headers, enum `name()`/`description()` arms, tooltip/toast/banner copy.
2. For each: add a sorted, section-commented entry to `en-US.ron`; replace the literal with `tr("ns.key")` or, when it had `format!` args, `trf("ns.key", &[("arg", &value.to_string())])`. Import `crate::localization::{tr, trf}` (only what's used).
3. **Leave alone:** `Name::new("…")` entity labels, `info!`/`warn!` logs, dev_tools, string keys/IDs, RON data content, proper names.
4. Run `cargo test localization::tests::coverage_set_equality` — set equality catches typos and orphans immediately.
5. Run `cargo test` (module's own tests may assert on literals — update those asserts to `tr()` calls or new copy).
6. Commit: `feat(ti6): externalize <group> strings`.

**Task groups (each one commit):**

| Task | Group | Files | Key namespace |
|---|---|---|---|
| 2 | Data-ish enums | `src/materials.rs`, `src/buildings.rs`, `src/equipment.rs`, `src/hero/perk.rs`, `src/hero/epithet.rs`, `src/hero/status.rs`, `src/reputation.rs`, `src/recruiting.rs` | `material.*`, `building.*`, `equipment.*`, `perk.*`, `epithet.*`, `status.*`, `reputation.*`, `recruit.*` |
| 3 | Shared UI chrome | `src/ui/mod.rs`, `src/ui/toast.rs`, `src/ui/banner.rs`, `src/ui/feed.rs` (chrome only, not templates), `src/screens/sidebar.rs` | `common.*`, `banner.*`, `sidebar.*` |
| 4 | Guild + economy screens | `src/screens/guild.rs`, `src/screens/armory.rs`, `src/screens/roster.rs`, `src/screens/recruiting_screen.rs` | `guild.*`, `armory.*`, `roster.*`, `recruit.*` |
| 5 | Mission screens | `src/screens/missions.rs`, `src/screens/party_select.rs`, `src/screens/mission_view.rs` | `missions.*`, `party.*`, `mission_view.*` |
| 6 | Menus + title flow | `src/menus/*.rs`, `src/screens/title.rs`, `src/screens/splash.rs`, `src/screens/loading.rs`, `src/screens/gameplay.rs` | `menu.*`, `settings.*`, `credits.*`, `title.*` |
| 7 | Tutorial + stragglers | `src/tutorial.rs` (5 step prompts, Guide n/n, Skip/Done), `src/time_bank.rs`, `src/training.rs`, `src/hero/history.rs`, anything a final sweep finds | `tutorial.*`, misc |
| 8 | Final sweep | `rg '"[A-Z][a-z]' src --type rust` minus allowed patterns; verify nothing player-visible remains | — |

---

### Task 9: Full verification

1. `cargo test` — all green.
2. `cargo clippy --all-targets` — clean.
3. Launch the game (per `docs/plans` verification memory: PowerShell SendInput driving); walk guild, missions, party select, mission view, roster, armory, recruiting, settings, pause, tutorial (fresh save) looking for leaked `ns.key` text or blanks.
4. Tick TI-6 in `docs/steam-release-chunks.md` with date + summary line; commit `docs: tick TI-6`.
5. Merge branch to main (superpowers:finishing-a-development-branch).
