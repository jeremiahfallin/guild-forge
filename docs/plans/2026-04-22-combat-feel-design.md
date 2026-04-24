# Combat Feel — Design Direction

**Date:** 2026-04-22
**Status:** Design direction (not yet an implementation plan)
**Scope:** The moment-to-moment combat experience — how fights are structured, how turns resolve, how classes feel different in action. Companion to the 2026-04-21 scale-and-automation design.

---

## 1. Context

The current combat loop is real-time walk-up-and-hit with auto-attacks. Early feedback (Jebbins, 2026-04-17) surfaced that the game's vibe — "watch a D&D campaign by myself" — calls for combat with clear beats, visible "fancy moves," and legible class identity. The existing real-time model blurs actions together and makes classes feel interchangeable apart from damage type.

This design commits combat to an **encounter-based turn system on a tile grid** with one decision per turn (move or act). It is deliberately chosen to make **class identity the strongest mechanical axis in combat** — the original conversation identified this as the most important missing depth.

This design also intentionally unifies combat and exploration into a single turn-based simulation. They become the same loop at different tempos.

---

## 2. Goals

- Every class *plays* visibly different in combat, not just "different damage numbers."
- Each turn is one legible beat — the player can always tell what just happened and why.
- Support the "highlight reel" pacing so a player can casually observe many missions in a short session.
- Combat outcomes trace cleanly to pre-mission decisions (composition, gear, training).
- Create recurring "holy-shit-did-you-see-that" moments without making every turn a spectacle.

## 3. Non-Goals (this doc)

- Exact per-class ability lists, cooldown timings, damage numbers — tuning concern.
- Full enemy bestiary design.
- Art style, animation keyframe authoring.
- Exploration-layer puzzles, traps, interactables (separate thread).
- PvP or competitive balance (the game is single-player).

---

## 4. Encounter-Based Turn Structure

Heroes walk a procedural dungeon. Movement is turn-based (see §6). When an enemy enters a hero's **action range** (or vice versa), an encounter begins:

1. All combatants within engagement range are enrolled.
2. Initiative is rolled for each (§8).
3. Rounds begin. Each round: every combatant acts once, in initiative order.
4. Encounter ends when one side is defeated, flees, or is reduced to zero combatants within range.
5. Surviving heroes resume exploration from where they stand.

The camera does not change mode. There is no pan, no zoom, no scene transition. The dungeon view stays the dungeon view. The tempo slows (signaled by the UI and optionally a musical shift), and action beats become more deliberate.

---

## 5. Unified Exploration + Combat Sim

Exploration and combat share one system:

- Every entity (hero and enemy) exists on the same tile grid.
- Every entity takes turns on the same initiative-sorted loop.
- In exploration, action targets are interactables (chests, doors, objectives) or "move toward assigned goal." No enemies in action range → no combat rules engaged, just continuous progression.
- In combat, action targets include enemies and ability effects. Movement tiles may be restricted by engagement/opportunity rules (deferred — initial version: free movement always).

### 5.1 Tempo as the mode signal
Exploration ticks turns fast — 4–8 turns per real-time second at 1x — so dungeon walking feels brisk. Combat slows to 1–2 turns per real-time second so the player can read each beat. Speed controls (1x/2x/3x, already in the game) multiply both rates.

The tempo change itself signals combat. No discrete mode switch is needed in the simulation; the renderer and audio respond to "any engaged entities in the party" to pick tempo.

### 5.2 Architectural win
One turn-driven simulation system instead of two (separate movement sim + combat sim). The current mission state can evolve cleanly to this — the existing per-mission tick becomes a turn-queue tick.

---

## 6. Grid & Move-or-Act

Combat and exploration happen on the procedural dungeon tile grid. On each entity's turn, they choose *one* of:

- **Move one tile** in a chosen direction.
- **Take one action** — attack, ability, interact.

Not both. This is the key constraint: melee must spend turns closing distance before they swing, ranged/caster classes act every turn from range. This single rule is what makes melee vs. ranged feel mechanically distinct rather than cosmetic.

### 6.1 Action range
Each action has a tile-distance range. Basic melee attacks: adjacent tile. Ranged attacks: class-dependent (Ranger 6 tiles, Mage 5 tiles, etc.). Abilities may extend or shape range (lines, cones, AoE patterns).

