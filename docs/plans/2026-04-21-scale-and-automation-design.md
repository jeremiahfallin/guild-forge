# Scale & Automation — Design Direction

**Date:** 2026-04-21
**Status:** Design direction (not yet an implementation plan)
**Scope:** Guild-wide scaling (10s → 1000s of concurrent missions) and the automation funnel that makes it playable.

---

## 1. Context

Early playtest feedback (conversation with Jebbins, 2026-04-17) and internal reflection surfaced that the current game has the right core loop — dispatch a party, watch them crawl a dungeon, collect loot — but lacks depth in three directions: combat/classes, equipment, and *what happens when the guild grows large*. This document addresses the third.

The core creative bet is: **the late-game fantasy is running an adventuring empire, not micromanaging a five-person party forever.** The game should funnel outward (à la *Big Ambitions*) — each stage offloads work the player has mastered and replaces it with higher-order decisions. The botwatch fantasy scales from "watch one dungeon" to "oversee a guild that runs itself, step in where it matters."

This design direction also resolves an existing GDD tension: "Meaningful Risk" via permadeath is replaced with a softer **Missing → Rescue** mechanic, which preserves emotional stakes without making 1000-mission scale devolve into constant roster wipes.

---

## 2. Goals

- Support 1000+ concurrent missions without overwhelming the player.
- Preserve the current dispatch-and-watch loop as a thing the player *chooses* to do, not a thing they *must* do.
- Give the player a reason to get attached to individual heroes without forcing attachment at scale.
- Make each automation unlock a legible, satisfying moment ("you no longer have to sort loot").
- Keep combat, equipment, and class-depth work (separate threads) compatible with this structure.

## 3. Non-Goals (this doc)

- Combat feel / turn structure / "fancy moves" — separate design thread.
- Equipment slot system (Melvor-style) — separate design thread.
- Class differentiation — separate design thread.
- Exact numerical balance of XP / gold / difficulty curves.
- Save/load changes (the existing save system covers the data here with minor additions).

---

## 4. Favoriting & Personal Management

Two orthogonal per-hero flags:

### 4.1 `Favorite`
UI prominence only. A favorited hero is:
- Pinned at the top of the roster.
- Highlighted in mission feeds and event logs.
- Surfaced as a high-priority event in the Field Report dashboard on level-up, rare loot, injury, or Missing status.

Favoriting does not change game rules. It is a following / attention mechanic.

### 4.2 `PersonallyManaged`
Excludes the hero from the Dispatcher's auto-assign pool. The player hand-picks their missions.

Rationale: the original dispatch-and-watch loop never disappears. It becomes *opt-in*. A player can run a four-hero strike team by hand forever while the rest of the guild runs itself, or delegate everything and only curate at exceptions.

### 4.3 Combinations
- **Favorite + PersonallyManaged** (typical): your A-team — you pick their missions and get notified on every meaningful event.
- **Favorite only**: "I want to follow this hero's career, but the manager still deploys them." Good for mid-tier fan-favorites.
- **PersonallyManaged only** (rare): "Don't let the manager send this trainee anywhere — I'm raising them."
- **Neither** (the mass): the hero is a faceless member of the Dispatcher's pool.

---

## 5. Automation Funnel (Staff Roster)

Automation arrives as **named NPC staff**, each owning one domain. Staff are unlocked via guild upgrades and/or reputation thresholds. Each has quality tiers (`Rookie → Journeyman → Veteran → Master`) purchased with gold over time.

| Staff | Domain | Effect |
|-------|--------|--------|
| **Dispatcher** | Mission assignment | Auto-assigns non-managed heroes to available missions within its policy limits. Higher tier = better party composition, avoids mismatches, respects fatigue. |
| **Quartermaster** | Loot & gear | Auto-equips upgrades, sells junk below a quality threshold, stockpiles materials. Higher tier = smarter upgrade decisions (considers trait synergies, stat priorities). |
| **Recruiter** | Tavern hiring | Auto-hires from the tavern pool within a quality/cost rule. Higher tier = better talent judgment, fewer bad picks. |
| **Infirmary Steward** | Rest & recovery | Auto-rests fatigued or injured heroes, shortens Missing timers, triages rescue priority. Higher tier = faster recovery, higher rescue success rate. |

