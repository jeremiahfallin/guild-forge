# Guild Forge — Path to Steam Release

> **Created:** 2026-06-09
> **Strategy:** Demo → wishlists → **Early Access** → 1.0 (per GDD §10.2)
> **Working assumption:** solo dev, LLM-assisted, part-time pace. Dates are targets, not promises.
> **Execution companion:** `docs/steam-release-chunks.md` — the engineering work broken into LLM-tackleable chunks.

---

## 1. Where the Game Is Today

Grounded in the repo as of June 2026 (~11k LOC, Bevy 0.18):

**Done / working**
- Core loop: recruit → equip → train → dispatch → watch → results (GDD Phases 0–4 essentially complete)
- Mission sim: procedural dungeons, tileset rendering, pathfinding, autonomous combat, sequential missions
- Guild layer: economy, buildings, recruiting, training, reputation, equipment, materials
- Hero depth: stats, classes, traits, growth rates, favorites, Missing → Injured status pipeline (replaces hard permadeath)
- Save/load with offline time bank
- Full screen flow: splash, title, menus, settings, pause, credits
- CI + release workflows from the Bevy 2D template

**Not done (the gap between "playable" and "shippable")**
- **Art:** placeholder sprites (6 idle sprites, one tileset, template ducky.png/splash). No animations beyond idle, no hero portraits, no guild hall visuals, no VFX/juice
- **Audio:** template music (Kevin MacLeod tracks) and stock SFX — must be replaced or properly licensed/credited
- **Content volume:** one tileset/biome, ~5 enemy types, limited mission/gear/class variety
- **Design depth flagged in own docs:** combat feel, equipment slot system, scale & automation funnel (2026-04-21/22 design docs are direction, not implementation)
- **Steam:** zero Steamworks integration, no store presence, no marketing assets
- **Dependency risk:** `bevy_declarative` is a local path dep (`../bevy_declarative`) — must be vendored into the repo, published, or pinned via git URL before anyone else can build the game

---

## 2. Phase Overview

| Phase | Goal | Target |
|---|---|---|
| 0 | Business & Steamworks setup | Jun–Jul 2026 |
| 1 | Demo-quality vertical slice | Jul–Oct 2026 |
| 2 | Store page live ("Coming Soon") | Oct 2026 |
| 3 | Public demo + Steam Next Fest | Feb 2027 fest |
| 4 | Early Access launch | Q2 2027 |
| 5 | EA → 1.0 | 6–12 months in EA |

Key external constraints driving the dates:
- $100 Steam Direct fee per app, recoupable after $1,000 adjusted gross revenue
- ~30-day wait between paying the fee and being able to release anything
- Store page must be public ≥ 2 weeks before launch (in practice: months, for wishlists)
- Next Fest runs Feb/Jun/Oct; a game may participate **once, ever**, must be unreleased, and needs a public demo — this is the single best free visibility event, so don't burn it early. Oct 2026 is too tight; **Feb 2027** is the realistic slot.

---

## 3. Phase 0 — Business & Steamworks Setup (now)

Cheap, slow-moving, paperwork-bound tasks. Start immediately so nothing blocks later phases.

- [ ] Decide entity: sole proprietor vs. LLC (consult an accountant; an LLC is common for liability + clean revenue separation). *Not legal advice — verify locally.*
- [ ] **Name check:** search USPTO/Steam/itch for "Guild Forge" conflicts before investing in branding. It's a generic-sounding name; collisions are likely. Decide now whether to keep or rename — renaming after the store page exists is costly.
- [ ] Create Steamworks partner account: identity verification, tax docs (W-9), bank details
- [ ] Pay the $100 app fee (starts the 30-day clock; the clock is cheap, start it early)
- [ ] Fix the `bevy_declarative` path dependency: publish to crates.io, move into a workspace, or pin to a git rev
- [ ] Audit asset licenses: list every third-party asset (fonts, music, SFX, sprites) with license + attribution requirements. Replace anything non-commercial or unclear
- [ ] If any AI-generated assets ship, note them — Steam requires AI-content disclosure on the store page

---

## 4. Phase 1 — Demo-Quality Vertical Slice (Jul–Oct 2026)