### 6.2 Line of sight, obstacles
Deferred. Initial version: any enemy in tile-distance range of an action is a valid target. Walls block movement via the existing dungeon procgen; ranged attacks are not yet blocked by walls in the first pass. Tune in playtest.

### 6.3 No movement budget, no engagement rules (initial)
A hero on an "act" turn attacks once and is done. A hero on a "move" turn steps one tile and is done. Opportunity attacks, zone of control, attack-of-opportunity — deferred. Adding them is a richness-vs-readability trade we'll make after the base system lands.

---

## 7. Ability Model

Every hero has a three-layer kit:

### 7.1 Basic attack (always available on "act" turns)
Class-flavored: Warrior sword strike, Mage wand bolt, Ranger bow shot. Consistent damage baseline. This is what fires on most turns — rhythm and predictability come from here.

### 7.2 Short-cooldown abilities (2–3 per class)
Turn-counted cooldowns (typical 3–6 turns). AI fires them when conditions match the ability's priority rule (e.g., Cleave fires when 2+ enemies are adjacent; Heal fires when ally at <50% HP in range). These are the "fancy moves" that color every encounter — most fights will see each cooldown ability fire 1–3 times.

### 7.3 Signature move (1 per class, one use per encounter)
Big, flashy, distinctive. Warrior's Rallying Cry, Cleric's Mass Heal, Mage's Meteor, Rogue's Shadowstep, Ranger's Volley. AI saves it for the "right moment" — a threshold-based priority rule per class (e.g., Mage's Meteor fires when ≥3 enemies are within blast radius, or when an encounter passes round 5 without it firing).

Refreshes between encounters, not between turns. Creates highlight-reel moments and a visible "she's saving it" tension that makes a signature's eventual use land hard.

### 7.4 Why this hybrid
- **Cooldowns** give the rhythmic beat and readability the player needs to parse combat at a glance.
- **Signature moves** create clutch moments without the tuning burden of per-encounter charges on every ability.
- **Traits** (§10) modify timing — Brave rushes signatures, Cautious saves them — giving emergent personality on top.
- **Gear** modifies cooldown length, signature-move effects, or basic-attack properties, giving the Quartermaster (from the scale/automation design) meaningful work.

---

## 8. Initiative

At the start of every round, each combatant rolls:
`initiative = d20 + DEX + modifiers`

Sort descending. That's the turn order for this round only. Next round, reroll.

### 8.1 Why reroll each round
- Small narrative arc per round — "who goes first?" is an ever-present micro-question.
- Lets the encounter swing: sometimes the mage opens with a nuke, sometimes the ogre clubs them first.
- Makes DEX relevant on every class, not only Rogue/Ranger — affects recruitment and gear value across the board.
- Cheap: one sort per round.

### 8.2 Trait/gear hooks
Natural modifier surface:
- *Swift* trait: +5 initiative.
- *Slow* trait: −5.
- *Lucky*: reroll once per encounter on a result below 10.
- Boots of the Gale: +3 initiative.

### 8.3 Variance knob
If the d20 variance feels too swingy in playtest, fallback is #2 from the brainstorm — roll once at encounter start and lock. Design assumes reroll-per-round unless playtest disproves it.

---

## 9. Class Differentiation — Sketches

These are illustrative only. The point is to show how the systems above produce distinctly different play for each base class.

