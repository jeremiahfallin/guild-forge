# bevy_declarative

A fluent, declarative UI library for [Bevy](https://bevyengine.org/) using the builder pattern. Build game UI — HUDs, menus, inventory screens — with a chainable API inspired by [GPUI](https://gpui.rs/).

```rust
use bevy::prelude::*;
use bevy_declarative::prelude::*;
use bevy_declarative::style::values::px; // explicit import avoids ambiguity with bevy::prelude::px

fn setup_ui(mut commands: Commands) {
    div()
        .flex().col().gap_4().p_6()
        .bg(slate_900())
        .rounded(px(8.0))
        .child(
            text("My Game").text_3xl().color(white()),
        )
        .child(
            div().flex().row().gap_4()
                .child(text("Score: 100").text_lg().color(green_500()))
                .child(text("Lives: 3").text_lg().color(red_500())),
        )
        .child(
            div().p_3().bg(blue_600()).rounded(px(4.0))
                .on_click(|_: On<Pointer<Click>>| {
                    println!("Button clicked!");
                })
                .child(text("Play").text_lg().color(white())),
        )
        .spawn(&mut commands);
}
```

## Features

- **Fluent builder pattern** — chain `.flex().col().gap_2().p_4()` to build layouts
- **Styled trait** — shared styling methods across all element types
- **Tailwind-inspired presets** — `gap_2()`, `p_4()`, `text_lg()` with a 4px base scale
- **Raw values** — `gap(px(8.0))`, `w(pct(50.0))` when you need precise control
- **Built-in color palette** — Tailwind colors as functions: `red_500()`, `slate_900()`, `white()`
- **Event observers** — `on_click`, `on_hover`, `on_press` using Bevy's native observer system
- **Zero wrapper types** — produces standard Bevy `Node`, `Text`, `BackgroundColor` components

## Requirements

| Crate | Version |
|-------|---------|
| Bevy  | 0.18    |
| Rust  | 2024 edition |

## Installation

```toml
[dependencies]
bevy_declarative = { path = "path/to/bevy_declarative" }
```

## Usage

### Elements

**`div()`** — a layout container, maps to a Bevy `Node` entity:

```rust
div()
    .flex().row().gap_2()
    .bg(slate_800())
    .child(/* ... */)
    .spawn(&mut commands);
```

**`text("...")`** — a text leaf, maps to a Bevy `Text` component:

```rust
text("Hello World")
    .text_2xl()
    .color(white())
    .spawn(&mut commands);
```

### Layout

All elements implement the `Styled` trait for layout properties:

```rust
div()
    .flex().col()             // display + direction
    .gap_4()                  // spacing (Tailwind preset: 16px)
    .p_2()                    // padding (Tailwind preset: 8px)
    .items_center()           // align-items
    .justify_between()        // justify-content
    .w_full().h(px(200.0))   // sizing
    .absolute().top(px(0.0)) // positioning
```

**Raw values** for when presets aren't enough:

```rust
.gap(px(13.0))       // any pixel value
.w(pct(33.3))        // percentage
.p(Val::Vw(5.0))     // viewport units — use Bevy's Val directly
```

### Styling

```rust
div()
    .bg(red_500())                           // background color
    .rounded(px(8.0))                        // uniform border radius
    .border_radius(BorderRadius::all(px(8.0))) // custom border radius
```

### Text sizes

Tailwind-inspired font size presets on `TextEl`:

| Method | Size |
|--------|------|
| `text_xs()` | 12px |
| `text_sm()` | 14px |
| `text_base()` | 16px |
| `text_lg()` | 18px |
| `text_xl()` | 20px |
| `text_2xl()` | 24px |
| `text_3xl()` | 30px |
| `text_4xl()` | 36px |
| `text_5xl()` | 48px |
| `text_6xl()` | 60px |

Or set an exact size: `.font_size(22.0)`

### Colors

**Raw constructors:**

```rust
rgb(0xFF6B35)                  // hex -> Color
rgba(1.0, 0.5, 0.0, 0.8)      // RGBA floats -> Color
```

**Tailwind palette** (50–950 shades):

```rust
slate_500()   // gray tones
red_500()     // reds
green_500()   // greens
blue_500()    // blues
yellow_500()  // yellows
white()       // #FFFFFF
black()       // #000000
transparent() // fully transparent
```

### Events

Event handlers use Bevy's observer system — callbacks have full ECS access:

```rust
div()
    .on_click(|ev: On<Pointer<Click>>| {
        println!("clicked!");
    })
    .on_hover(|ev: On<Pointer<Over>>| {
        println!("hovered!");
    })
    .on_hover_out(|ev: On<Pointer<Out>>| { /* ... */ })
    .on_press(|ev: On<Pointer<Press>>| { /* ... */ })
    .on_release(|ev: On<Pointer<Release>>| { /* ... */ })
    .child(text("Interactive"))
    .spawn(&mut commands);
```

Callbacks are normal Bevy observer system params — you can query components, access resources, and issue commands:

```rust
div()
    .on_click(|_ev: On<Pointer<Click>>, mut query: Query<&mut BackgroundColor>| {
        for mut bg in &mut query {
            bg.0 = Color::srgb(1.0, 0.0, 0.0);
        }
    })
```

### Nesting

Build complex layouts by nesting `div()` and `text()`:

```rust
div().flex().col().gap_2().p_4().bg(slate_900())
    .child(
        text("Header").text_2xl().color(white()),
    )
    .child(
        div().flex().row().gap_4()
            .child(
                div().flex_1().p_3().bg(slate_800())
                    .child(text("Panel A").color(slate_200())),
            )
            .child(
                div().flex_1().p_3().bg(slate_800())
                    .child(text("Panel B").color(slate_200())),
            ),
    )
    .spawn(&mut commands);
```

### Note on `px` import

Both Bevy and bevy_declarative export a `px()` function. When using both preludes, add an explicit import to disambiguate:

```rust
use bevy::prelude::*;
use bevy_declarative::prelude::*;
use bevy_declarative::style::values::px; // resolves ambiguity
```

## License

MIT OR Apache-2.0