Goal: a stranger can download the game, play 30–60 minutes unguided, and want more. This is the bar for both the demo and the Next Fest build.

**Design depth (from your own design docs — pick what the demo needs, defer the rest)**
- [ ] Combat feel pass (2026-04-22 doc): hit feedback, ability variety, readable fights — this is the "botwatch" signature feature, it must be watchable
- [ ] Equipment slot system (Melvor-style direction) — at least enough that loot decisions feel interesting
- [ ] First slice of the automation funnel (2026-04-21 doc) — demo can cap at ~3 concurrent missions; full scaling is EA content
- [ ] Rescue missions (scale doc §8): a wipe auto-generates a recovery mission to reach Missing heroes before the timer expires — completes the already-built Missing → Injured loop and is the demo's emotional hook *(demo scope, decided 2026-06-11)*

**Content minimums for a demo**
- [ ] 2 biomes/tilesets (have 1)
- [ ] 8–10 enemy types with distinct behavior (have ~5)
- [ ] 3–4 classes that play visibly differently
- [ ] ~2 hours of progression, soft-capped so the demo ends with a hook
- [ ] First-time-user experience: tutorialization or guided first mission — currently none

**Art & audio replacement**
- [ ] Commission or buy a coherent 16-bit pixel art set: heroes (with walk/attack/death anims), enemies, tileset, guild hall, UI skin, hero portraits. Budget realistically: $2–6k commissioned, or $100–500 from asset packs (CraftPix, itch.io) accepting less uniqueness
- [ ] Replace template music with licensed/commissioned tracks (3–5 loops); replace stock SFX
- [ ] Key capsule art is a **separate, critical** commission (~$200–1,000) — it's the single highest-leverage marketing asset on Steam

**Stability**
- [ ] Crash-free 1-hour sessions on Windows + Linux; save-corruption safeguards (versioned saves, backup-on-write)
- [ ] Settings completeness: resolution/fullscreen, volume sliders, key rebinding if feasible

---

## 5. Phase 2 — Store Page Live (Oct 2026)

The "Coming Soon" page should go up **months** before launch — wishlists accumulate from day one and drive launch-week visibility.

- [ ] Capsule images (all required sizes), 5+ real screenshots, short + long description
- [ ] Trailer (30–60s, gameplay-first; the GIF-able moments from GDD §10.3 — clutch kills, rare drops, party wipes — are the script)
- [ ] Tags: Management, Idle, Auto-battler, RPG, Indie, Pixel Graphics — tags drive Steam's discovery algorithm, research comparable games' tags (Loop Hero, Melvor Idle, Guild Master)
- [ ] Pass Valve's store page review (1–5 business days; placeholder-quality assets are a common rejection reason)
- [ ] Mark Early Access intent and draft the EA questionnaire answers (why EA, how long, price change plans, current state — be specific and honest)
- [ ] Start a devlog cadence: posts on Steam community hub + one external channel (Bluesky/Reddit r/incremental_games + r/bevy, TIGSource, etc.). The botwatch niche overlaps idle/incremental communities — that's the beachhead audience

---

## 6. Phase 3 — Demo & Next Fest (Feb 2027)

- [ ] Ship the demo as a separate Steam demo app, gated version of the Phase 1 slice
- [ ] Register for **February 2027 Next Fest** (registration typically closes weeks before; watch the Steamworks docs for deadlines). One shot ever — only enter when the demo is good
- [ ] Instrument the demo: anonymized funnel telemetry or at minimum a feedback link + Discord invite in-game
- [ ] Run 2–3 closed playtest rounds before the fest (Steam Playtest feature is free and separate from the demo)
- [ ] During fest week: livestream on the store page (big visibility multiplier), daily devlog, respond to every piece of feedback
- [ ] Post-fest: triage feedback into "fix before EA" vs "EA roadmap" vs "won't do"

**Wishlist sanity check:** common wisdom is ~5–10k wishlists for a viable indie EA launch. If post-fest numbers are far below that, extend the marketing runway rather than launching into silence — the launch date is movable, the one-time fest is not.

---

## 7. Phase 4 — Early Access Launch (Q2 2027)

