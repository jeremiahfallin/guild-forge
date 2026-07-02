# BevyDeclarativePlugin Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a `BevyDeclarativePlugin` to bevy_declarative that provides scroll input handling and interaction palette color feedback, plus builder ergonomics.

**Architecture:** Two new modules (`scroll.rs`, `interaction.rs`) in bevy_declarative register systems/observers via a single `BevyDeclarativePlugin`. Guild-forge removes its duplicated scroll/interaction code and imports from the library instead.

**Tech Stack:** Rust, Bevy 0.18, bevy_declarative

---

### Task 1: Add `interaction.rs` to bevy_declarative

**Files:**
- Create: `C:\Users\bullf\dev\games\bevy_declarative\src\interaction.rs`

**Step 1: Create the interaction module**

```rust
//! Interaction palette — automatic background color changes on hover/press/release.

use bevy::color::Color;
use bevy::prelude::*;

/// Color palette for interactive UI elements. Attach to any entity with pointer
/// events to automatically update its [`BackgroundColor`] on hover, press, and release.
#[derive(Component, Debug, Reflect)]
#[reflect(Component)]
pub struct InteractionPalette {
    pub none: Color,
    pub hovered: Color,
    pub pressed: Color,
}

pub(crate) fn plugin(app: &mut App) {
    app.add_observer(on_click);
    app.add_observer(on_release);
    app.add_observer(on_over);
    app.add_observer(on_out);
}

fn on_click(
    click: On<Pointer<Click>>,
    mut q: Query<(&InteractionPalette, &mut BackgroundColor)>,
) {
    if let Ok((palette, mut bg)) = q.get_mut(click.event_target()) {
        *bg = palette.pressed.into();
    }
}

fn on_release(
    release: On<Pointer<Release>>,
    mut q: Query<(&InteractionPalette, &mut BackgroundColor)>,
) {
    if let Ok((palette, mut bg)) = q.get_mut(release.event_target()) {
        *bg = palette.hovered.into();
    }
}

fn on_over(
    over: On<Pointer<Over>>,
    mut q: Query<(&InteractionPalette, &mut BackgroundColor)>,
) {
    if let Ok((palette, mut bg)) = q.get_mut(over.event_target()) {
        *bg = palette.hovered.into();
    }
}

fn on_out(
    out: On<Pointer<Out>>,
    mut q: Query<(&InteractionPalette, &mut BackgroundColor)>,
) {
    if let Ok((palette, mut bg)) = q.get_mut(out.event_target()) {
        *bg = palette.none.into();
    }
}
```

**Step 2: Verify it compiles**

Run: `cargo check -p bevy_declarative`
Expected: PASS (module not yet wired into lib.rs, so no effect yet — this step is deferred to Task 3)

---

### Task 2: Add `scroll.rs` to bevy_declarative

**Files:**
- Create: `C:\Users\bullf\dev\games\bevy_declarative\src\scroll.rs`

**Step 1: Create the scroll module**

```rust
//! Scroll input handling — converts mouse wheel events into ScrollPosition updates
//! for any UI node with `OverflowAxis::Scroll`.

use bevy::{
    input::mouse::{MouseScrollUnit, MouseWheel},
    picking::hover::HoverMap,
    prelude::*,
};

const LINE_HEIGHT: f32 = 21.0;

pub(crate) fn plugin(app: &mut App) {
    app.add_systems(Update, send_scroll_events);
    app.add_observer(on_scroll);
}

#[derive(EntityEvent, Debug)]
#[entity_event(propagate, auto_propagate)]
struct Scroll {
    entity: Entity,
    delta: Vec2,
}

fn send_scroll_events(
    mut mouse_wheel: MessageReader<MouseWheel>,
    hover_map: Res<HoverMap>,
    mut commands: Commands,
) {
    for event in mouse_wheel.read() {
        let mut delta = -Vec2::new(event.x, event.y);
        if event.unit == MouseScrollUnit::Line {
            delta *= LINE_HEIGHT;
        }
        for pointer_map in hover_map.values() {
            for &entity in pointer_map.keys() {
                commands.trigger(Scroll { entity, delta });
            }
        }
    }
}

fn on_scroll(
    mut scroll: On<Scroll>,
    mut query: Query<(&mut ScrollPosition, &Node, &ComputedNode)>,
) {
    let Ok((mut scroll_position, node, computed)) = query.get_mut(scroll.entity) else {
        return;
    };

    let max_offset = (computed.content_size() - computed.size()) * computed.inverse_scale_factor();
    let delta = &mut scroll.delta;

    if node.overflow.x == OverflowAxis::Scroll && delta.x != 0.0 {
        let at_limit = if delta.x > 0.0 {
            scroll_position.x >= max_offset.x
        } else {
            scroll_position.x <= 0.0
        };
        if !at_limit {
            scroll_position.x += delta.x;
            delta.x = 0.0;
        }
    }

    if node.overflow.y == OverflowAxis::Scroll && delta.y != 0.0 {
        let at_limit = if delta.y > 0.0 {
            scroll_position.y >= max_offset.y
        } else {
            scroll_position.y <= 0.0
        };
        if !at_limit {
            scroll_position.y += delta.y;
            delta.y = 0.0;
        }
    }

    if *delta == Vec2::ZERO {
        scroll.propagate(false);
    }
}
```