### 9.1 Warrior
- **Basic:** Sword strike (melee, adjacent).
- **Short-CD:** Shield Bash (adjacent, stuns 1 turn, 4-turn CD). Cleave (hits all adjacent, 5-turn CD). Intercept (on ally's turn, if ally is hit in melee, Warrior can swap positions — 6-turn CD).
- **Signature:** Rallying Cry — all allies within 3 tiles get +20% damage for 3 rounds.
- **Play pattern:** Closes distance turn 1–2, anchors front line, cleaves clumps, intercepts squishies, opens with Rally in big fights.

### 9.2 Rogue
- **Basic:** Dagger strike (melee, adjacent, +dmg if flanking).
- **Short-CD:** Backstab (requires flanking, 3-turn CD). Shadowstep (teleport up to 4 tiles, 5-turn CD — note: this is a move *action*, so it happens on an act turn, not a move turn). Smoke Bomb (AoE stealth for allies, 6-turn CD).
- **Signature:** Assassinate — massive single-target damage if target is below 30% HP.
- **Play pattern:** Shadowsteps into flank positions, stacks backstabs, saves Assassinate to execute wounded enemies.

### 9.3 Mage
- **Basic:** Arcane bolt (5-tile range).
- **Short-CD:** Fireball (AoE at 6 range, 4-turn CD). Frost Nova (adjacent ring, roots enemies 1 turn, 5-turn CD). Arcane Shield (self or ally, 50% damage reduction 2 turns, 5-turn CD).
- **Signature:** Meteor — 3-tile AoE, massive damage, 2-turn cast (Mage is stationary).
- **Play pattern:** Stays at range, picks off stragglers with bolt, fireballs groups, saves Meteor for boss/dense packs.

### 9.4 Cleric
- **Basic:** Mace strike (melee) *or* Smite (3-range, holy damage) — picks by situation.
- **Short-CD:** Heal (range, 4-turn CD). Bless (target ally gets +damage, 5-turn CD). Turn Undead (undead-only, AoE, 6-turn CD).
- **Signature:** Mass Heal — heals whole party to full.
- **Play pattern:** Stays near the injured, heals reactively, saves Mass Heal for post-boss-phase recovery.

### 9.5 Ranger
- **Basic:** Bow shot (6-tile range).
- **Short-CD:** Hunter's Mark (marked enemy takes +damage, passive once applied, 4-turn CD). Piercing Shot (line AoE, 5-turn CD). Kite Step (move up to 2 tiles and attack, 5-turn CD — breaks the move-or-act rule situationally).
- **Signature:** Volley — hits all enemies in a 4-tile radius of a chosen point.
- **Play pattern:** Stays at max range, marks priority targets, kites with Kite Step when melee closes, Volleys groups.

### 9.6 Cross-class behaviors the system naturally produces
- Mages and Rangers get free full-round damage while melee classes close — which makes party composition (do I have enough ranged cover?) a real pre-mission decision.
- Rogues want a Warrior to hold a front line for them to flank — party synergy emerges.
- Cleric + Warrior tank duo is tight; Cleric + glass-cannon Mage is high-risk.

This is exactly the "classes actually do different things" depth the original conversation asked for.

---

## 10. Traits as AI Modifiers

Traits (from GDD §4.2) layer onto the combat AI's decision rules:

| Trait | Combat effect |
|-------|---------------|
| **Brave** | Uses signature moves earlier; closes to melee faster; prioritizes engaging strongest enemy. |
| **Cautious** | Saves signature longer; retreats when HP below 30% (walks away on move turns until healed); prioritizes weakest enemy. |
| **Greedy** | On turns where a loot chest is in range and no enemy threatens, uses act turn to open the chest. |
| **Berserker** | Locks on nearest enemy, never retreats, rushes signature. |
| **Leader** | Prioritizes ability targets that buff allies; uses Rally/Bless on initiative round 1. |
| **Lucky** | Signature move has a 10% chance to refresh on kill. |
| **Loner** | Attacks less effectively near allies (−10% dmg within 3 tiles of another hero). |

These are levers that make each hero feel like a specific person playing their class, not just "a Warrior." Two Brave Warriors and two Cautious Warriors should feel meaningfully different to watch.

---

## 11. Integration with Scale & Automation Design

From the 2026-04-21 design doc:

- **Per-encounter signature moves** are prime exception-queue events when used by favorite heroes ("Arya Meteor-one-shot the boss goblin"). This gives the Field Report dashboard emotional content without firehose.
- **Grid combat** produces rich, readable play that works at Tier 2 (Operations Wall) — each mini-render shows a legible turn-by-turn fight. Real-time blur was always going to look samey in miniatures.
- **Unified sim** means unobserved missions tick logic-only turns at high speed — combat resolution is fast when no one's watching, which is fine because the player isn't watching.
- **Gear/Quartermaster integration** — gear modifies ability cooldowns, signature effects, basic-attack properties. The Quartermaster's tier determines how smart gear swaps are (rookie auto-equips by item level; master considers ability-synergy and trait fit).
- **Missing/Rescue** from the scale doc — a defeated hero enters Missing status at encounter end. The encounter itself never has "permadeath on the tile" — it just determines who leaves the encounter standing.

---

## 12. Asset & Content Implications

The move-or-act grid is not free. It requires:

### 12.1 Per-class assets
- Idle sprite (4 directions)
- Walk animation (4 directions, tile-step)
- Basic attack animation (4 directions)
- One animation per short-CD ability (3 per class × 5 classes = 15 ability animations)
- One signature move animation per class (5 total)
- Hit/hurt animation (shared?)
- Death / downed animation (shared?)

### 12.2 Enemy work
Similar scope per enemy archetype. Initial bestiary should be small (5–8 archetypes) and expand as classes/content grow.

### 12.3 Effects library
Damage numbers, hit flashes, heal sparkles, AoE indicators, status-effect icons (stun, root, mark, buff, debuff).

### 12.4 UI needs
- Turn-order readout (shows initiative ladder)
- Cooldown indicators on each hero in Mission View HUD
- Signature-move availability pips
- Ability-range previews when the AI is about to act (optional, "ghost" highlight on the target tile a moment before action)

### 12.5 Placeholder path
Early development can use colored-rectangle heroes, arrow-direction facing, text-based damage numbers. The *systems* work regardless. Art pass fits the existing Phase 5 slot in the GDD roadmap.

---

## 13. Phased Implementation Sketch

Rough ordering:

1. **Turn queue + initiative** — add a round/turn scheduler to the mission sim. Heroes and enemies take turns. Movement becomes tile-stepped. Validate the unified sim runs exploration correctly (no combat yet).
2. **Move-or-act rule + basic attacks** — each turn is one choice. Basic-attack resolution, HP, damage numbers. Encounters engage when ranges overlap. Validate one-hero-vs-one-enemy encounters.
3. **Class differentiation via basic attacks** — melee range, ranged range, class-tagged damage. Validate party composition matters.
4. **Short-CD abilities** — 2–3 per class, AI priority rules. Validate encounters feel distinct per class.
5. **Signature moves** — per-encounter, priority-gated. Validate the clutch-moment payoff.
6. **Trait AI modifiers** — layer on top. Validate two heroes of the same class with different traits feel different.
7. **DEX initiative reroll + modifiers** — drop in the d20 system; wire DEX, Swift, Slow, Lucky. Validate rounds feel narratively varied.
8. **Polish & juice** — hit flashes, damage numbers, screen shake on signatures, audio beats, range previews.
9. **Tune & balance** — cooldown lengths, signature thresholds, initiative variance. This is ongoing.

---

## 14. Open Questions

- **Opportunity attacks / engagement rules** — skipped for first pass. Adding them increases depth but costs readability. Revisit after base system ships.
- **Line of sight for ranged** — initial version ignores walls for simplicity. Proper LoS adds realism but may frustrate when "obvious shots" miss. Decide after playtest.
- **Multi-tile enemies (bosses)** — does a boss occupy 2×2 tiles? Probably yes, deferred.
- **Kite Step and Shadowstep — do they break the move-or-act rule?** Current sketch says abilities can compound move and action. Need a clear rule on which abilities are "act-turn movement" vs. true compound actions.
- **Signature priority rules** — each class needs authored "save it for this condition" logic. Lots of tuning work.
- **Enemy "classes"** — do enemies also get signatures and short-CD abilities? Design assumes yes for elites/bosses, no for trash mobs (trash mobs have basic-attack only, maybe one ability).
- **How deadly should trash mobs be?** With one-act-per-turn melee, a pack of fast enemies can lock down a single Mage. Need tuning on enemy counts per encounter.
- **Display of enemy intent** — do we show "goblin is about to cleave" one turn ahead (Into the Breach style)? Increases readability, requires AI to commit to next action a turn early. Likely yes in a later pass; not in the first version.

---

## 15. Deferred / Adjacent Threads

- **Equipment system (Melvor-style slots)** — gear should hook directly into the ability layer (cooldown modifiers, damage modifiers, signature modifiers). Design this next once this combat model is validated.
- **Class specializations** (GDD §4.2) — branching abilities (Warrior → Knight vs. Berserker) are content expansion on top of this model; system-level work is minimal.
- **Enemy AI authoring tools** — a data-driven format for enemy behaviors (priority lists, conditions, abilities) will be needed to scale the bestiary without hand-coding each.