### 5.1 Staff are flavored, not mechanical slots
Each staff member is a named NPC with a portrait, a short bio, and a personality quirk reflected in their log output ("Hild the Quartermaster grumbles about the state of your reserves"). This is cheap content that massively improves charm.

### 5.2 No tunable sliders at first
Initial design: staff behavior is determined by their tier alone. No per-staff policy sliders. If playtesting shows the defaults are wrong too often, we can add sliders later (see Open Questions).

### 5.3 Staff quality affects signal-to-noise
A Rookie Dispatcher generates *more* exception-queue alerts (failed missions, mis-assigned parties) because they make more mistakes. A Veteran generates fewer. This is both a gameplay loop (upgrade to reduce noise) and a natural difficulty curve that lets early-mid-game feel chaotic and late-game feel like stewardship.

---

## 6. Scale System

Two interlocking caps:

- **War Room** (existing GDD concept): raw concurrent-mission ceiling. The maximum number of missions that can be active at once, regardless of who assigned them. Upgraded with gold + materials.
- **Dispatcher tier**: auto-managed concurrent-mission ceiling. The maximum number of missions the Dispatcher will run in parallel. Always ≤ War Room.

This means:
- A big War Room + bad Dispatcher = high ceiling, mostly empty. You must hand-dispatch to fill it. Rewards direct play.
- Small War Room + good Dispatcher = low ceiling, always full automatically. Rewards delegation.
- Both high = late-game empire.

### 6.1 Pacing Targets

| Stage | War Room cap | Auto-managed cap | Feel |
|-------|--------------|------------------|------|
| Early | 3 | 0 (no Dispatcher) | Hand-dispatch only, named heroes, learn the loop. |
| Mid | ~50 | ~40 | Small strike team + growing auto-pool. Operations Wall becomes interesting. |
| Late | ~500 | ~495 | A-team plus a guild that basically runs itself. Field Report is the main screen. |
| Stretch | 1000–2000 | nearly all | Guild empire. Exception curation is the gameplay. |

Numbers are ballpark — actual balancing is deferred.

### 6.2 Simulation cost
Per GDD §8.4, unobserved missions already tick in logic-only mode. At 1000+ missions we likely need to:
- Stagger mission ticks across frames (not every mission every frame).
- Consider a coarser simulation mode for unobserved, un-escalated missions.
- Batch event emissions and coalesce in the exception queue.

Implementation detail, flagged here as a known concern.

---

## 7. Observation UI — Three Tiers

At 1000-mission scale, the "wall of tiny live renders" is physically impossible as a single view. The observation experience is tiered by zoom.

### 7.1 Tier 1 — Field Report Dashboard
The default view once the guild is large. Contains:

- **Aggregate stats**: active missions, today's gold, today's XP, total recruits, total losses.
- **Exception queue**: a scrollable, filterable feed of events that want attention. Priorities:
  1. Favorite hero events (level-up, death-equivalent Missing status, rare loot).
  2. Rescue-window opportunities (a Missing hero still in the rescue window).
  3. Staff alerts (Dispatcher flagged a mission as high-risk; Recruiter found an exceptional tavern candidate).
  4. Guild milestones (reputation thresholds, upgrades complete).
  5. Batched mass events ("14 heroes lost in Region 4 today" as a single summary line).
- Click any exception → jump to the relevant hero / mission / screen.

Staff quality affects signal-to-noise here: a Rookie surfaces more noise, a Master surfaces only the decisions that truly need you.

### 7.2 Tier 2 — Operations Wall
The botwatch wall-of-miniatures view, but **always filtered**. Filters include: favorites only, region, difficulty range, currently in combat, currently in boss fight, managed heroes. Caps display at ~20–40 mini-renders at once.

This is the "I want to watch" view. In early/mid game when the whole guild fits in one filter, this is the natural main screen. In late game, it's where you go when you want to enjoy the dungeon-crawl fantasy.

### 7.3 Tier 3 — Mission View
Unchanged from today. Click a mission → full top-down tile rendering, the current observation experience. Speed controls still live here.

---