---

### Task 3: Add `BevyDeclarativePlugin` and wire modules into `lib.rs`

**Files:**
- Modify: `C:\Users\bullf\dev\games\bevy_declarative\src\lib.rs`

**Step 1: Update lib.rs**

```rust
pub mod colors;
pub mod element;
pub mod events;
mod interaction;
mod scroll;
pub mod style;

use bevy::prelude::*;

pub use interaction::InteractionPalette;

/// Registers scroll input handling and interaction palette systems.
pub struct BevyDeclarativePlugin;

impl Plugin for BevyDeclarativePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((interaction::plugin, scroll::plugin));
    }
}

pub mod prelude {
    pub use crate::colors::*;
    pub use crate::element::*;
    pub use crate::events::*;
    pub use crate::style::*;
    pub use crate::{BevyDeclarativePlugin, InteractionPalette};
}
```

**Step 2: Verify bevy_declarative compiles**

Run: `cargo check -p bevy_declarative`
Expected: PASS

---

### Task 4: Add `interaction_palette()` builder method to `Div`

**Files:**
- Modify: `C:\Users\bullf\dev\games\bevy_declarative\src\element\div.rs`

**Step 1: Add the method**

Add to the `impl Div` block, after the `on_release` method (after line 112):

```rust
    /// Attach an [`InteractionPalette`] that automatically changes
    /// [`BackgroundColor`] on hover, press, and release.
    pub fn interaction_palette(
        mut self,
        none: bevy::color::Color,
        hovered: bevy::color::Color,
        pressed: bevy::color::Color,
    ) -> Self {
        self.observers
            .push(Box::new(move |ec: &mut EntityCommands| {
                ec.insert(crate::interaction::InteractionPalette {
                    none,
                    hovered,
                    pressed,
                });
            }));
        self
    }
```

**Step 2: Add a unit test**

Add to the existing `mod tests` block in `div.rs`:

```rust
    #[test]
    fn interaction_palette_adds_observer() {
        let d = div().interaction_palette(
            Color::srgb(0.0, 0.0, 0.0),
            Color::srgb(0.5, 0.5, 0.5),
            Color::srgb(1.0, 1.0, 1.0),
        );
        // interaction_palette pushes one closure into observers
        assert_eq!(d.observers.len(), 1);
    }
```

**Step 3: Verify tests pass**

Run: `cargo test -p bevy_declarative`
Expected: PASS

---

### Task 5: Add `overflow_x_scroll()` to `Styled` trait

**Files:**
- Modify: `C:\Users\bullf\dev\games\bevy_declarative\src\style\styled.rs`

**Step 1: Add the method**

Add after `overflow_y_scroll` (after line 454):

```rust
    fn overflow_x_scroll(mut self) -> Self {
        self.style_mut().overflow.x = OverflowAxis::Scroll;
        self
    }
```

**Step 2: Verify it compiles**

Run: `cargo test -p bevy_declarative`
Expected: PASS

**Step 3: Commit bevy_declarative changes**

```bash
cd C:\Users\bullf\dev\games\bevy_declarative
git add src/interaction.rs src/scroll.rs src/lib.rs src/element/div.rs src/style/styled.rs
git commit -m "feat: add BevyDeclarativePlugin with scroll and interaction palette systems"
```

---

### Task 6: Register `BevyDeclarativePlugin` in guild-forge

**Files:**
- Modify: `C:\Users\bullf\dev\games\guild-forge\src\main.rs:58`

**Step 1: Add the plugin**

In the `add_plugins` call at line 58, add `bevy_declarative::BevyDeclarativePlugin` to the tuple:

```rust
        app.add_plugins((
            bevy_declarative::BevyDeclarativePlugin,
            asset_tracking::plugin,
            // ... rest unchanged
        ));
```

**Step 2: Verify it compiles**

