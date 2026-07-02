# Guild Forge — The Finished Product

> **Created:** 2026-06-09 · **Revised:** 2026-06-11 (claims re-verified against the working tree)
> A brainstormed vision of the complete game, grounded in the current code, the GDD, and the April design docs.
> Each feature is tagged: **[Built]** exists in src/, **[In progress]** partially implemented in the working tree, **[Designed]** has a design doc but no code, **[New]** brainstormed here.

---

## 1. The Finished Game in One Paragraph

You run an adventurer's guild that grows from a leaky tavern with three nobodies into an empire running a thousand simultaneous expeditions. Early on you know every hero by name and watch every fight; by the end you're a guildmaster reading field reports, stepping in only when your favorite veteran goes missing in a cursed crypt and you have one hour to mount a rescue. The game is a story generator disguised as a management sim: heroes with personalities play out little D&D sessions on their own, and your job is to build the machine that lets a thousand of those stories happen — while still caring about a few of them.

---

## 2. Why It Feels Bare-Bones Right Now (The Diagnosis)

Reading the code, the skeleton is all there — recruit, equip, dispatch, watch, loot, upgrade. The problem isn't missing systems. It's that **nothing surprising ever happens, and nothing that happens is legible or memorable.** Specifically:

1. **Combat is unreadable mush.** Real-time walk-up auto-attacks (`mission/combat.rs` — still the shipped default; a first turn-based mode now exists behind a dev-only F1 toggle, see §3) blur together. There are no visible decisions, no beats, no "did you see that?!" Every class is a different damage number wearing a different sprite. Your own combat-feel doc already nailed this — it's the single biggest fun deficit.
2. **Zero variance between runs.** Three mission templates, five enemy stat blocks, one tileset, no mission events, no modifiers. The second Goblin Cave is identical to the first. Surprise is the raw fuel of "one more mission," and the game currently has none.
3. **Heroes don't accumulate stories.** Six of seven traits already steer mission AI (`mission/ai.rs` score multipliers — Brave attacks more, Cautious flees sooner, Greedy detours toward treasure, Loner fights better alone, Cursed swings harder — plus Lucky's +3 on rolls; only Leader is inert), but none of it is *legible* in the real-time blur, so it may as well not exist. There are no portraits, no mission history, no kill counts, no scars. A level 12 veteran and a fresh recruit differ only in numbers, so there's nothing to get attached to — and attachment is the engine of the whole "watch them grow" pillar.
4. **No drama in outcomes.** Wins and losses arrive as toasts. No combat log, no close calls you can see, no rising tension. The Missing/Injured system (which is good!) fires without ceremony.
5. **Short goal horizon.** Buildings cap at level 3, gear paths are shallow, and reputation's only job is gating two of the three missions. After ~2 hours there is nothing left to want.

The fix is not "more features." It's three properties layered onto what exists: **variance** (no two missions alike), **legibility** (you can see and understand the cool thing that happened), and **attachment** (heroes accrue identity over time). Almost everything below serves one of those three.

---

## 3. The Core: Watchable Combat **[Designed → In progress]**

*From the 2026-04-22 combat-feel doc — this is the foundation everything else sits on, and it should be built first.*

> **Status (2026-06-11):** a first slice exists in `src/mission/sequential.rs` (uncommitted): a per-mission turn queue with move-or-act turns, wired into `mission/mod.rs` behind `SimulationMode::Sequential`. It is currently **dev-only** — F1 toggle in `dev_tools.rs`, compiled out of release builds, default still the real-time mode — and it drifts from the design below in three ways to resolve: initiative is a deterministic speed sort with seeded tie-breaks (not d20 + DEX rerolled each round), a move turn covers up to `MoveRange` (~3) tiles (not one), and everything ticks at a flat 2 Hz (no brisk-exploration vs. slow-combat tempo split). No abilities, cooldowns, or signature moves yet — though `classes.ron` already ships unused `starting_abilities` data as a hook.

