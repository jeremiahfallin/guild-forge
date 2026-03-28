# Bevy Declarative UI — Design

## Goal

A fluent, declarative Bevy UI crate using the builder pattern for game UI (HUDs, menus, inventory screens). API inspired by GPUI.

## Approach: Builders as Bundle Constructors

Builders accumulate style and children, then produce Bevy bundles. `.spawn(&mut commands)` is the convenience path; `.build()` returns a raw bundle for manual control.

```rust
div().flex().row().gap_2()
    .child(text("Hello").text_lg())
    .spawn(&mut commands);
```

Children are spawned recursively and attached via Bevy's `ChildBuild` hierarchy.

## Core Elements

- **`div()`** — layout container, maps to Bevy `Node` entity
- **`text("...")`** — text leaf, maps to Bevy `Text` component

Both implement the `Styled` trait for shared styling methods.

## Styled Trait

```rust
pub trait Styled: Sized {
    fn style_mut(&mut self) -> &mut Node;
    // Layout: flex(), grid(), row(), col(), ...
    // Raw values: gap(Val), p(Val), w(Val), h(Val), ...
    // Tailwind presets: gap_1(), gap_2(), p_4(), text_lg(), ...
}
```

- Raw methods take Bevy's `Val` type
- Tailwind presets use 4px base unit (1=4px, 2=8px, 4=16px, ...)
- `px()` and `pct()` helper functions for `Val` construction
- Non-layout properties (bg, color) live on the builder structs directly

## Color System

- Raw: `rgb(0xFF0000)`, `rgba(r, g, b, a)` returning Bevy `Color`
- Palette: Tailwind color scale as free functions (`red_500()`, `slate_900()`, etc.)
- Flat module, glob-importable via prelude

## Event System

Uses Bevy's observer pattern. Builders store observer closures, registered on the entity during `.spawn()`.

```rust
div().on_click(|trigger: Trigger<Pointer<Click>>, ...| { ... })
```

Supported: `on_click`, `on_hover`, `on_hover_out`, `on_down`, `on_up`. Callbacks are normal Bevy observer system params with full ECS access.

## Module Structure

```
src/
  lib.rs          — prelude, re-exports
  element/
    mod.rs
    div.rs        — Div builder + Styled impl
    text.rs       — Text builder + Styled impl
  style/
    mod.rs
    styled.rs     — Styled trait definition
    values.rs     — px(), pct(), Tailwind presets
  colors/
    mod.rs        — rgb(), rgba(), palette functions
  events/
    mod.rs        — observer registration helpers
```

## Public API (prelude)

`div()`, `text()`, `px()`, `pct()`, `rgb()`, `rgba()`, color palette functions, `Styled` trait.
