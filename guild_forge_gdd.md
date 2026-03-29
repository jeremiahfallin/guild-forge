# Guild Forge — Game Design Document

> **Working Title:** Guild Forge
> **Genre:** Guild Management Sim / Idle-Adjacent RPG
> **Engine:** Bevy (Rust)
> **Platform:** PC (Steam), with potential for web export
> **Target Audience:** Fans of idle games, auto-battlers, and RPG progression systems
> **Inspirations:** Diablo botting culture, OSRS AFK grinding, Majesty, Guild Master, Loop Hero

---

## 1. Elevator Pitch

You are the master of a fledgling adventurer's guild in a medieval fantasy world. Recruit heroes, equip them, and dispatch them on missions — then watch them fight, loot, and level up in real-time top-down encounters. You don't control the heroes directly; you shape them through gear, training, party composition, and strategic mission selection. The joy is in watching your investments pay off as a ragtag band of Level 1 nobodies becomes an unstoppable force.

**The core fantasy:** You're the person behind the screen watching bots grind — except the bots have personalities, permadeath is on the table, and you're the one who built them.

---

## 2. Design Pillars

### 2.1 — "Watch Them Grow"
The primary emotional hook. Heroes start weak. The player makes meaningful choices about how they develop — class paths, stat allocation, gear — and the payoff comes from watching those choices play out autonomously in missions. Progression should feel tangible and visible (a hero that once struggled against goblins now cleaves through them).

### 2.2 — "Check In, Not Check Out"
The game respects the player's time and attention. Missions run in real-time and the player can observe them, but they are never *required* to micromanage. The management layer (guild upgrades, recruitment, mission selection) is the strategic core. Watching missions is a reward, not a chore.

### 2.3 — "Meaningful Risk"
Sending heroes on missions should carry weight. Failure has consequences — injury, lost gear, or even permanent death. This makes success satisfying and forces the player to weigh risk vs. reward when assigning missions.

### 2.4 — "Your Guild, Your Story"
Over time, the player builds a roster with history. Heroes have names, traits, and a track record. Veteran heroes who survived dozens of missions feel different from fresh recruits. The guild itself grows and changes visually and mechanically.

---

## 3. Core Gameplay Loop

```
┌─────────────────────────────────────────────────────┐
│                  GUILD MANAGEMENT                    │
│  Recruit → Equip → Train → Assign to Mission        │
└──────────────────────┬──────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────┐
│                  MISSION PHASE                       │
│  Heroes auto-explore a top-down map                  │
│  Combat is real-time and autonomous                  │
│  Player can observe any active mission               │
└──────────────────────┬──────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────┐
│                  RESULTS & GROWTH                    │
│  XP gained → Level ups → New abilities               │
│  Loot found → Equip or sell                          │
│  Injuries / deaths → Roster management               │
│  Guild reputation grows → Unlock harder missions     │
└──────────────────────┬──────────────────────────────┘
                       │
                       ▼
              (Back to Guild Management)
```

**Session structure:** A typical play session involves checking in on the guild, reviewing completed or ongoing missions, making a few strategic decisions (recruit someone new, upgrade a building, send a party on a harder mission), watching a mission unfold for a bit, then repeating. Designed for both long and short sessions.

---

## 4. Game Systems

### 4.1 — Guild Management Layer

This is the strategic hub. The player spends most of their decision-making time here.

#### The Guild Hall (Hub Screen)
- Visual representation of the guild that evolves as it is upgraded
- Key areas the player can interact with: **Roster Board**, **Mission Board**, **Armory**, **Training Grounds**, **Treasury**, **Tavern** (recruitment)
- Guild has a **Reputation** stat that gates access to higher-tier missions and recruits
- Guild has a **Gold** economy: missions earn gold, gold pays for recruits, gear, upgrades, and training

#### Recruitment (The Tavern)
- A rotating pool of available recruits refreshes periodically
- Each recruit has randomized: **Name**, **Portrait**, **Base Stats**, **Traits** (1-2), **Starting Class**
- Traits affect behavior in missions and stat growth (e.g., *Cautious* — avoids risky fights, takes less damage; *Greedy* — prioritizes loot chests over objectives; *Berserker* — charges into fights aggressively)
- Recruitment costs gold; rarer/higher-level recruits cost more
- Possible later system: heroes can arrive seeking your guild if reputation is high enough (passive recruitment)

