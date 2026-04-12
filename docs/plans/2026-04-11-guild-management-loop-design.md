# Guild Management Loop — Design

## Core Loop

```
Missions generate → Gold + Materials + Reputation + XP
    ↓
Gold spent on → Recruiting heroes, Crafting gear, Upgrading buildings
Materials spent on → Crafting gear, Upgrading buildings
Reputation (earned, not spent) → Gates recruit quality + mission access
    ↓
Stronger guild → Harder missions → Better rewards → cycle continues
```

## Resources

### Gold
- **Source**: Mission completion rewards (randomized per template, already exists).
- **Spent on**: Recruiting heroes, crafting gear, upgrading buildings.
- **Existing**: Already implemented as `Gold` resource.

### Materials (Typed)
- **Types**: Iron, Leather, Wood, Herbs, Gems (and potentially more as content expands).
- **Source**: Mission loot. Each mission template defines which material types drop and in what quantities. Thematic mapping — forest dungeons drop wood/herbs, mines drop iron/gems, etc.
- **Spent on**: Crafting gear, upgrading buildings.
- **Storage**: New `Materials` resource holding a map of material type to quantity.

### Reputation
- **Source**: Earned on mission completion, scaling with mission difficulty.
- **Not spent** — acts as a passive progression gate.
- **Effects**: Higher reputation increases the quality floor of recruit applicants and unlocks harder mission templates on the mission board.

## Recruiting

### Applicant Board
- New UI tab or sub-screen showing a pool of up to 5 hero candidates.
- Pool cap scales with Recruitment Office building level.

### Arrival & Expiry
- New applicants arrive on a real-time timer (~1 per hour).
- Each applicant has an individual **availability window** (4–8 hours). A visible countdown shows time remaining.
- When an applicant's timer expires, they leave and the slot opens for a future arrival.
- Applicants persist across missions — no sudden board wipes.

### Quality Scaling
- Reputation determines:
  - Stat floor for generated candidates (higher rep = higher minimum stats).
  - Probability of rarer classes and better traits appearing.
- Low reputation: mostly average-stat common classes.
- High reputation: chance of strong candidates with powerful trait combinations.

### Cost
- Flat gold cost per recruit, scaling with candidate quality.
- Better stats/traits/classes = higher signing bonus.

### Roster Cap
- Maximum number of heroes in the guild, gated by Barracks building level.
- Cannot recruit if roster is full.

## Equipment — Class-Specific Upgrade Paths

### Slot System
Each hero has 3 gear slots: **Weapon**, **Armor**, **Accessory**.

### Linear Upgrade Paths
Each class defines a linear upgrade track per slot. Upgrading requires the previous tier to be equipped.

Example — Warrior:
- Weapon: Wooden Sword → Iron Sword → Steel Sword → Enchanted Blade
- Armor: Leather Tunic → Chainmail → Plate Armor → Runic Plate
- Accessory: Wooden Shield → Iron Shield → Tower Shield → Aegis

Example — Mage:
- Weapon: Wooden Staff → Crystal Staff → Arcane Staff → Staff of Power
- Armor: Cloth Robes → Enchanted Robes → Arcane Vestments → Robes of the Archmage
- Accessory: Pendant → Crystal Amulet → Arcane Focus → Orb of Insight

### Crafting
- **Source**: Crafting only. No equipment drops from missions.
- **Location**: Armory building (must be constructed first).
- **Cost**: Gold + specific typed materials per tier.
- **UI**: Select hero → select slot → view current tier and next upgrade → craft if resources available.
- **Stat effect**: Each equipment piece provides flat bonuses to hero combat stats (attack, defense, HP, etc.).

## Guild Buildings

Buildings are constructed and upgraded at the guild. Each has its own level track (Lv 0 = unbuilt → Lv 1 → Lv 2 → Lv 3, etc.). Building levels are **not gated by reputation** — only by gold + material costs.

### Building List

| Building | Unlocks | Upgrade Effect |
|----------|---------|----------------|
| **Armory** | Gear crafting | Higher levels unlock higher-tier recipes |
| **Training Grounds** | Passive XP for idle (non-deployed) heroes | More XP per tick, option to target specific stats |
| **Barracks** | Increases roster cap | +2 hero slots per level |
| **Recruitment Office** | More applicant slots on the board | +1 applicant slot per level, slight quality bump |
| **Workshop** | Material processing/conversion | Higher levels unlock higher-tier conversions |

### Construction
- Each building starts unbuilt (Lv 0).
- Constructing (Lv 0 → Lv 1) and upgrading cost gold + materials.
- Costs increase per level.

## Workshop — Material Processing

The Workshop allows converting raw materials into refined materials needed for higher-tier crafting.

### Conversion Examples
- 3 Iron Ore → 1 Steel Ingot
- 3 Raw Leather → 1 Cured Leather
- 3 Rough Gems → 1 Cut Gem

### Bulk Processing
- Player sets a quantity and processes the entire batch at once.
- UI shows: current stock of input/output materials, conversion recipe, max craftable quantity, and a quantity selector.
- No per-unit clicking.

### Tier Gating
- Workshop level determines which conversion recipes are available.
- Higher-tier conversions (e.g., Steel Ingot → Enchanted Steel) require higher Workshop levels.

## New UI

### New GameTab States
- **Guild Tab**: View/construct/upgrade buildings. Shows guild stats (reputation, roster capacity, building levels).
- **Armory Tab** (or sub-screen of Guild): Select hero → view gear slots → craft upgrades.
- **Applicant Board**: View available recruits, their stats/traits/class, time remaining, and hiring cost.

### Sidebar Updates
- Display current reputation alongside gold.
- Show material counts (or link to a detailed inventory view).

## Mission Template Changes

Mission templates need new fields:
- **`reputation_required`**: Minimum reputation to see this mission on the board.
- **`reputation_reward`**: Reputation earned on completion.
- **`material_drops`**: List of (material_type, quantity_range) pairs defining what materials the mission yields.

## Data Files (New/Modified)

- `assets/data/equipment.ron` — Class-specific upgrade paths with stat bonuses and crafting costs.
- `assets/data/buildings.ron` — Building definitions with per-level costs and effects.
- `assets/data/materials.ron` — Material type definitions and Workshop conversion recipes.
- `assets/data/mission_templates.ron` — Add reputation_required, reputation_reward, material_drops fields.