## 8. Missing → Rescue (replaces permadeath)

Permadeath is removed as a binary outcome. When a hero would previously have died:

1. They enter `Missing` status with a timer (`N` minutes).
2. A **rescue mission** is auto-generated and surfaced in the exception queue.
3. If another party is dispatched and reaches them before the timer expires → the hero is recovered, usually with injuries / gear loss.
4. If the timer expires → the hero is lost permanently.

### 8.1 Why this is better at scale
- Preserves the emotional weight of "my hero is in danger."
- Gives the player *agency* in the moment it matters most — a choice, not an event they read about after the fact.
- Scales well: at 1000 missions, mass casualties become "14 rescues pending in Region 4" on the Field Report, and the player can prioritize which favorites get saved first.
- Non-favorites usually aren't rescued, which feels correct for the faceless mass without being cruel.

### 8.2 Tuning knobs
- `Missing` timer length (base 10–30 min? scales with hero tier?).
- Rescue mission difficulty (usually harder than the mission that caused the loss — you're going into the same trouble).
- Infirmary Steward tier lengthens the timer and auto-dispatches rescue for managed heroes.

### 8.3 GDD impact
The "Meaningful Risk" design pillar (§2.3) is rewritten: risk = time lost, gear lost, rescue cost, morale hit. Not binary death. The pillar's intent (decisions have weight) survives.

---

## 9. How This Composes With Other Threads

- **Combat depth (class differentiation, fancy moves)**: happens *inside* the Mission View (Tier 3). This design doesn't block or shape it.
- **Equipment (Melvor-style slots)**: fits naturally — the Quartermaster's job becomes much more interesting once gear choice is non-trivial. Slotted gear makes the Quartermaster tier progression legibly valuable ("Rookie auto-equips by item level; Master considers set bonuses and trait synergies").
- **Hero growth rates (current work branch)**: unaffected; favorite-worthy heroes are often the ones with lucky rolls, which gives growth-rate RNG a gameplay consumer.

---

## 10. Phased Implementation Sketch

Rough ordering (not a plan, just a feel for dependencies):

1. **Favorite & PersonallyManaged flags** — components, UI pins, event-feed highlighting. Cheap, useful immediately.
2. **Missing → Rescue mechanic** — replace permadeath. Also enables testing the exception queue.
3. **Field Report Dashboard (Tier 1)** — initially minimal, just a stats panel + event feed. Grows with more staff.
4. **Dispatcher staff (first one)** — unlocks the auto-managed cap. Validates the split-cap model.
5. **Operations Wall (Tier 2)** — filtered multi-mission render. Requires cheap mini-render mode.
6. **Remaining staff (Quartermaster, Recruiter, Infirmary Steward)** — each a small, flavored addition.
7. **Staff tiers & upgrade sinks** — long-term gold sink; lands late in the sequence.
8. **Simulation perf work** for 1000+ mission scale — done when scale actually demands it, not pre-emptively.

---

## 11. Open Questions

- How long should the `Missing` rescue window be? Fixed, or scaling with hero level / staff tier / region distance?
- Should staff be *hired* from a pool (like recruits) or *unlocked* via guild upgrades (like buildings)? Leaning toward unlocked-then-upgraded, with named NPCs tied to the slot.
- At what point do tunable staff sliders become necessary? Plan: ship without, add if playtest shows it.
- How much exception-queue configurability does the player want? Fixed priorities vs. a "what pings me" preferences panel.
- What's the Operations Wall's render budget? 20? 40? Depends on mini-render cost.
- Does the Dispatcher ever *accept/reject* missions from the mission board, or only assign heroes to pre-accepted missions? (Leaning: only assigns, player still curates which missions exist.)

---

## 12. Deferred / Next Threads

Once this direction is validated, the three other threads from the 2026-04-17 conversation become natural follow-ups, in rough priority order:

1. **Equipment system** (Melvor-style slots, gear as meaningful choice) — gives the Quartermaster something real to do.
2. **Class differentiation** (classes actually do different things in combat) — makes the Dispatcher's party-composition judgment matter.
3. **Combat feel** (walk-and-hit vs. turn-based back-and-forth; "fancy moves") — polish on the Mission View experience.