#### Roster Management
- View all heroes: stats, level, equipment, trait list, mission history
- Assign heroes to **active**, **resting** (recovering from injury), or **training**
- Heroes have a fatigue or morale system — running them constantly without rest has diminishing returns
- Permadeath is a core mechanic, but can be tuned: configurable at game start (e.g., permadeath / injury-only / hardcore)

#### Equipment & Gear
- Gear is found on missions or purchased
- Slots: Weapon, Armor, Accessory (simple to start, expandable)
- Gear has rarity tiers: Common → Uncommon → Rare → Epic → Legendary
- Gear affects stats and can grant passive abilities (e.g., a ring that gives life steal, a sword that deals splash damage)
- Optional crafting system: combine materials from missions to forge gear

#### Guild Upgrades
- Spend gold and materials to upgrade guild facilities
- Examples:
  - **Training Grounds** → Faster XP gain for resting heroes, unlock advanced training
  - **Armory** → Unlock higher gear tiers, gear repair
  - **Infirmary** → Faster injury recovery, chance to survive a killing blow
  - **Library** → Unlock new class specializations
  - **Tavern** → Better recruit pool, higher recruit refresh rate
  - **War Room** → More simultaneous missions, mission intel (preview difficulty/rewards)

### 4.2 — Hero System

#### Stats
| Stat | Effect |
|------|--------|
| **STR** | Melee damage, carry capacity |
| **DEX** | Attack speed, dodge chance, ranged accuracy |
| **CON** | Max HP, injury resistance |
| **INT** | Magic damage, skill effectiveness |
| **WIS** | Mana/resource pool, trap detection |
| **CHA** | Party morale bonus, shop prices in missions |

#### Leveling
- Heroes gain XP from missions (combat, exploration, objectives)
- On level-up, the player allocates a small number of stat points and may choose from ability options
- This is a key player-agency moment: shaping heroes is the main strategic lever
- Level-up choices are influenced by class and traits (a *Berserker* warrior gets different ability options than a *Cautious* warrior)

#### Classes & Specializations
**Base Classes** (available at recruitment):
- **Warrior** — Melee frontline, high HP
- **Rogue** — Fast attacks, high dodge, can detect/disarm traps
- **Mage** — Ranged AoE magic, squishy
- **Cleric** — Heals and buffs, moderate combat
- **Ranger** — Ranged physical, good at scouting

**Specializations** (unlocked at a level threshold + Library upgrade):
Each base class branches into 2-3 specializations. Examples:
- Warrior → **Knight** (tankier, party defense aura) or **Berserker** (more damage, lower defense)
- Mage → **Elementalist** (raw damage) or **Enchanter** (buffs/debuffs)
- Rogue → **Assassin** (burst damage) or **Thief** (better loot find, can steal from enemies)

#### Traits
Traits are permanent personality quirks that affect mission AI behavior and stat growth. They create emergent storytelling and differentiation between heroes.

Example traits:
| Trait | Mission Behavior | Stat Effect |
|-------|-----------------|-------------|
| **Brave** | Engages enemies head-on | +CON growth |
| **Cautious** | Avoids fights when possible | +WIS growth |
| **Greedy** | Prioritizes loot over objectives | +CHA growth |
| **Loner** | Fights better solo, worse in parties | +STR growth |
| **Leader** | Buffs nearby allies | +CHA growth |
| **Cursed** | Attracts elite enemies | +XP gain |
| **Lucky** | Higher rare loot chance | Slightly random stat growth |

### 4.3 — Mission System