- **Encounter-based turns on the tile grid.** Exploration and combat are one simulation at two tempos: brisk turn-ticks while walking (4–8/sec), slow deliberate beats in combat (1–2/sec). No camera change, no mode switch — the tempo shift *is* the signal.
- **Move-or-act.** Each turn an entity moves one tile *or* takes one action. This one rule makes melee vs. ranged real: Warriors spend turns closing while Mages cast every round. Party composition becomes a genuine decision.
- **Three-layer ability kits.** Basic attack (rhythm) → 2–3 short-cooldown abilities (color, fire 1–3 times per fight) → one signature move per encounter (Meteor, Mass Heal, Shadowstep — the highlight-reel moment the AI visibly *saves* for the right time).
- **Initiative rerolled every round** (d20 + DEX) so each round has a micro-story: sometimes the mage nukes first, sometimes the ogre clubs her first.
- **Traits as AI personality.** Brave rushes the signature; Cautious retreats below 30% HP; Greedy spends a combat turn opening a chest; Loner fights worse near allies. The score-multiplier half of this already runs in `ai.rs` today; the missing half is the legible turn-based stage plus signature/chest behaviors for the multipliers to point at. Two Brave Warriors and two Cautious Warriors are *different people playing the same class* — this is where emergent stories come from.

**Why this is the fun unlock:** the game's signature feature is watching. Watching is only fun when you can read what's happening and occasionally be surprised by it. Turn beats give legibility; signature moves + traits give surprise.

---

## 4. Heroes Worth Caring About

### 4.1 Identity **[Built, thin → expand]**
- **[Built]** Stats, classes (5), traits (7), growth rates, fatigue, Favorite/PersonallyManaged flags.
- **[New] Portraits** — even a small generated/layered pixel portrait set transforms attachment. Faceless heroes can't be loved.
- **[New] Expanded trait pool (20–30)** with at least one visible combat or exploration behavior each. Traits are the cheapest story generator in the game.
- **[New] Quirk lines** — one flavor sentence per trait combo, shown on the roster card ("Refuses to sleep indoors. Counts her arrows twice.").

### 4.2 History — the Chronicle **[New]**
Every hero accumulates a record: missions run, kills, near-deaths survived, signature moves landed, dungeons cleared, rescues performed *and received*, lifetime gold earned. Show it on the hero sheet as a scrollable career timeline ("Day 12: survived the Skeleton Crypt wipe — sole survivor").
- **Epithets**: milestone-triggered titles — *Slimebane*, *the Twice-Lost*, *Hundred-Mission Hera*. Auto-granted, displayed everywhere their name appears.
- **Scars & mementos**: a survived Missing episode leaves a permanent cosmetic mark and a log entry. Veterans should *look* and *read* like veterans.
- **Why:** attachment compounds. The roster stops being a stat table and becomes a cast.

### 4.3 Advancement **[Built, shallow → deepen]**
- **[Built]** XP/levels, stat growth, training.
- **[Designed/GDD] Class promotions** — at level cap thresholds, Warrior → Knight or Berserker (GDD §4.2 names only this pair; further splits like Mage → Pyromancer or Chronomancer are **[New]** flavor). Each promotion swaps the ability kit, giving a *visible* change in how the hero fights — the payoff moment for "watch them grow."
- **[New] Veteran perks** — small earned passives from history, not levels ("survived 3 rescues: +10% HP"). History literally makes them stronger.

---

## 5. Missions That Surprise You

### 5.1 Variety **[Built: 3 templates → expand]**
- **[GDD] Six mission types**: Dungeon Crawl **[Built]**, Hunt, Escort, Gather, Exploration, Raid. Each is a different *shape* of watchable content (Hunts are one long boss fight; Escorts are a moving defense; Exploration is low-combat discovery that unlocks the map).
- **[GDD] Modifiers** on generated missions: *Foggy*, *Infested*, *Cursed Ground* (no healing), *Bountiful*, *Trapped*. Modifiers × templates × biomes = combinatorial freshness from data files you already load via RON.
- **[New] Biomes** (target 6+): cave, crypt, forest, swamp, ruins, The Hollow. Each with its own tileset, enemy families, hazards, ambience.

