# Guild Forge — LLM-Tackleable Chunks: Current State → Early Access Launch

> **Created:** 2026-06-11 · grounded in the working tree as verified that day
> **Verified:** 2026-06-16 · all 26 ticked chunks confirmed present in code (most test-backed); RS-3 found ~75% built (see its note); stale `Touches` pointers on CT-2/CT-4 corrected
> **Companions:** `finished-product-vision.md` (what & why) · `steam-release-roadmap.md` (when & how to ship) · this doc (what to hand Claude Code next session)
> **End point:** EA launch (roadmap Phase 4). EA→1.0 content is sketched, not chunked — it will be reshaped by player feedback.

---

## How to Use This Document

- **One chunk ≈ one focused LLM session ≈ one PR.** Pick a chunk whose `Needs:` are all ticked, brainstorm/plan it first (the superpowers flow), build test-first, then tick its box with the date.
- **Definition of done for every chunk:** `cargo test` passes, `cargo clippy` clean, the chunk's *Done when* verified by hand, checkbox ticked.
- **IDs are stable.** Never renumber. New work gets a new ID (next free number in its workstream). If a chunk outgrows a session, split at the seam noted in its text and give the remainder a new ID.
- **Decisions inside** flags a design call embedded in the chunk — resolve it (alone or with the LLM) before or during the session, and record it in the relevant design doc.
- **Chunks point at design docs instead of restating them:** `combat-feel` = `docs/plans/2026-04-22-combat-feel-design.md`, `scale` = `docs/plans/2026-04-21-scale-and-automation-design.md`, `GDD` = `guild_forge_gdd.md`, `vision` = `docs/finished-product-vision.md`.

---

## CB — Combat Core

*The critical path. From `combat-feel`. A first slice exists in `src/mission/sequential.rs` (turn queue, dev-only F1 toggle); these chunks take it from slice to shipped default.*

