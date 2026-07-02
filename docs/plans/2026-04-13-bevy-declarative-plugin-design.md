# bevy_declarative: BevyDeclarativePlugin Design

## Problem

bevy_declarative exposes builder methods (`overflow_y_scroll()`, `on_click()`) that imply runtime behavior, but provides no systems to back them. Every consuming project must independently wire up scroll input handling and interaction color feedback.

## Solution

Add `BevyDeclarativePlugin` to bevy_declarative that registers the required systems and observers. Add builder ergonomics for interaction palettes and the missing `overflow_x_scroll()`.

## Changes to bevy_declarative

### New files

- `src/scroll.rs` — Scroll input system + handler observer
- `src/interaction.rs` — `InteractionPalette` component + 4 color observers (click/release/over/out)

### Modified files

- `src/lib.rs` — Add `BevyDeclarativePlugin`, re-export from prelude
- `src/element/div.rs` — Add `Div::interaction_palette(none, hovered, pressed)` method
- `src/style/styled.rs` — Add `overflow_x_scroll()` method

### Public API additions

- `BevyDeclarativePlugin` — registers scroll + interaction systems
- `InteractionPalette` — component with `none`, `hovered`, `pressed` colors
- `Div::interaction_palette()` — fluent builder method
- `Styled::overflow_x_scroll()` — missing counterpart

## Changes to guild-forge

- Delete `src/theme/scroll.rs`
- Strip `InteractionPalette` + 4 color observers from `src/theme/interaction.rs`, keep sound effect observers (import `InteractionPalette` from bevy_declarative)
- Replace `.insert(InteractionPalette { ... })` with `.interaction_palette(...)` in all screens
- Register `BevyDeclarativePlugin` in app setup