#### Mission Board
- Available missions are presented on a board in the guild hall
- Each mission has: **Name**, **Difficulty Rating**, **Estimated Duration**, **Reward Preview** (gold range, XP range, possible loot types), **Party Size** (solo / 2 / 3 / 4), **Special Conditions** (e.g., "undead heavy," "trapped dungeon," "boss encounter")
- The player assigns heroes to a mission and sends them off
- Multiple missions can run concurrently (gated by guild upgrade level)
- Missions have a real-time duration (minutes, not hours — this isn't a mobile idle game)

#### Mission Types
| Type | Description |
|------|-------------|
| **Dungeon Crawl** | Explore a procedurally generated dungeon, fight enemies, find loot, reach the boss |
| **Hunt** | Track and kill a specific monster in an open area |
| **Escort** | Protect an NPC along a path (NPC has its own AI) |
| **Gather** | Collect specific resources from a dangerous area |
| **Raid** | High-difficulty multi-party mission against a major threat |
| **Exploration** | Scout a new region; low combat, high discovery, unlocks future missions |

#### Mission Generation
- Missions are procedurally generated from templates + modifiers
- Higher guild reputation unlocks higher-tier mission templates
- Modifiers add variety: *Foggy* (reduced vision), *Infested* (more enemies), *Cursed Ground* (no healing), *Bountiful* (extra loot)

### 4.4 — Real-Time Mission View (The "Botwatch")

This is the signature feature. The player can click into any active mission and observe it in real time.

#### Visual Style
- Top-down 2D view (pixel art or low-poly — see Art Direction)
- Dungeon/environment tiles, hero sprites, enemy sprites, loot sparkles, spell effects
- Clear visual feedback: health bars, damage numbers, status icons, level-up fanfare

#### What the Player Sees
- Heroes navigating the map autonomously (pathfinding, room-by-room exploration)
- Real-time combat: heroes auto-attack and use abilities based on their class/trait AI
- Loot drops, chest openings, trap triggers
- Party health/mana bars, cooldown indicators
- A mission log/feed showing events ("Arden found a Rare Sword!" / "Mira leveled up to 5!" / "Bran triggered a trap!")

#### Player Interaction During Missions
The player does **not** directly control heroes. However, they can:
- **Watch** — Pure observation. The core experience.
- **Speed controls** — 1x, 2x, 4x speed (or pause to review the situation)
- **Ping/Retreat** (unlockable) — A limited-use signal that influences hero AI (e.g., ping a door to prioritize it, signal retreat to pull back). This is a guild upgrade, not a default ability.
- **Use consumables** (unlockable) — Drop a health potion or scroll into the mission from the guild's reserves. Limited uses, costs guild resources.

The key design constraint: **the player is a manager, not a commander.** Heroes have autonomy. The player's influence is indirect and limited, making pre-mission preparation the real lever.

#### Hero Mission AI
Heroes behave based on their class, traits, and stats:
- **Warriors** move to the front, engage the nearest enemy
- **Rogues** flank, avoid direct confrontation, target low-HP enemies
- **Mages** stay at range, prioritize AoE when enemies cluster
- **Clerics** stay near the most injured ally, heal proactively
- **Rangers** kite enemies, maintain distance

Traits modify this:
- A *Brave* rogue might be less cautious about flanking
- A *Greedy* warrior might break formation to grab a chest
- A *Loner* mage might wander away from the party

This creates emergent, watchable behavior that feels alive.

### 4.5 — Progression & Meta Loop

#### Short-term (per mission)
- Heroes gain XP and loot
- Gold earned for the guild treasury
- Mission log entries (memorable moments)

#### Medium-term (per play session)
- Level ups and stat allocation
- Gear upgrades
- Guild building upgrades
- New missions unlocked

#### Long-term (across many sessions)
- Guild reputation milestones that unlock new regions, mission types, and recruit tiers
- Veteran heroes with deep stat sheets and long mission histories
- Legendary gear sets to collect
- Story-gated "chapter" missions that advance a loose narrative
- Endgame: raid-tier missions requiring carefully optimized parties

#### Prestige / New Game+ (stretch goal)
- Option to "retire" the guild and start fresh with bonuses
- Retired heroes become legends, granting passive buffs to new guilds
- Unlocks new starting conditions, trait pools, or classes

---

## 5. Narrative & World

### 5.1 — Setting
A medieval fantasy world recovering from a cataclysm. Monsters have overrun the wilds, ancient dungeons have resurfaced, and civilization clings to walled cities and towns. Adventurer guilds have become essential — part mercenary company, part public service.

### 5.2 — Tone
- **Not grimdark**, but with real stakes. Heroes can die. The world is dangerous but not hopeless.
- Humor comes from hero traits and emergent behavior, not from the writing being silly.
- Think: the tone of a well-run D&D campaign — serious enough to care, fun enough to laugh.

### 5.3 — Narrative Structure
- No heavy-handed main story. The guild's growth *is* the story.
- Loose narrative delivered through: mission briefings, guild reputation milestones, rare "story missions" that reveal world lore, NPC visitors to the guild
- The player's attachment is to their heroes, not to a plot.

### 5.4 — Lore Hooks (expandable)
- The Sundering — the cataclysm that broke the old kingdom
- The Hollow — a mega-dungeon that serves as endgame content
- The Five Crowns — rival guilds that serve as both competitors and quest-givers
- The Old Gods — mysterious entities tied to legendary gear and secret missions

---

## 6. Art Direction

### 6.1 — Primary Recommendation: Pixel Art (16-bit era)
- Sprite-based characters and environments
- Resolution: 16x16 or 32x32 base tile size
- Animated hero sprites (idle, walk, attack, cast, hurt, death)
- Particle effects for spells, loot drops, level-ups
- Inspired by: Stardew Valley (polish), Moonlighter (dungeon feel), Kingdom (atmosphere)

### 6.2 — Alternative: Low-Poly 3D
- Simple, stylized 3D models with flat or lightly textured shading
- Top-down camera for mission view; isometric or free camera for guild hall
- Lower content creation overhead per-model but higher engine complexity
- Inspired by: Crossy Road (simplicity), For the King (style), Cult of the Lamb (charm)

### 6.3 — UI Style
- Clean, parchment/wood-themed UI panels for the guild management layer
- Minimal HUD during mission observation — health bars, floating damage numbers, subtle ability icons
- Mission log as a scrolling text feed, styled like a journal or ledger
- Responsive to resolution; design for 1920x1080 primary, scale down gracefully

---

## 7. Audio Direction

### 7.1 — Music
- Guild hall: Warm tavern/hearth music. Lute, flute, soft percussion. Cozy and inviting.
- Mission (exploration): Ambient, atmospheric. Builds tension subtly.
- Mission (combat): Up-tempo medieval fantasy. Drums, strings, brass.
- Results screen: Triumphant fanfare for success, somber tone for losses.

### 7.2 — SFX
- Satisfying hit/impact sounds for combat
- Distinct audio cues: loot drop chime, level-up fanfare, trap trigger, chest open
- UI sounds: parchment rustle for menus, coin clinks for transactions, stamp/seal for mission dispatch

---

## 8. Technical Architecture (Bevy-Specific)

### 8.1 — Why Bevy
- Rust's safety and performance are ideal for a game with multiple concurrent simulation threads
- Bevy's ECS (Entity Component System) maps naturally to this game's data model: heroes are entities, stats/traits/gear are components, mission AI and combat are systems
- Hot reloading and modular plugin architecture support iterative development
- Strong community and growing ecosystem

### 8.2 — High-Level Architecture

```
┌──────────────────────────────────────────────────────────┐
│                        Bevy App                          │
│                                                          │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────┐  │
│  │ Guild Plugin │  │ Mission      │  │ UI Plugin      │  │
│  │              │  │ Plugin       │  │ (egui or       │  │
│  │ - Roster     │  │              │  │  custom)       │  │
│  │ - Economy    │  │ - Simulation │  │                │  │
│  │ - Buildings  │  │ - Combat     │  │ - Guild View   │  │
│  │ - Recruit    │  │ - AI         │  │ - Mission View │  │
│  │   Pool       │  │ - Loot       │  │ - Menus        │  │
│  │              │  │ - Proc Gen   │  │ - HUD          │  │
│  └──────────────┘  └──────────────┘  └────────────────┘  │
│                                                          │
│  ┌──────────────┐  ┌──────────────┐  ┌────────────────┐  │
│  │ Hero Plugin  │  │ Save/Load    │  │ Audio Plugin   │  │
│  │              │  │ Plugin       │  │                │  │
│  │ - Stats      │  │              │  │ - Music        │  │
│  │ - Leveling   │  │ - Serde      │  │ - SFX          │  │
│  │ - Classes    │  │ - Autosave   │  │ - Ambience     │  │
│  │ - Traits     │  │ - Slots      │  │                │  │
│  │ - Equipment  │  │              │  │                │  │
│  └──────────────┘  └──────────────┘  └────────────────┘  │
│                                                          │
│  ┌──────────────────────────────────────────────────────┐ │
│  │                  Shared Resources                    │ │
│  │  GameTime, RNG Seed, GuildState, AssetHandles       │ │
│  └──────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────┘
```

### 8.3 — Key ECS Patterns

**Hero Entity:**
```
Entity: Hero
├── Name("Arden")
├── Stats { str: 12, dex: 8, con: 14, int: 6, wis: 10, cha: 9 }
├── Level(3)
├── Experience(450)
├── Class(Warrior)
├── Specialization(None)
├── Traits([Brave, Lucky])
├── Equipment { weapon: Some(entity), armor: Some(entity), accessory: None }
├── HeroState(OnMission(mission_entity))
├── Fatigue(0.3)
├── MissionHistory(vec![...])
└── Sprite / AnimationState (when rendered in mission view)
```

**Mission Entity:**
```
Entity: Mission
├── MissionType(DungeonCrawl)
├── Difficulty(3)
├── Modifiers([Foggy, Infested])
├── Duration { elapsed: 120.0, estimated: 300.0 }
├── Party(vec![hero_entity_1, hero_entity_2])
├── MissionMap(proc_gen_map_data)
├── MissionState(InProgress)
├── LootTable(vec![...])
└── EventLog(vec![...])
```

### 8.4 — Mission Simulation
- Missions run as self-contained simulations, ticked by a Bevy system
- When not being observed, missions can tick at an accelerated rate (logic-only, no rendering)
- When the player opens a mission view, the simulation syncs to real-time and rendering systems activate for that mission
- This allows many concurrent missions without GPU cost

### 8.5 — Procedural Generation
- Dungeon maps: BSP tree or wave function collapse for room/corridor layouts
- Enemy encounters: difficulty-scaled spawn tables
- Loot: weighted random from mission-specific loot tables
- All seeded from a deterministic RNG for reproducibility / save compatibility

### 8.6 — Save System
- Serialize full game state with `serde`
- Autosave on mission completion and periodic interval
- Multiple save slots
- Save includes: guild state, all hero data, active mission states, RNG state, game clock

### 8.7 — Suggested Bevy Crate Ecosystem
| Crate | Purpose |
|-------|---------|
| `bevy_egui` or `bevy_lunex` | UI framework for guild management screens |
| `bevy_ecs_tilemap` | Tilemap rendering for mission view |
| `bevy_asset_loader` | Streamlined asset loading states |
| `bevy_kira_audio` | Advanced audio (crossfades, layering) |
| `bevy_save` | Save/load framework |
| `bevy_turborand` | Deterministic RNG |
| `serde` / `ron` | Serialization (RON for human-readable configs) |
| `noise` or `bracket-lib` | Procedural generation utilities |

---

## 9. LLM-Assisted Development Strategy

Since this game is being built with LLM assistance (Claude), here's how to structure the workflow for maximum effectiveness.

### 9.1 — Phased Build Plan

**Phase 0 — Scaffolding (Week 1-2)**
- Set up Bevy project with plugin architecture
- Define core ECS components and resources
- Basic app states (MainMenu, GuildView, MissionView)
- Placeholder art (colored rectangles)

**Phase 1 — Hero Foundation (Week 3-4)**
- Hero entity with stats, traits, classes
- Roster management UI (list, inspect, equip)
- Basic leveling system
- Data-driven hero/class/trait definitions (RON or JSON)

**Phase 2 — Mission Simulation (Week 5-8)**
- Procedural dungeon generation (start with BSP)
- Tilemap rendering for mission view
- Hero AI (pathfinding, basic combat behavior)
- Enemy spawning and combat resolution
- Mission state machine (dispatch → in progress → complete/failed)
- Basic loot system

**Phase 3 — Guild Management (Week 9-11)**
- Mission board UI with procedural mission generation
- Recruitment system (tavern)
- Gold economy
- Guild building upgrades (data-driven)

**Phase 4 — The Glue (Week 12-14)**
- Connect mission results to hero progression
- Equipment system with gear affecting combat
- Mission log / event feed
- Multiple concurrent missions
- Speed controls for mission view

**Phase 5 — Polish & Content (Week 15-20)**
- Art pass (replace placeholders)
- Audio integration
- Balancing: XP curves, difficulty scaling, economy tuning
- Save/load system
- More mission types, traits, classes, gear
- UI polish and juice (animations, particles, screen shake)

**Phase 6 — Endgame & Release Prep (Week 21+)**
- Raid missions
- Story missions / world lore
- Steam integration (achievements, cloud saves)
- Playtesting and iteration

### 9.2 — LLM Workflow Tips
- **Work in vertical slices.** Get one hero doing one mission in one dungeon before expanding. Claude is most effective when the scope of a single conversation is focused.
- **Keep data-driven configs.** Define heroes, traits, missions, loot tables in RON/JSON files. This makes it easy to ask Claude to generate or balance content.
- **Use Bevy's plugin pattern aggressively.** Each plugin is a natural unit of work for an LLM session.
- **Write tests for simulation logic.** Combat resolution, XP calculations, loot rolls — these are pure functions that are easy to test and easy for an LLM to help write.
- **Version control after every working milestone.** If Claude introduces a regression, you can diff and isolate.
- **Maintain a living spec.** Keep this GDD updated as decisions change. Reference it at the start of new LLM sessions for context.

### 9.3 — Prompt Strategy
When working with Claude on implementation:
1. Start each session with the relevant section of this GDD + current code context
2. Ask for one plugin/system at a time
3. Request code with inline comments explaining *why*, not just *what*
4. Ask for Bevy-idiomatic patterns (systems, queries, events, states)
5. Have Claude generate data files (RON) for content like trait definitions, loot tables, etc.

---

## 10. Monetization & Release Strategy

### 10.1 — Business Model
- **Premium** (one-time purchase, no microtransactions)
- Target price: $14.99–$19.99
- DLC potential: new regions, class packs, guild themes

### 10.2 — Release Roadmap
1. **Prototype / Demo** — Playable vertical slice (1 guild, 3 heroes, dungeon crawl only)
2. **Early Access** — Core loop complete, limited content, community feedback
3. **1.0 Release** — Full content, polished, all systems in place
4. **Post-launch** — Community-requested features, DLC, mod support (stretch)

### 10.3 — Marketing Hooks
- "Watch your heroes grind so you don't have to"
- The botwatch / idle-RPG crossover is a unique niche
- GIF-able moments: hero clutch kills, rare loot drops, party wipes
- Streaming-friendly: the observation mechanic is inherently watchable

---

## 11. Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Hero AI feels dumb or repetitive | High — kills the core loop | Invest heavily in trait-driven behavior variation; playtest AI early |
| Mission observation gets boring | High — undermines the signature feature | Varied environments, random events, speed controls, emergent trait interactions |
| Scope creep | High — indie project | Strict phased plan; cut features, not polish |
| Bevy ecosystem immaturity | Medium — missing crates or breaking changes | Pin Bevy version per phase; contribute upstream if needed |
| Balancing economy / progression | Medium — too grindy or too fast | Data-driven tuning; spreadsheet model XP/gold curves before implementing |
| Art pipeline bottleneck | Medium — solo/small team | Start with asset packs; commission key art later; pixel art scales well |

---

## 12. Open Questions

- **How long should a typical mission take in real-time?** (Proposal: 3–10 minutes at 1x speed, depending on type)
- **How punishing should permadeath be?** (Offer difficulty modes? Iron Man mode as opt-in?)
- **Should the player ever be able to directly control a hero?** (Current design says no — but a "take the reins" emergency ability could be a late-game unlock)
- **Multiplayer?** (Competitive guild rankings? Cooperative raids? Or strictly single-player?)
- **Mod support?** (Bevy's data-driven architecture makes this feasible but it's a scope question)

---

*This is a living document. Update it as design decisions are made and playtesting reveals what works.*