- [x] **CB-1 · Design-parity turn rules** *(✓ 2026-06-12)* — bring `sequential.rs` to the written design: move **one tile** or act (not multi-tile `MoveRange` steps), initiative = d20 + DEX **rerolled every round** (replace the deterministic speed sort; keep seeded tie-breaking and the existing unit tests' spirit).
  Touches: `src/mission/sequential.rs`, `src/mission/entities.rs` (MoveRange semantics) · Design: combat-feel §6, §8 · Needs: —
  Done when: tests cover per-round reroll and one-tile movement; an F1-toggled mission visibly obeys both.
  Decision inside: confirm d20+DEX over speed-sort (the design argues reroll; the slice disagrees).

- [x] **CB-2 · Encounter enrollment + tempo split** *(✓ 2026-06-12)* — encounters begin when action ranges overlap; exploration ticks 4–8 turns/sec, combat 1–2 turns/sec. Replaces the flat 2 Hz `FixedUpdate` tick with a variable turn cadence (keep `Time<Virtual>` speed scaling working).
  Touches: `src/mission/mod.rs` (tick architecture), `sequential.rs` · Design: combat-feel §4, §5.1 · Needs: CB-1
  Done when: watching a mission, walking is brisk and fights are deliberate, with no camera/mode change — the tempo shift alone signals combat.

- [x] **CB-3 · Ability data layer** *(✓ 2026-06-12)* — RON-defined abilities (range, cooldown, effect, AI priority rule) loaded like the existing databases; wire up the currently-unused `starting_abilities` in `classes.ron` (Slash, Fireball, Heal…). Data + validation only, no sim behavior yet.
  Touches: new `assets/data/abilities.ron`, `src/mission/data.rs`, `src/hero/data.rs` · Design: combat-feel §7 · Needs: —
  Done when: abilities load at startup, every class resolves its kit, malformed data fails loudly in tests.

- [x] **CB-4 · Cooldown abilities in the sim** *(✓ 2026-06-12)* — heroes fire their 2–3 short-cooldown abilities on turn-counted cooldowns via per-ability AI priority rules (Cleave at 2+ adjacent, Heal at ally <50%).
  Touches: `sequential.rs`, `src/mission/ai.rs` · Design: combat-feel §7.2 · Needs: CB-1, CB-3
  Done when: in a watched fight each class fires its abilities 1–3 times, cooldowns respected (unit-tested).

- [x] **CB-5 · Signature moves** *(✓ 2026-06-12)* — one per class per encounter (Rallying Cry, Mass Heal, Meteor, Shadowstep/Assassinate, Volley), threshold-based timing rules so the AI visibly *saves* it; refreshes between encounters, not turns.
  Touches: `sequential.rs`, `ai.rs`, `abilities.ron` · Design: combat-feel §7.3 · Needs: CB-4
  Done when: a signature fires at most once per encounter at a sensible moment, and a wasted one is impossible.

- [x] **CB-6 · Traits as visible personality** *(✓ 2026-06-13)* — connect the existing `ai.rs` score multipliers (6 of 7 traits already steer decisions) to the new turn loop, then add the designed behaviors: Brave rushes the signature, Cautious retreats below 30% HP, Greedy spends a turn on chests, Loner positions away from allies. Give Leader its first mechanical effect.
  Touches: `ai.rs`, `sequential.rs` · Design: combat-feel §10, vision §3 · Needs: CB-4, CB-5
  Done when: two same-class heroes with opposite traits are tellable apart in one watched fight.
  Decision inside: what Leader does (party-buff aura is the natural fit).

- [x] **CB-7 · Enemy kits + real bosses** *(✓ 2026-06-13)* — `enemies.ron` gains abilities; every difficulty-3+ template ends in a boss with 1–2 telegraphed mechanics (AoE slam heroes scatter from, summoned adds, enrage timer). Retires the "BossRat is just 40 HP" era.
  Touches: `assets/data/enemies.ron`, `sequential.rs`, `ai.rs`, `dungeon.rs` (boss room) · Design: vision §5.3 · Needs: CB-3, CB-4
  Done when: a boss fight is visually and mechanically distinct from a room of trash mobs.

- [x] **CB-8 · Retire the simultaneous mode** *(✓ 2026-06-13)* — make Sequential the only player-facing sim. Keep the shared systems (`handle_death_system`, `update_room_status`, `check_mission_completion`), delete or dev-gate the walk-up combat systems and the F1 toggle, and migrate the serialized `SimulationMode` out of saves.
  Touches: `src/mission/mod.rs`, `combat.rs`, `dev_tools.rs`, `save.rs` · Needs: CB-1–CB-5 stable, TI-2 (save migration)
  Done when: a release build runs turn-based by default and an old save loads cleanly.
  Decision inside: hard-delete the old mode vs. keep it dev-only for A/B comparison.

## UX — Watchability

- [x] **UX-1 · Combat log / mission feed** *(✓ 2026-06-12)* — **do this first.** A scrolling, DM-voiced feed in the mission view ("Sera shadowsteps behind the orc — 17 damage!"), driven by sim events with string templates in RON. Emit from the *current* sim today; it gets richer as CB lands, and later becomes the Field Report's data source.
  Touches: new feed module under `src/ui/`, `src/screens/mission_view.rs`, event emission in `combat.rs`/`sequential.rs` · Design: vision §5.4 · Needs: —
  Done when: a full mission reads as a story — attacks, crits, deaths, loot, room entries all narrated.

- [x] **UX-2 · Hit feedback** *(✓ 2026-06-12)* — floating damage numbers, hit-flash, death poof, knockback nudge on the mission-view proxies; screen shake reserved for signature moves (that part lands with CB-5).
  Touches: `mission_view.rs`, `entities.rs` (proxy sync), `tileset.rs` · Design: vision §10.2 · Needs: — (shake part: CB-5)
  Done when: every hit is visible without reading the log.

- [x] **UX-3 · Event banners** *(✓ 2026-07-06)* — floating banners in the mission view ("BOSS ENCOUNTER", "RARE DROP!", "RESCUE WINDOW CLOSING"), driven by the UX-1 event stream.
  Touches: feed module (`src/ui/banner.rs`), `mission_view.rs` · Needs: UX-1
  Done when: the three banner-worthy moments interrupt the eye reliably and clear themselves.
  Design: `docs/plans/2026-07-06-event-banners-design.md` (boss fires on first combat overlap, Legendary-only drops, rescue threshold 30s).

- [ ] **UX-4 · Audio states** — exploration/combat/boss music layers with crossfade plus ability SFX hooks. Code and state machine are LLM work; the actual tracks are human-led (appendix).
  Touches: `src/audio.rs` · Design: vision §10.5 · Needs: CB-2 (combat state signal)
  Done when: with placeholder assets, entering combat audibly shifts and the boss layer triggers.

## CT — Content & Variance

- [x] **CT-1 · Mission modifiers** *(✓ 2026-06-13)* — data-driven modifiers from the GDD (Foggy, Infested, Cursed Ground, Bountiful + Trapped): template field, generation roll, sim effect, badge on the mission board.
  Touches: `assets/data/mission_templates.ron`, `src/mission/data.rs`, `dungeon.rs`, `src/screens/missions.rs` · Design: GDD §4.3, vision §5.1 · Needs: —
  Done when: generated missions roll 0–2 modifiers and each one demonstrably changes a run.

- [x] **CT-2 · Mid-mission events engine** *(✓ 2026-06-14)* — events fire 0–2× per mission, resolve via trait-and-stat checks, and *which hero* triggers them follows personality (Greedy touches the shrine). RON-defined, narrated through the feed.
  Touches: `assets/data/events.ron` + defs/DB in `src/mission/data.rs`, firing in `sequential.rs`, chronicle in `hero/mod.rs` (the planned `src/mission/events.rs` was never created — logic landed in those files instead) · Design: vision §5.2 · Needs: UX-1; richer after CB-1
  Done when: shrine/ambush/hidden-chamber events fire, resolve by check, and read clearly in the feed.

- [x] **CT-3 · Event content pack** *(✓ 2026-06-14)* — ~12–15 events across themes: wandering merchant, collapsed floor splits the party, rival-guild party cameo, cursed fountain. Pure data + flavor writing on the CT-2 engine. A good low-energy session.
  Touches: `events.ron` · Needs: CT-2

- [x] **CT-4 · Second biome** *(✓ 2026-06-14)* — crypt or forest tileset wired through the existing autotile bitmask system, an enemy-family mapping, and template binding. Code/data are LLM work against placeholder art until the commissioned set lands.
  Touches: enemy-family mapping + template binding in `src/mission/data.rs`, biome tile-tint in `mission_view.rs`, RON files · Design: vision §5.1 · Needs: — (final art: appendix)
  Done when: two visually and ecologically distinct biomes generate from templates.
  Note (2026-06-16): visual differentiation is currently a per-biome tile *tint* in `mission_view.rs` (Crypt → purple), not a distinct autotile set in `tileset.rs` as the chunk text implies — revisit when commissioned art lands.

- [x] **CT-5 · Enemy roster to 8–10 with behaviors** *(✓ 2026-06-14)* — new enemy entries plus per-enemy AI knobs (skirmisher kites, swarmers flood, shaman heals). Pairs with CB-7 kits.
  Touches: `enemies.ron`, `ai.rs` · Design: roadmap Phase 1 minimums · Needs: CB-4 (for kit-based behaviors)
  Done when: the demo's enemies are tellable apart by behavior in the feed and on screen.

- [x] **CT-6 · Gear rarity + behavioral affixes (first pass)** *(✓ 2026-06-14)* — rarity tiers Common→Legendary (GDD), one behavioral affix slot on Rare+ items (lifesteal, +initiative, cleave-on-hit) hooked into the ability system; legendary drops announced as events. This is the roadmap's "loot decisions feel interesting" bar.
  Touches: `src/equipment.rs`, `assets/data/equipment.ron`, loot resolution, feed · Design: GDD §4.1, vision §8 · Needs: CB-3, CB-4, UX-1
  Done when: a watched legendary drop changes what the hero visibly does next fight.

- [ ] **CT-7 · Demo progression tuning** — data-driven pass over XP/gold/building costs/reputation gates targeting a ~2-hour arc, soft-capped so the demo ends on a hook.
  Touches: RON files only, plus playtesting · Design: roadmap Phase 1 · Needs: most of CB/CT/HR landed; pairs with FT-2

## HR — Heroes & Attachment

- [x] **HR-1 · Chronicle data layer** *(✓ 2026-06-14)* — per-hero history (missions, kills, near-deaths, rescues given/received, lifetime gold, signature moments) captured from the UX-1 event stream and persisted in saves.
  Touches: `src/hero/mod.rs`, `src/save.rs` · Design: vision §4.2 · Needs: UX-1, TI-2 (schema change)
  Done when: history accumulates across missions and survives save/load.

- [x] **HR-2 · Career timeline UI** *(✓ 2026-06-14)* — scrollable timeline on the hero sheet ("Day 12: sole survivor of the Skeleton Crypt wipe").
  Touches: `src/screens/roster.rs` (or new hero-sheet screen) · Needs: HR-1

- [x] **HR-3 · Epithets** *(✓ 2026-06-14)* — milestone rules grant titles (*Slimebane*, *the Twice-Lost*) auto-displayed everywhere the name renders: roster, feed, toasts.
  Touches: `hero/mod.rs`, name-rendering call sites · Design: vision §4.2 · Needs: HR-1
  Done when: a kill-count epithet triggers mid-session and shows up in the next feed line.

- [x] **HR-4 · Portraits** *(✓ 2026-06-14)* — layered pixel-portrait compositor (base/hair/gear slots) rendering on roster, hero sheet, and feed. Compositor + integration are LLM work; the layer art is human-led (appendix).
  Touches: new portrait module, `roster.rs`, feed · Design: vision §4.1 · Needs: — (final art: appendix)

- [x] **HR-5 · Veteran perks** *(✓ 2026-06-14)* — small history-earned passives ("survived 3 rescues: +10% HP"), capped, shown on the hero sheet.
  Touches: `hero/mod.rs`, stat resolution · Design: vision §4.3 · Needs: HR-1

## RS — Rescue Missions *(demo scope, decided 2026-06-11)*

- [x] **RS-1 · Rescue generation + Missing semantics** *(✓ 2026-06-15)* — a wipe auto-generates a rescue mission into the same dungeon (same seed/layout) that must succeed before the Missing timer expires.
  Touches: mission generation, `src/hero/status.rs`, `status_tick.rs`, `save.rs` · Design: scale §8, vision §7 · Needs: —
  Done when: wiping a party immediately offers a runnable rescue with a live countdown.
  Decision inside: what expiry without rescue means — today Missing softens to Injured; the scale doc implies lost-forever. (Middle path: un-rescued heroes still soften to Injured for the demo; lost-forever + memorial arrives in EA.)

- [x] **RS-2 · Rescue beats** *(✓ 2026-06-15)* — the lost party's trail, campsite, and dropped gear appear as events in the rescue run; recovering gear closes the loss loop.
  Touches: `events.ron`, rescue generation · Design: vision §7 · Needs: RS-1, CT-2

- [x] **RS-3 · Rescue UX** *(✓ 2026-07-06)* — the wipe toast becomes actionable ("Mount rescue"), a priority card pins to the mission board, party select shows the countdown, and resolution writes chronicle entries for rescuers and rescued.
  Touches: `ui/toast.rs`, `missions.rs`, `party_select.rs` · Needs: RS-1; chronicle entries need HR-1
  Done when: the whole loop — wipe, alarm, rescue, reunion — plays without touching a menu you didn't expect.
  Note (2026-07-06): the 2026-06-16 "remaining" item (rescue resolution chronicle bookkeeping) landed with the M1/M2 sprint (a635358) — `check_mission_completion` in `src/mission/combat.rs` increments `rescues_given`/`rescues_received` and writes rescuer/rescued timeline lines, covered by `test_rescue_mission_success`.

## FT — First-Time Experience

- [ ] **FT-1 · Guided first mission** — a lightly scripted first session: recruit a starter, dispatch, watch with contextual prompts. Skippable.
  Touches: screens flow, new tutorial module · Design: roadmap Phase 1 ("currently none") · Needs: stable core loop (post-CB-8 ideally)

- [ ] **FT-2 · Demo gating + hook** — soft content cap (reputation/template gate) and an end-of-demo screen with a wishlist call-to-action.
  Touches: `missions.rs`, new screen · Needs: CT-7; wishlist link needs the Phase 2 store page

## TI — Tech Foundations

- [x] **TI-1 · Fix the `bevy_declarative` dependency** *(✓ 2026-06-12 — vendored via git subtree into `crates/bevy_declarative`; standalone repo archived)* — vendor into a workspace, pin a git rev, or publish; a clean clone must build in CI. **Blocks anyone else ever building the game — do early.**
  Touches: `Cargo.toml`, CI · Design: roadmap Phase 0 · Needs: —

- [x] **TI-2 · Save versioning + migration + backup** *(✓ 2026-06-12 — version field, migration registry, write-then-rename with .bak, corrupt-load fallback)* — Every schema-touching chunk (CB-8, HR-1, RS-1) leans on this; breaking saves is the #1 EA review killer.
  Touches: `src/save.rs` · Design: roadmap §9 · Needs: — · **Do before HR-1/CB-8 merge.**
  Done when: a deliberately old-versioned save migrates; a corrupted file falls back without a crash.

- [x] **TI-3 · Crash reporting** *(✓ 2026-06-12 — panic hook writing a log, version + recent log ring, RFD popup dialog)* — panic hook writing a log (version + recent log ring) plus a dialog pointing to Discord/issues.
  Touches: `main.rs`, `src/crash_reporting.rs` · Design: roadmap §9 · Needs: —

- [ ] **TI-4 · Settings completeness** — resolution/fullscreen, separate music/SFX volume buses, key rebinding if feasible.
  Touches: `src/menus/settings.rs`, `audio.rs` · Design: roadmap Phase 1 · Needs: —

- [ ] **TI-5 · Concurrent-mission cap** — introduce the cap (~3 for the demo; today dispatch is unbounded), surface it in party select/mission board, leave a hook for War-Room-tier growth.
  Touches: `party_select.rs`, `missions.rs`, `buildings.rs` · Design: scale doc, roadmap Phase 1 · Needs: —
  Decision inside: which building/tier gates the cap.

- [ ] **TI-6 · String externalization** — move user-facing strings to a lookup table now; EFIGS localization is an EA-era decision but retrofitting strings is the painful part. Mechanical, high-volume, ideal LLM work. The earlier the cheaper — slot any time after the fun-proof milestone.
  Touches: every screen, feed templates · Design: roadmap Phase 5 note · Needs: UX-1 (so feed templates externalize once)

- [ ] **TI-7 · Performance baseline** — tracy profiling pass, a bench for sim ticks, budget written down for 50+ concurrent missions (the EA scale story), top offenders fixed.
  Touches: profiling hooks (already noted in `Cargo.toml`), hot sim paths · Design: roadmap §9 · Needs: CB-8 (profile the real sim)

## ST — Steam Integration

- [ ] **ST-1 · Steamworks spike** — verify `bevy-steamworks`/`steamworks-rs` against Bevy 0.18 (the plugin historically lags), wrap behind our own feature-flagged plugin, smoke-test init + overlay. **Do this spike early (Phase 2) — it de-risks everything else in ST.**
  Touches: `Cargo.toml`, new `src/steam.rs` · Design: roadmap Phase 4 · Needs: app credentials (appendix: Steamworks account)

- [ ] **ST-2 · Steam Cloud saves** — Auto-Cloud path configuration for the RON saves (the dirs-based path maps cleanly); verify sync both directions.
  Touches: Steamworks config, `save.rs` docs · Needs: ST-1, TI-2

- [ ] **ST-3 · Achievements** — 10–20 wired to chronicle/stat events (first legendary, first hero lost, guild milestones). The LLM drafts the list; Steamworks holds the definitions.
  Touches: `steam.rs`, chronicle hooks · Needs: ST-1, HR-1

- [ ] **ST-4 · Overlay + hardware verification** — test matrix for the Steam overlay against wgpu/Vulkan quirks on real hardware; fix what surfaces. Part manual, but investigation and fixes are LLM-assisted.
  Needs: ST-1

- [ ] **ST-5 · Steam Deck pass** — UI scaling at 1280×800, controller/touch input mapping, Deck performance check. Likely splits into input-map and layout sessions ("Verified/Playable" badge meaningfully helps sales).
  Touches: UI layout constants, input handling · Design: roadmap Phase 4 · Needs: TI-4

- [ ] **ST-6 · Demo build flavor** — feature-flagged demo build (FT-2 gating baked in) as a separate Steam appid/depot, with build scripts.
  Touches: `Cargo.toml` features, CI · Needs: FT-2, ST-1

- [ ] **ST-7 · Depots, branches, upload CI** — Windows + Linux depots from the existing CI, `default`/`beta`/`internal` branches, steamcmd upload automation.
  Touches: CI workflows · Needs: ST-1

- [ ] **ST-8 · Demo telemetry + feedback** — in-game feedback link and Discord invite; minimal anonymized funnel counters (mission 1 finished, session length), opt-out respected.
  Touches: UI, tiny telemetry module · Design: roadmap Phase 3 · Needs: —

---

## Order Map

| Milestone | Roadmap phase | Chunks | Gate |
|---|---|---|---|
| **M0 Foundations** — start now, all parallel | 0 | TI-1, TI-2, TI-3, UX-1 | clean clone builds; saves are safe to evolve; missions narrate |
| **M1 Fun-proof** — the critical path | 1 | CB-1 → CB-2 → CB-3 → CB-4 → CB-5, then CB-6 · UX-2, CT-1 alongside | *watching one mission is fun, hands-off* (roadmap risk #3 — playtest the spectating, not the managing) |
| **M2 Demo-complete** | 1 | CB-7, CB-8 · CT-2–CT-7 · HR-1–HR-5 · RS-1–RS-3 · FT-1, FT-2 · UX-3, UX-4 · TI-4, TI-5 (TI-6 whenever) | a stranger plays 30–60 min unguided and wants more; crash-free 1-hour sessions |
| **M3 Store-ready** | 2 | ST-1 (spike early) — rest is human-led + LLM-drafted copy | "Coming Soon" page live |
| **M4 Fest-ready** | 3 | ST-6, ST-8, playtest-driven fixes | demo shipped to Next Fest (Feb 2027, one shot ever) |
| **M5 EA-launch** | 4 | ST-2, ST-3, ST-4, ST-5, ST-7 · TI-7 · balance pass | EA live, Q2 2027 |

**Critical path:** UX-1 → CB-1…CB-5 → (CB-6/7, content fan-out) → CB-8. Everything outside CB can proceed in parallel whenever its `Needs:` are met. If a session is short on energy, CT-3, TI-6, or appendix drafting are always safe picks.

---

## EA → 1.0 Themes (deliberately not chunked yet)

From vision §6/§9 and the GDD, to be chunked against EA feedback: staff NPCs + exception queue + observation tiers (the scale funnel to 1000 missions) · class promotions · remaining mission types (Hunt, Escort, Gather, Exploration, Raid) · world/region map + chapter missions + Five Crowns ticker · The Hollow · set items & Old Gods · funerals + Memorial Wall · trait pool to 20–30 · building levels past 3 · prestige/NG+.

---

## Human-Led Tasks (the LLM can't do these — but can help)

| Task | Phase | Where the LLM helps |
|---|---|---|
| Entity/LLC decision, Steamworks account, $100 fee, tax docs | 0 | checklist prep only |
| "Guild Forge" name/trademark check | 0 | search compilation, risk summary |
| Asset license audit | 0 | **mostly LLM**: inventory every asset + license, flag problems |
| Art: pack-vs-commission decision, then the commission | 1 | brief-writing, style-ref collation; integration is CT-4/HR-4 |
| Capsule/key art commission | 2 | brief-writing |
| Music + SFX licensing or commission | 1 | requirements list; wiring is UX-4 |
| Trailer | 2 | shot-list from the GIF-able moments (GDD §10.3) |
| Store page copy, tags, EA questionnaire | 2 | **drafts all of it** |
| Devlogs + community cadence | 2+ | **drafts all of it** |
| Playtest rounds, Next Fest registration, pricing | 3–4 | feedback triage, comparable-pricing research |

---

*Living document. Tick boxes with dates. Revisit at each milestone gate alongside the roadmap.*