**Steamworks integration**
- [ ] Integrate the `steamworks`/`bevy-steamworks` crate — **verify Bevy 0.18 compatibility first**; the plugin historically lags Bevy releases. Fallback: use the raw `steamworks-rs` bindings behind your own plugin (you already wrap things via bevy_declarative, same pattern)
- [ ] Steam Cloud for saves (your RON save in a dirs-based path maps cleanly to Auto-Cloud path config)
- [ ] Achievements: 10–20 at launch (first legendary drop, first hero lost, guild level milestones)
- [ ] Steam Overlay verification (known wgpu/Vulkan quirks — test early, on real hardware)
- [ ] Optional but cheap wins: Rich Presence ("Watching 12 missions"), trading cards later

**Builds & QA**
- [ ] Depots: Windows + Linux native (you have CI for this); macOS only if you can test it
- [ ] **Steam Deck verification pass** — a management game with observable missions is a great Deck fit; test UI scaling at 1280×800 and controller/touch input. "Verified" or "Playable" badge meaningfully helps sales
- [ ] Branch setup: `default` (public), `beta` (opt-in), `internal`
- [ ] Build review by Valve (runs alongside the store review; submit ≥ 1–2 weeks before target date)

**Launch decisions**
- [ ] Price: comparable EA titles in the idle/management niche run $10–20. Plan the EA→1.0 price increase up front and say so on the page
- [ ] Launch discount (10–15% is conventional)
- [ ] EA roadmap graphic on the store page: what's in now, what's coming, rough cadence
- [ ] Press/creator outreach 2–3 weeks ahead: keymailer/Woovit + direct emails to idle-game YouTubers/streamers (the genre has dedicated channels with hungry audiences)

---

## 8. Phase 5 — Early Access → 1.0 (6–12 months)

- Monthly-ish content updates beat rare big ones for Steam's algorithm and community trust
- EA content backlog (from existing design docs): full automation funnel to 1000+ concurrent missions, raid missions, story/lore missions, more biomes/classes/traits/gear tiers, crafting
- 1.0 criteria: roadmap delivered, stable, balanced economy/XP curves (data-driven tuning via the RON files), Deck verified, localization decision made (even just EFIGS for UI text — plan string externalization early, retrofitting is painful)
- 1.0 launch is a second marketing beat: price increase, trailer refresh, creator outreach round two

---

## 9. Cross-Cutting Checklist

**Technical debt to clear before strangers run the game**
- [ ] `bevy_declarative` dependency (see Phase 0)
- [ ] Save format versioning + migration story (EA players' saves must survive updates — breaking saves is the #1 EA review killer)
- [ ] Crash reporting (even just a panic hook writing a log + dialog pointing to Discord)
- [ ] Performance budget: the scale design implies hundreds of off-screen sims — profile early (tracy hooks are already noted in Cargo.toml)

**Budget (rough, beyond your time)**
| Item | Cost |
|---|---|
| Steam Direct fee | $100 (recoupable) |
| Art (commissioned, coherent set) | $2,000–6,000 |
| Capsule/key art | $200–1,000 |
| Music + SFX | $300–1,500 |
| Trailer (DIY with OBS/DaVinci) | $0–500 |
| LLC + misc legal/accounting | $200–800 |
| **Total** | **~$3–10k** |

**Recurring marketing habits (low effort, compounding)**
- One GIF/short clip per week from dev builds — the game's premise is inherently GIF-able
- Devlog per milestone; Discord from Phase 2 onward
- Track wishlists weekly; they're the main health metric pre-launch

---

## 10. Biggest Risks to This Plan

1. **Art pipeline** (GDD §11 already flags it) — it's the longest pole in Phase 1 and gates everything downstream. Decide pack-vs-commission in the next month.
2. **Burning Next Fest early.** If the Feb 2027 demo isn't genuinely fun, slip to Jun 2027. One shot.
3. **Combat watchability.** The signature feature ("botwatch") is also the flagged weak point. Playtest the *spectating* experience specifically, not just the management loop.
4. **Bevy churn.** Pin 0.18 through the demo; only upgrade between marketing beats.
5. **Scope creep** — the scale-and-automation vision is EA content, not demo content. The demo sells the fantasy with 3 missions, not 1000.

---

*Living document. Revisit after each phase gate.*