Run: `cargo check -p guild-forge`
Expected: PASS

---

### Task 7: Remove guild-forge's scroll module

**Files:**
- Delete: `C:\Users\bullf\dev\games\guild-forge\src\theme\scroll.rs`
- Modify: `C:\Users\bullf\dev\games\guild-forge\src\theme\mod.rs`

**Step 1: Remove scroll.rs and its registration**

Delete `src/theme/scroll.rs`.

In `src/theme/mod.rs`, change:

```rust
pub mod interaction;
pub mod palette;
pub mod scroll;
pub mod widgets;

// ...

pub(super) fn plugin(app: &mut App) {
    app.add_plugins((interaction::plugin, scroll::plugin));
}
```

To:

```rust
pub mod interaction;
pub mod palette;
pub mod widgets;

// ...

pub(super) fn plugin(app: &mut App) {
    app.add_plugins(interaction::plugin);
}
```

**Step 2: Verify it compiles**

Run: `cargo check -p guild-forge`
Expected: PASS

---

### Task 8: Slim down guild-forge's interaction module

**Files:**
- Modify: `C:\Users\bullf\dev\games\guild-forge\src\theme\interaction.rs`

**Step 1: Remove InteractionPalette and color observers, keep sound effects**

Replace the entire file with:

```rust
use bevy::prelude::*;
use bevy_declarative::InteractionPalette;

use crate::{asset_tracking::LoadResource, audio::sound_effect};

pub(super) fn plugin(app: &mut App) {
    app.load_resource::<InteractionAssets>();
    app.add_observer(play_sound_effect_on_click);
    app.add_observer(play_sound_effect_on_over);
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
struct InteractionAssets {
    #[dependency]
    hover: Handle<AudioSource>,
    #[dependency]
    click: Handle<AudioSource>,
}

impl FromWorld for InteractionAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            hover: assets.load("audio/sound_effects/button_hover.ogg"),
            click: assets.load("audio/sound_effects/button_click.ogg"),
        }
    }
}

fn play_sound_effect_on_click(
    on: On<Pointer<Click>>,
    interaction_assets: If<Res<InteractionAssets>>,
    interaction_entities: Query<Entity, With<InteractionPalette>>,
    mut commands: Commands,
) {
    if interaction_entities.contains(on.event_target()) {
        commands.spawn(sound_effect(interaction_assets.click.clone()));
    }
}

fn play_sound_effect_on_over(
    on: On<Pointer<Over>>,
    interaction_assets: If<Res<InteractionAssets>>,
    interaction_entities: Query<Entity, With<InteractionPalette>>,
    mut commands: Commands,
) {
    if interaction_entities.contains(on.event_target()) {
        commands.spawn(sound_effect(interaction_assets.hover.clone()));
    }
}
```

**Step 2: Verify it compiles**

Run: `cargo check -p guild-forge`
Expected: PASS

---

### Task 9: Migrate guild-forge screens to use `.interaction_palette()` builder and bevy_declarative import

**Files:**
- Modify: `C:\Users\bullf\dev\games\guild-forge\src\screens\guild.rs` (lines 213, 299)
- Modify: `C:\Users\bullf\dev\games\guild-forge\src\screens\armory.rs` (line 286)
- Modify: `C:\Users\bullf\dev\games\guild-forge\src\screens\recruiting_screen.rs` (line 192)
- Modify: `C:\Users\bullf\dev\games\guild-forge\src\screens\sidebar.rs` (lines 167, 180, 226)
- Modify: `C:\Users\bullf\dev\games\guild-forge\src\theme\widgets.rs` (lines 13, 90, 120)

**Step 1: Update each file**

For every occurrence of:
```rust
crate::theme::interaction::InteractionPalette {
    none: SOME_COLOR,
    hovered: SOME_COLOR,
    pressed: SOME_COLOR,
}
```
inside an `.insert((...))` call, remove it from the `.insert()` tuple and add `.interaction_palette(SOME_COLOR, SOME_COLOR, SOME_COLOR)` as a chained builder call instead.

For import statements like `use crate::theme::interaction::InteractionPalette;` or `use super::interaction::InteractionPalette;`, change to `use bevy_declarative::InteractionPalette;`.

Each file needs careful attention to preserve the other items in the `.insert()` tuple.

**Step 2: Verify it compiles and runs**

Run: `cargo check -p guild-forge`
Expected: PASS

**Step 3: Commit guild-forge changes**

```bash
cd C:\Users\bullf\dev\games\guild-forge
git add -A
git commit -m "refactor: use BevyDeclarativePlugin for scroll and interaction palette"
```