### 5.2 Mid-Mission Events **[New — highest variance-per-effort in the game]**
A random-event system that fires 0–2 times per mission: a hidden chamber, a wandering merchant, a shrine (bless or curse — Greedy heroes always touch it), an ambush, a collapsed floor splitting the party, a rival guild's party racing you to the boss. Events resolve via trait-and-stat checks, and *which hero* triggers them depends on personality. This is the system that makes players tell stories: "my Greedy rogue drank the cursed fountain AGAIN."

### 5.3 Bosses **[Built: stat-block "BossRat" → real bosses]**
Every difficulty-3+ mission ends in a boss with 1–2 telegraphed mechanics (AoE slam you watch heroes scatter from, summoned adds, an enrage timer). Bosses are the encounter-system showcase and the natural GIF moment.

### 5.4 The Combat Log / Mission Feed **[New, cheap, do early]**
A scrolling narrated feed per mission, written in DM voice: "Sera shadowsteps behind the orc — backstab! 17 damage." / "Brom is surrounded. He uses Rallying Cry!" Even before any art improves, a good log makes the *existing* sim feel dramatic. Doubles as the data source for the Field Report later.

---

## 6. The Guild: From Tavern to Empire **[Designed]**

*From the 2026-04-21 scale-and-automation doc — this is the long-game retention layer.*

- **Staff roster**: automation arrives as named NPCs, not checkboxes — the **Dispatcher** (auto-assigns parties), **Quartermaster** (auto-equips/sells loot), **Recruiter** (auto-hires), **Infirmary Steward** (auto-rests, triages rescues). Each has tiers (Rookie → Master), a portrait, and a voice in the logs ("Hild grumbles about the state of your reserves").
- **Staff quality = signal-to-noise**: a Rookie Dispatcher makes mistakes and spams alerts; a Master surfaces only real decisions. Upgrading staff literally makes the game calmer — chaos in the mid-game, stewardship in the late game.
- **War Room scaling**: introduce a concurrent-mission cap and grow it 3 → 50 → 500 → 1000+ (today dispatch is uncapped — `party_select.rs` enforces no limit — so the starting cap is new scope, not a raise). The dispatch-and-watch loop never disappears; it becomes *opt-in* via PersonallyManaged heroes (flag already in the code).
- **Three observation tiers**: **Field Report** dashboard (aggregate stats + prioritized exception queue) → **Operations Wall** (filtered grid of 20–40 live mini-views) → **Mission View** (today's full view). The game's main screen changes as the guild grows.
- **[Built]** Buildings, economy, materials, reputation, recruiting, training, offline time bank — the chassis for all of this already runs. Extend building levels well past 3 and tie unlocks to staff/War Room tiers so the goal horizon stretches to dozens of hours.

---

## 7. Risk, Loss, and Rescue **[Built: statuses → Designed: missions]**

- **[Built]** Missing → Injured pipeline with timers, save persistence, roster countdowns, party wipes marking heroes Missing.
- **[Designed] Rescue missions** — the missing half: a wipe auto-generates a rescue mission to that dungeon; reach your people before the timer expires or lose them forever. Rescuers can find the lost party's gear, their last campsite, their trail. This converts your worst moments into your best stories and gives the player *agency* exactly when the stakes peak.
- **[New] Funerals & the Memorial Wall** — when a hero is truly lost, a short ceremony screen and a permanent memorial in the guild hall listing their epithet and career highlight. Loss should cost a feeling, not just a roster slot.

---

## 8. Loot Worth Watching For

- **[Built]** 3 slots, tiered upgrade paths, crafting via Workshop, material economy.
- **[GDD] Rarity tiers** Common → Legendary with **affixes** that change *behavior*, not just stats: a lifesteal ring, a cleaving sword, Boots of the Gale (+initiative), a cloak that triggers Shadowstep on first hit. Gear that alters what you *see* the hero do feeds the watchability loop and gives the Quartermaster real decisions.
- **[New] Legendary drops are events** — the mission pauses for a beat, the log announces it, the Field Report pins it. Rare loot should be witnessed, not discovered in a menu later.
- **[New] Set items & the Old Gods** — legendary sets tied to lore (GDD §5.4); collecting a set is a long-horizon chase goal and unlocks a secret mission.

---

## 9. World & Narrative **[GDD, unbuilt]**

- **Region map**: missions live on a world map unlocked by Exploration missions and reputation — visible long-term progress (the map itself is a progress bar).
- **Chapter missions**: a loose story spine — the Sundering, the Five Crowns rival guilds, culminating in **The Hollow**, the endgame mega-dungeon raid requiring multiple optimized parties.
- **Rival guilds [New twist]**: the Five Crowns run missions on the same board. You see their results in a weekly standings ticker; occasionally your party meets theirs mid-dungeon (event system). Cheap asynchronous "competition" with zero multiplayer cost.
- **Prestige / New Game+ [GDD stretch]**: retire the guild; legendary heroes become statues granting passive buffs to the next run; unlock new starting traits/classes.

---

## 10. Juice & Presentation **[New — cuts across everything]**

The cheapest fun multipliers, roughly in order of impact per effort:

1. **Combat log with personality** (§5.4) — makes existing sim dramatic with zero art.
2. **Hit feedback**: damage numbers, hit-flash, knockback nudge, death poofs, screen-shake on signatures.
3. **Floating event banners** in mission view ("BOSS ENCOUNTER", "RARE DROP!").
4. **Portraits** (§4.1).
5. **Audio states**: exploration vs. combat vs. boss music layers; ability SFX.
6. **Guild hall that visibly grows** with building levels — walk-around hub instead of menu screens (GDD §4.1).

---

## 11. How It All Loops (The Fun Model)

| Horizon | Loop | What makes it fun |
|---|---|---|
| 30 seconds | Watch a combat round | Legible beats, signature moves, trait quirks |
| 5 minutes | One mission | Events, boss, loot drama, log worth reading |
| 30 minutes | Session | Level-ups, gear decisions, a rescue, a building finished |
| Hours | Arc | Promotions, new biome/region, staff hires, War Room growth |
| Tens of hours | Campaign | Chronicle-rich veterans, story chapters, The Hollow, prestige |

The current build has only the 30-minute row. Every feature above exists to fill in the rows around it — surprise at the small scale, goals at the large scale, attachment binding them together.

---

## 12. If You Only Build Five Things Next

1. **Encounter-based combat + ability kits** (§3) — the foundation; everything else amplifies it. *(First slice already in `sequential.rs` — see the §3 status note.)*
2. **Combat log / mission feed** (§5.4) — do it *first* actually; it makes today's build feel 2× better in a weekend.
3. **Mission events + modifiers** (§5.2) — variance engine, mostly data-driven through your existing RON pipeline.
4. **Hero chronicle + epithets + portraits** (§4.2) — attachment engine.
5. **Rescue missions** (§7) — completes the loss loop you already half-built; the game's emotional peak. *(Demo scope — decided 2026-06-11; the Steam roadmap lists it in Phase 1.)*

These five turn "systems that work" into "stories that happen." The empire-scale layer (§6) is what you build *after* the 5-minute loop is reliably fun — it multiplies fun, it can't create it.

---

*Companion docs: `guild_forge_gdd.md` (original design), `docs/plans/2026-04-21-scale-and-automation-design.md`, `docs/plans/2026-04-22-combat-feel-design.md`, `docs/steam-release-roadmap.md` (release sequencing), `docs/steam-release-chunks.md` (LLM-tackleable execution chunks).*
