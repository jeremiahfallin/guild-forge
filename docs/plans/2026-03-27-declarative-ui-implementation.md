# Declarative UI Builder Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a fluent, declarative Bevy UI crate using the builder pattern for game UI, inspired by GPUI.

**Architecture:** Builders accumulate style properties and children, then produce Bevy components via `.build()` or spawn entities via `.spawn(&mut commands)`. A `Styled` trait provides shared layout methods across all element types. Events use Bevy's observer system.

**Tech Stack:** Rust, Bevy 0.18 (`bevy_ui`, `bevy_picking`)

---

### Task 1: Project Scaffolding

**Files:**
- Create: `src/element/mod.rs`, `src/element/div.rs`, `src/element/text.rs`
- Create: `src/style/mod.rs`, `src/style/styled.rs`, `src/style/values.rs`
- Create: `src/colors/mod.rs`
- Create: `src/events/mod.rs`
- Modify: `src/lib.rs`

**Step 1: Create module directory structure with empty modules**

```rust
// src/style/values.rs
// Value helpers for UI dimensions

// src/style/styled.rs
// Styled trait definition

// src/style/mod.rs
pub mod styled;
pub mod values;

// src/colors/mod.rs
// Color constructors and palette

// src/events/mod.rs
// Event observer helpers

// src/element/div.rs
// Div builder

// src/element/text.rs
// Text builder

// src/element/mod.rs
pub mod div;
pub mod text;
```

**Step 2: Set up lib.rs with module declarations and prelude**

```rust
// src/lib.rs
pub mod colors;
pub mod element;
pub mod events;
pub mod style;

pub mod prelude {
    pub use crate::colors::*;
    pub use crate::element::div::*;
    pub use crate::element::text::*;
    pub use crate::events::*;
    pub use crate::style::styled::*;
    pub use crate::style::values::*;
}
```

**Step 3: Run `cargo check` to verify everything compiles**

Run: `cargo check`
Expected: Compiles with no errors (may have unused warnings, that's fine)

**Step 4: Commit**

```bash
git add src/
git commit -m "scaffold module structure for declarative UI"
```

---

### Task 2: Value Helpers

**Files:**
- Modify: `src/style/values.rs`

**Step 1: Write tests for value helpers**

```rust
// src/style/values.rs
use bevy::ui::Val;

/// Create a `Val::Px` value.
pub fn px(value: f32) -> Val {
    Val::Px(value)
}

/// Create a `Val::Percent` value.
pub fn pct(value: f32) -> Val {
    Val::Percent(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn px_creates_val_px() {
        assert_eq!(px(8.0), Val::Px(8.0));
    }

    #[test]
    fn pct_creates_val_percent() {
        assert_eq!(pct(50.0), Val::Percent(50.0));
    }
}
```

**Step 2: Run tests**

Run: `cargo test --lib style::values`
Expected: 2 tests pass

**Step 3: Commit**

```bash
git add src/style/values.rs
git commit -m "add px() and pct() value helpers"
```

---

### Task 3: Color System

**Files:**
- Modify: `src/colors/mod.rs`

**Step 1: Write rgb/rgba constructors and tests**

```rust
// src/colors/mod.rs
use bevy::color::Color;

/// Create a color from a hex RGB value (e.g., `0xFF0000` for red).
pub fn rgb(hex: u32) -> Color {
    let r = ((hex >> 16) & 0xFF) as f32 / 255.0;
    let g = ((hex >> 8) & 0xFF) as f32 / 255.0;
    let b = (hex & 0xFF) as f32 / 255.0;
    Color::srgb(r, g, b)
}

/// Create a color from RGBA float values (0.0 to 1.0).
pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Color {
    Color::srgba(r, g, b, a)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::color::Srgba;

    #[test]
    fn rgb_red() {
        let c = rgb(0xFF0000);
        let srgba: Srgba = c.into();
        assert!((srgba.red - 1.0).abs() < 0.01);
        assert!((srgba.green).abs() < 0.01);
        assert!((srgba.blue).abs() < 0.01);
    }

    #[test]
    fn rgba_semitransparent() {
        let c = rgba(1.0, 0.0, 0.0, 0.5);
        let srgba: Srgba = c.into();
        assert!((srgba.alpha - 0.5).abs() < 0.01);
    }
}
```

**Step 2: Run tests**

Run: `cargo test --lib colors`
Expected: 2 tests pass

**Step 3: Add Tailwind color palette**

Add below the constructors in `src/colors/mod.rs`. Include the most commonly used colors from Tailwind's palette. Each is a free function returning `Color`:

```rust
// Slate
pub fn slate_50() -> Color { rgb(0xF8FAFC) }
pub fn slate_100() -> Color { rgb(0xF1F5F9) }
pub fn slate_200() -> Color { rgb(0xE2E8F0) }
pub fn slate_300() -> Color { rgb(0xCBD5E1) }
pub fn slate_400() -> Color { rgb(0x94A3B8) }
pub fn slate_500() -> Color { rgb(0x64748B) }
pub fn slate_600() -> Color { rgb(0x475569) }
pub fn slate_700() -> Color { rgb(0x334155) }
pub fn slate_800() -> Color { rgb(0x1E293B) }
pub fn slate_900() -> Color { rgb(0x0F172A) }
pub fn slate_950() -> Color { rgb(0x020617) }

// Red
pub fn red_50() -> Color { rgb(0xFEF2F2) }
pub fn red_100() -> Color { rgb(0xFEE2E2) }
pub fn red_200() -> Color { rgb(0xFECACA) }
pub fn red_300() -> Color { rgb(0xFCA5A5) }
pub fn red_400() -> Color { rgb(0xF87171) }
pub fn red_500() -> Color { rgb(0xEF4444) }
pub fn red_600() -> Color { rgb(0xDC2626) }
pub fn red_700() -> Color { rgb(0xB91C1C) }
pub fn red_800() -> Color { rgb(0x991B1B) }
pub fn red_900() -> Color { rgb(0x7F1D1D) }
pub fn red_950() -> Color { rgb(0x450A0A) }

// Green
pub fn green_50() -> Color { rgb(0xF0FDF4) }
pub fn green_100() -> Color { rgb(0xDCFCE7) }
pub fn green_200() -> Color { rgb(0xBBF7D0) }
pub fn green_300() -> Color { rgb(0x86EFAC) }
pub fn green_400() -> Color { rgb(0x4ADE80) }
pub fn green_500() -> Color { rgb(0x22C55E) }
pub fn green_600() -> Color { rgb(0x16A34A) }
pub fn green_700() -> Color { rgb(0x15803D) }
pub fn green_800() -> Color { rgb(0x166534) }
pub fn green_900() -> Color { rgb(0x14532D) }
pub fn green_950() -> Color { rgb(0x052E16) }

// Blue
pub fn blue_50() -> Color { rgb(0xEFF6FF) }
pub fn blue_100() -> Color { rgb(0xDBEAFE) }
pub fn blue_200() -> Color { rgb(0xBFDBFE) }
pub fn blue_300() -> Color { rgb(0x93C5FD) }
pub fn blue_400() -> Color { rgb(0x60A5FA) }
pub fn blue_500() -> Color { rgb(0x3B82F6) }
pub fn blue_600() -> Color { rgb(0x2563EB) }
pub fn blue_700() -> Color { rgb(0x1D4ED8) }
pub fn blue_800() -> Color { rgb(0x1E40AF) }
pub fn blue_900() -> Color { rgb(0x1E3A8A) }
pub fn blue_950() -> Color { rgb(0x172554) }

// Yellow
pub fn yellow_50() -> Color { rgb(0xFEFCE8) }
pub fn yellow_100() -> Color { rgb(0xFEF9C3) }
pub fn yellow_200() -> Color { rgb(0xFEF08A) }
pub fn yellow_300() -> Color { rgb(0xFDE047) }
pub fn yellow_400() -> Color { rgb(0xFACC15) }
pub fn yellow_500() -> Color { rgb(0xEAB308) }
pub fn yellow_600() -> Color { rgb(0xCA8A04) }
pub fn yellow_700() -> Color { rgb(0xA16207) }
pub fn yellow_800() -> Color { rgb(0x854D0E) }
pub fn yellow_900() -> Color { rgb(0x713F12) }
pub fn yellow_950() -> Color { rgb(0x422006) }

// Common aliases
pub fn white() -> Color { rgb(0xFFFFFF) }
pub fn black() -> Color { rgb(0x000000) }
pub fn transparent() -> Color { rgba(0.0, 0.0, 0.0, 0.0) }
```

**Step 4: Run all tests**

Run: `cargo test --lib colors`
Expected: All tests pass

**Step 5: Commit**

```bash
git add src/colors/mod.rs
git commit -m "add color constructors and Tailwind palette"
```

---

### Task 4: Styled Trait — Core Layout Methods

**Files:**
- Modify: `src/style/styled.rs`

**Step 1: Define the Styled trait with core layout methods**

```rust
// src/style/styled.rs
use bevy::ui::{
    AlignContent, AlignItems, AlignSelf, Display, FlexDirection, FlexWrap,
    JustifyContent, JustifyItems, JustifySelf, Node, Overflow, OverflowAxis,
    PositionType, Val,
};

use super::values::{pct, px};

pub trait Styled: Sized {
    fn style_mut(&mut self) -> &mut Node;

    // Display
    fn flex(mut self) -> Self {
        self.style_mut().display = Display::Flex;
        self
    }
    fn grid(mut self) -> Self {
        self.style_mut().display = Display::Grid;
        self
    }
    fn block(mut self) -> Self {
        self.style_mut().display = Display::Block;
        self
    }
    fn hidden(mut self) -> Self {
        self.style_mut().display = Display::None;
        self
    }

    // Flex direction
    fn row(mut self) -> Self {
        self.style_mut().flex_direction = FlexDirection::Row;
        self
    }
    fn col(mut self) -> Self {
        self.style_mut().flex_direction = FlexDirection::Column;
        self
    }
    fn row_reverse(mut self) -> Self {
        self.style_mut().flex_direction = FlexDirection::RowReverse;
        self
    }
    fn col_reverse(mut self) -> Self {
        self.style_mut().flex_direction = FlexDirection::ColumnReverse;
        self
    }

    // Flex wrap
    fn flex_wrap(mut self) -> Self {
        self.style_mut().flex_wrap = FlexWrap::Wrap;
        self
    }
    fn flex_nowrap(mut self) -> Self {
        self.style_mut().flex_wrap = FlexWrap::NoWrap;
        self
    }

    // Flex properties
    fn flex_grow(mut self, val: f32) -> Self {
        self.style_mut().flex_grow = val;
        self
    }
    fn flex_shrink(mut self, val: f32) -> Self {
        self.style_mut().flex_shrink = val;
        self
    }
    fn flex_basis(mut self, val: Val) -> Self {
        self.style_mut().flex_basis = val;
        self
    }
    fn flex_1(mut self) -> Self {
        self.style_mut().flex_grow = 1.0;
        self.style_mut().flex_shrink = 1.0;
        self.style_mut().flex_basis = Val::Px(0.0);
        self
    }
    fn flex_auto(mut self) -> Self {
        self.style_mut().flex_grow = 1.0;
        self.style_mut().flex_shrink = 1.0;
        self.style_mut().flex_basis = Val::Auto;
        self
    }
    fn flex_none(mut self) -> Self {
        self.style_mut().flex_grow = 0.0;
        self.style_mut().flex_shrink = 0.0;
        self
    }

    // Alignment
    fn items_start(mut self) -> Self {
        self.style_mut().align_items = AlignItems::Start;
        self
    }
    fn items_end(mut self) -> Self {
        self.style_mut().align_items = AlignItems::End;
        self
    }
    fn items_center(mut self) -> Self {
        self.style_mut().align_items = AlignItems::Center;
        self
    }
    fn items_stretch(mut self) -> Self {
        self.style_mut().align_items = AlignItems::Stretch;
        self
    }
    fn items_baseline(mut self) -> Self {
        self.style_mut().align_items = AlignItems::Baseline;
        self
    }
    fn justify_start(mut self) -> Self {
        self.style_mut().justify_content = JustifyContent::Start;
        self
    }
    fn justify_end(mut self) -> Self {
        self.style_mut().justify_content = JustifyContent::End;
        self
    }
    fn justify_center(mut self) -> Self {
        self.style_mut().justify_content = JustifyContent::Center;
        self
    }
    fn justify_between(mut self) -> Self {
        self.style_mut().justify_content = JustifyContent::SpaceBetween;
        self
    }
    fn justify_around(mut self) -> Self {
        self.style_mut().justify_content = JustifyContent::SpaceAround;
        self
    }
    fn justify_evenly(mut self) -> Self {
        self.style_mut().justify_content = JustifyContent::SpaceEvenly;
        self
    }

    // Self alignment
    fn self_start(mut self) -> Self {
        self.style_mut().align_self = AlignSelf::Start;
        self
    }
    fn self_end(mut self) -> Self {
        self.style_mut().align_self = AlignSelf::End;
        self
    }
    fn self_center(mut self) -> Self {
        self.style_mut().align_self = AlignSelf::Center;
        self
    }
    fn self_stretch(mut self) -> Self {
        self.style_mut().align_self = AlignSelf::Stretch;
        self
    }

    // Position
    fn absolute(mut self) -> Self {
        self.style_mut().position_type = PositionType::Absolute;
        self
    }
    fn relative(mut self) -> Self {
        self.style_mut().position_type = PositionType::Relative;
        self
    }

    // Inset (position offsets)
    fn top(mut self, val: Val) -> Self {
        self.style_mut().top = val;
        self
    }
    fn bottom(mut self, val: Val) -> Self {
        self.style_mut().bottom = val;
        self
    }
    fn left(mut self, val: Val) -> Self {
        self.style_mut().left = val;
        self
    }
    fn right(mut self, val: Val) -> Self {
        self.style_mut().right = val;
        self
    }
    fn inset(mut self, val: Val) -> Self {
        let s = self.style_mut();
        s.top = val;
        s.bottom = val;
        s.left = val;
        s.right = val;
        self
    }

    // Sizing
    fn w(mut self, val: Val) -> Self {
        self.style_mut().width = val;
        self
    }
    fn h(mut self, val: Val) -> Self {
        self.style_mut().height = val;
        self
    }
    fn size(mut self, val: Val) -> Self {
        self.style_mut().width = val;
        self.style_mut().height = val;
        self
    }
    fn min_w(mut self, val: Val) -> Self {
        self.style_mut().min_width = val;
        self
    }
    fn min_h(mut self, val: Val) -> Self {
        self.style_mut().min_height = val;
        self
    }
    fn max_w(mut self, val: Val) -> Self {
        self.style_mut().max_width = val;
        self
    }
    fn max_h(mut self, val: Val) -> Self {
        self.style_mut().max_height = val;
        self
    }
    fn w_full(self) -> Self { self.w(pct(100.0)) }
    fn h_full(self) -> Self { self.h(pct(100.0)) }
    fn w_half(self) -> Self { self.w(pct(50.0)) }
    fn h_half(self) -> Self { self.h(pct(50.0)) }
    fn w_auto(self) -> Self { self.w(Val::Auto) }
    fn h_auto(self) -> Self { self.h(Val::Auto) }

    // Gap
    fn gap(mut self, val: Val) -> Self {
        self.style_mut().row_gap = val;
        self.style_mut().column_gap = val;
        self
    }
    fn gap_x(mut self, val: Val) -> Self {
        self.style_mut().column_gap = val;
        self
    }
    fn gap_y(mut self, val: Val) -> Self {
        self.style_mut().row_gap = val;
        self
    }

    // Padding (raw)
    fn p(mut self, val: Val) -> Self {
        let s = self.style_mut();
        s.padding.top = val;
        s.padding.bottom = val;
        s.padding.left = val;
        s.padding.right = val;
        self
    }
    fn px_val(mut self, val: Val) -> Self {
        self.style_mut().padding.left = val;
        self.style_mut().padding.right = val;
        self
    }
    fn py(mut self, val: Val) -> Self {
        self.style_mut().padding.top = val;
        self.style_mut().padding.bottom = val;
        self
    }
    fn pt(mut self, val: Val) -> Self {
        self.style_mut().padding.top = val;
        self
    }
    fn pb(mut self, val: Val) -> Self {
        self.style_mut().padding.bottom = val;
        self
    }
    fn pl(mut self, val: Val) -> Self {
        self.style_mut().padding.left = val;
        self
    }
    fn pr(mut self, val: Val) -> Self {
        self.style_mut().padding.right = val;
        self
    }

    // Margin (raw)
    fn m(mut self, val: Val) -> Self {
        let s = self.style_mut();
        s.margin.top = val;
        s.margin.bottom = val;
        s.margin.left = val;
        s.margin.right = val;
        self
    }
    fn mx(mut self, val: Val) -> Self {
        self.style_mut().margin.left = val;
        self.style_mut().margin.right = val;
        self
    }
    fn my(mut self, val: Val) -> Self {
        self.style_mut().margin.top = val;
        self.style_mut().margin.bottom = val;
        self
    }
    fn mt(mut self, val: Val) -> Self {
        self.style_mut().margin.top = val;
        self
    }
    fn mb(mut self, val: Val) -> Self {
        self.style_mut().margin.bottom = val;
        self
    }
    fn ml(mut self, val: Val) -> Self {
        self.style_mut().margin.left = val;
        self
    }
    fn mr(mut self, val: Val) -> Self {
        self.style_mut().margin.right = val;
        self
    }
    fn mx_auto(mut self) -> Self {
        self.style_mut().margin.left = Val::Auto;
        self.style_mut().margin.right = Val::Auto;
        self
    }

    // Overflow
    fn overflow_hidden(mut self) -> Self {
        self.style_mut().overflow.x = OverflowAxis::Clip;
        self.style_mut().overflow.y = OverflowAxis::Clip;
        self
    }
    fn overflow_visible(mut self) -> Self {
        self.style_mut().overflow.x = OverflowAxis::Visible;
        self.style_mut().overflow.y = OverflowAxis::Visible;
        self
    }
    fn overflow_x_hidden(mut self) -> Self {
        self.style_mut().overflow.x = OverflowAxis::Clip;
        self
    }
    fn overflow_y_hidden(mut self) -> Self {
        self.style_mut().overflow.y = OverflowAxis::Clip;
        self
    }

    // Aspect ratio
    fn aspect_ratio(mut self, ratio: f32) -> Self {
        self.style_mut().aspect_ratio = Some(ratio);
        self
    }
    fn aspect_square(mut self) -> Self {
        self.style_mut().aspect_ratio = Some(1.0);
        self
    }
    fn aspect_video(mut self) -> Self {
        self.style_mut().aspect_ratio = Some(16.0 / 9.0);
        self
    }

    // --- Tailwind preset scale (base = 4px) ---

    // Gap presets
    fn gap_0(self) -> Self { self.gap(px(0.0)) }
    fn gap_0p5(self) -> Self { self.gap(px(2.0)) }
    fn gap_1(self) -> Self { self.gap(px(4.0)) }
    fn gap_1p5(self) -> Self { self.gap(px(6.0)) }
    fn gap_2(self) -> Self { self.gap(px(8.0)) }
    fn gap_2p5(self) -> Self { self.gap(px(10.0)) }
    fn gap_3(self) -> Self { self.gap(px(12.0)) }
    fn gap_3p5(self) -> Self { self.gap(px(14.0)) }
    fn gap_4(self) -> Self { self.gap(px(16.0)) }
    fn gap_5(self) -> Self { self.gap(px(20.0)) }
    fn gap_6(self) -> Self { self.gap(px(24.0)) }
    fn gap_8(self) -> Self { self.gap(px(32.0)) }
    fn gap_10(self) -> Self { self.gap(px(40.0)) }
    fn gap_12(self) -> Self { self.gap(px(48.0)) }
    fn gap_16(self) -> Self { self.gap(px(64.0)) }

    // Padding presets
    fn p_0(self) -> Self { self.p(px(0.0)) }
    fn p_0p5(self) -> Self { self.p(px(2.0)) }
    fn p_1(self) -> Self { self.p(px(4.0)) }
    fn p_1p5(self) -> Self { self.p(px(6.0)) }
    fn p_2(self) -> Self { self.p(px(8.0)) }
    fn p_2p5(self) -> Self { self.p(px(10.0)) }
    fn p_3(self) -> Self { self.p(px(12.0)) }
    fn p_3p5(self) -> Self { self.p(px(14.0)) }
    fn p_4(self) -> Self { self.p(px(16.0)) }
    fn p_5(self) -> Self { self.p(px(20.0)) }
    fn p_6(self) -> Self { self.p(px(24.0)) }
    fn p_8(self) -> Self { self.p(px(32.0)) }
    fn p_10(self) -> Self { self.p(px(40.0)) }
    fn p_12(self) -> Self { self.p(px(48.0)) }
    fn p_16(self) -> Self { self.p(px(64.0)) }

    // Margin presets
    fn m_0(self) -> Self { self.m(px(0.0)) }
    fn m_0p5(self) -> Self { self.m(px(2.0)) }
    fn m_1(self) -> Self { self.m(px(4.0)) }
    fn m_1p5(self) -> Self { self.m(px(6.0)) }
    fn m_2(self) -> Self { self.m(px(8.0)) }
    fn m_2p5(self) -> Self { self.m(px(10.0)) }
    fn m_3(self) -> Self { self.m(px(12.0)) }
    fn m_3p5(self) -> Self { self.m(px(14.0)) }
    fn m_4(self) -> Self { self.m(px(16.0)) }
    fn m_5(self) -> Self { self.m(px(20.0)) }
    fn m_6(self) -> Self { self.m(px(24.0)) }
    fn m_8(self) -> Self { self.m(px(32.0)) }

    // Width presets
    fn w_0(self) -> Self { self.w(px(0.0)) }
    fn w_1(self) -> Self { self.w(px(4.0)) }
    fn w_2(self) -> Self { self.w(px(8.0)) }
    fn w_4(self) -> Self { self.w(px(16.0)) }
    fn w_8(self) -> Self { self.w(px(32.0)) }
    fn w_16(self) -> Self { self.w(px(64.0)) }
    fn w_32(self) -> Self { self.w(px(128.0)) }
    fn w_64(self) -> Self { self.w(px(256.0)) }

    // Height presets
    fn h_0(self) -> Self { self.h(px(0.0)) }
    fn h_1(self) -> Self { self.h(px(4.0)) }
    fn h_2(self) -> Self { self.h(px(8.0)) }
    fn h_4(self) -> Self { self.h(px(16.0)) }
    fn h_8(self) -> Self { self.h(px(32.0)) }
    fn h_16(self) -> Self { self.h(px(64.0)) }
    fn h_32(self) -> Self { self.h(px(128.0)) }
    fn h_64(self) -> Self { self.h(px(256.0)) }
}
```

**Step 2: Write tests**

Add to the bottom of `src/style/styled.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct TestElement {
        node: Node,
    }

    impl TestElement {
        fn new() -> Self {
            Self { node: Node::default() }
        }
    }

    impl Styled for TestElement {
        fn style_mut(&mut self) -> &mut Node {
            &mut self.node
        }
    }

    #[test]
    fn flex_sets_display() {
        let el = TestElement::new().flex();
        assert_eq!(el.node.display, Display::Flex);
    }

    #[test]
    fn col_sets_flex_direction() {
        let el = TestElement::new().col();
        assert_eq!(el.node.flex_direction, FlexDirection::Column);
    }

    #[test]
    fn gap_2_sets_both_gaps() {
        let el = TestElement::new().gap_2();
        assert_eq!(el.node.row_gap, Val::Px(8.0));
        assert_eq!(el.node.column_gap, Val::Px(8.0));
    }

    #[test]
    fn p_4_sets_all_padding() {
        let el = TestElement::new().p_4();
        assert_eq!(el.node.padding.top, Val::Px(16.0));
        assert_eq!(el.node.padding.bottom, Val::Px(16.0));
        assert_eq!(el.node.padding.left, Val::Px(16.0));
        assert_eq!(el.node.padding.right, Val::Px(16.0));
    }

    #[test]
    fn w_full_sets_100_percent() {
        let el = TestElement::new().w_full();
        assert_eq!(el.node.width, Val::Percent(100.0));
    }

    #[test]
    fn chaining_works() {
        let el = TestElement::new()
            .flex().col().gap_2().p_4()
            .items_center().justify_between();
        assert_eq!(el.node.display, Display::Flex);
        assert_eq!(el.node.flex_direction, FlexDirection::Column);
        assert_eq!(el.node.align_items, AlignItems::Center);
        assert_eq!(el.node.justify_content, JustifyContent::SpaceBetween);
    }

    #[test]
    fn absolute_and_inset() {
        let el = TestElement::new().absolute().top(px(10.0)).left(px(20.0));
        assert_eq!(el.node.position_type, PositionType::Absolute);
        assert_eq!(el.node.top, Val::Px(10.0));
        assert_eq!(el.node.left, Val::Px(20.0));
    }
}
```

**Step 3: Run tests**

Run: `cargo test --lib style::styled`
Expected: All tests pass

**Step 4: Commit**

```bash
git add src/style/styled.rs
git commit -m "add Styled trait with layout methods and Tailwind presets"
```

---

### Task 5: Div Builder

**Files:**
- Modify: `src/element/div.rs`

**Step 1: Implement the Div builder**

```rust
// src/element/div.rs
use bevy::color::Color;
use bevy::ecs::system::EntityCommands;
use bevy::hierarchy::ChildBuild;
use bevy::prelude::Commands;
use bevy::ui::{BackgroundColor, BorderRadius, Node, Val};

use crate::style::styled::Styled;

/// A child element that can be spawned into a parent.
pub trait Element {
    fn spawn_with_parent(self, parent: &mut impl ChildBuild);
}

/// A container layout element, analogous to an HTML `<div>`.
pub struct Div {
    node: Node,
    bg: Option<Color>,
    border_radius: Option<BorderRadius>,
    children: Vec<Box<dyn Element>>,
    #[cfg(feature = "picking")]
    observers: Vec<Box<dyn FnOnce(&mut EntityCommands)>>,
}

impl Div {
    pub fn new() -> Self {
        Self {
            node: Node::default(),
            bg: None,
            border_radius: None,
            children: Vec::new(),
        }
    }

    /// Set the background color.
    pub fn bg(mut self, color: Color) -> Self {
        self.bg = Some(color);
        self
    }

    /// Set a uniform border radius.
    pub fn rounded(mut self, val: Val) -> Self {
        self.border_radius = Some(BorderRadius::all(val));
        self
    }

    /// Set individual border radii.
    pub fn border_radius(mut self, radius: BorderRadius) -> Self {
        self.border_radius = Some(radius);
        self
    }

    /// Add a child element.
    pub fn child(mut self, child: impl Element + 'static) -> Self {
        self.children.push(Box::new(child));
        self
    }

    /// Spawn this element and its children into the world.
    pub fn spawn(self, commands: &mut Commands) -> EntityCommands {
        let mut node = self.node;
        if let Some(radius) = self.border_radius {
            node.border_radius = radius;
        }
        let mut ec = if let Some(bg) = self.bg {
            commands.spawn((node, BackgroundColor(bg)))
        } else {
            commands.spawn(node)
        };
        let children = self.children;
        if !children.is_empty() {
            ec.with_children(|parent| {
                for child in children {
                    child.spawn_with_parent(parent);
                }
            });
        }
        ec
    }
}

impl Default for Div {
    fn default() -> Self {
        Self::new()
    }
}

impl Styled for Div {
    fn style_mut(&mut self) -> &mut Node {
        &mut self.node
    }
}

impl Element for Div {
    fn spawn_with_parent(self, parent: &mut impl ChildBuild) {
        let mut node = self.node;
        if let Some(radius) = self.border_radius {
            node.border_radius = radius;
        }
        let mut ec = if let Some(bg) = self.bg {
            parent.spawn((node, BackgroundColor(bg)))
        } else {
            parent.spawn(node)
        };
        let children = self.children;
        if !children.is_empty() {
            ec.with_children(|parent| {
                for child in children {
                    child.spawn_with_parent(parent);
                }
            });
        }
    }
}

/// Create a new `Div` builder.
pub fn div() -> Div {
    Div::new()
}
```

**Step 2: Write tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ui::{Display, FlexDirection, Val};

    #[test]
    fn div_default_creates_default_node() {
        let d = div();
        assert_eq!(d.node.display, Display::Flex);
    }

    #[test]
    fn div_styled_chaining() {
        let d = div().flex().col().gap_2().p_4();
        assert_eq!(d.node.flex_direction, FlexDirection::Column);
        assert_eq!(d.node.row_gap, Val::Px(8.0));
        assert_eq!(d.node.padding.top, Val::Px(16.0));
    }

    #[test]
    fn div_bg_sets_color() {
        let d = div().bg(Color::srgb(1.0, 0.0, 0.0));
        assert!(d.bg.is_some());
    }

    #[test]
    fn div_child_adds_children() {
        let d = div()
            .child(div())
            .child(div());
        assert_eq!(d.children.len(), 2);
    }
}
```

**Step 3: Run tests**

Run: `cargo test --lib element::div`
Expected: All tests pass

**Step 4: Commit**

```bash
git add src/element/div.rs
git commit -m "add Div builder with Styled impl, children, and spawn"
```

---

### Task 6: Text Builder

**Files:**
- Modify: `src/element/text.rs`

**Step 1: Implement the Text builder**

```rust
// src/element/text.rs
use bevy::color::Color;
use bevy::ecs::system::EntityCommands;
use bevy::hierarchy::ChildBuild;
use bevy::prelude::Commands;
use bevy::text::{TextColor, TextFont};
use bevy::ui::Node;

use crate::element::div::Element;
use crate::style::styled::Styled;
use crate::style::values::px;

/// A text leaf element.
pub struct TextEl {
    node: Node,
    content: String,
    color: Option<Color>,
    font_size: Option<f32>,
    font_weight: Option<u16>,
}

impl TextEl {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            node: Node::default(),
            content: content.into(),
            color: None,
            font_size: None,
            font_weight: None,
        }
    }

    /// Set the text color.
    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    /// Set the font size in pixels.
    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = Some(size);
        self
    }

    // --- Tailwind text size presets ---

    pub fn text_xs(self) -> Self { self.font_size(12.0) }
    pub fn text_sm(self) -> Self { self.font_size(14.0) }
    pub fn text_base(self) -> Self { self.font_size(16.0) }
    pub fn text_lg(self) -> Self { self.font_size(18.0) }
    pub fn text_xl(self) -> Self { self.font_size(20.0) }
    pub fn text_2xl(self) -> Self { self.font_size(24.0) }
    pub fn text_3xl(self) -> Self { self.font_size(30.0) }
    pub fn text_4xl(self) -> Self { self.font_size(36.0) }
    pub fn text_5xl(self) -> Self { self.font_size(48.0) }
    pub fn text_6xl(self) -> Self { self.font_size(60.0) }

    /// Spawn this text element into the world.
    pub fn spawn(self, commands: &mut Commands) -> EntityCommands {
        let mut components = vec![];
        let text = bevy::text::Text::new(self.content);
        let mut ec = commands.spawn((self.node, text));
        if let Some(color) = self.color {
            ec.insert(TextColor(color));
        }
        if let Some(size) = self.font_size {
            ec.insert(TextFont {
                font_size: size,
                ..Default::default()
            });
        }
        ec
    }
}

impl Styled for TextEl {
    fn style_mut(&mut self) -> &mut Node {
        &mut self.node
    }
}

impl Element for TextEl {
    fn spawn_with_parent(self, parent: &mut impl ChildBuild) {
        let text = bevy::text::Text::new(self.content);
        let mut ec = parent.spawn((self.node, text));
        if let Some(color) = self.color {
            ec.insert(TextColor(color));
        }
        if let Some(size) = self.font_size {
            ec.insert(TextFont {
                font_size: size,
                ..Default::default()
            });
        }
    }
}

/// Create a new text element.
pub fn text(content: impl Into<String>) -> TextEl {
    TextEl::new(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_stores_content() {
        let t = text("Hello");
        assert_eq!(t.content, "Hello");
    }

    #[test]
    fn text_color_sets_color() {
        let t = text("Hi").color(Color::srgb(1.0, 0.0, 0.0));
        assert!(t.color.is_some());
    }

    #[test]
    fn text_lg_sets_font_size() {
        let t = text("Hi").text_lg();
        assert_eq!(t.font_size, Some(18.0));
    }

    #[test]
    fn text_styled_chaining() {
        let t = text("Hi").text_lg().p_2();
        assert_eq!(t.font_size, Some(18.0));
        assert_eq!(t.node.padding.top, Val::Px(8.0));
    }
}
```

**Step 2: Run tests**

Run: `cargo test --lib element::text`
Expected: All tests pass

**Step 3: Commit**

```bash
git add src/element/text.rs
git commit -m "add Text builder with font size presets and Styled impl"
```

---

### Task 7: Event System

**Files:**
- Modify: `src/events/mod.rs`
- Modify: `src/element/div.rs`
- Modify: `Cargo.toml`

**Step 1: Enable bevy_picking in Cargo.toml**

```toml
[dependencies]
bevy = { version = "0.18", default-features = false, features = ["bevy_ui", "bevy_picking"] }
```

**Step 2: Add observer storage and event methods to Div**

Add to `Div` struct (remove the `#[cfg(feature)]` gate since picking is now always on):

```rust
// In the Div struct, add:
observers: Vec<Box<dyn FnOnce(&mut EntityCommands)>>,

// In Div::new(), add to the initializer:
observers: Vec::new(),

// Add these methods to Div impl:
use bevy::ecs::observer::IntoObserverSystem;
use bevy::ecs::entity_disabling::Disabled;
use bevy::picking::events::{Click, Down, Out, Over, Pointer, Up};

pub fn on_click<M>(
    mut self,
    callback: impl IntoObserverSystem<Pointer<Click>, (), M> + Send + 'static,
) -> Self {
    self.observers.push(Box::new(move |ec: &mut EntityCommands| {
        ec.observe(callback);
    }));
    self
}

pub fn on_hover<M>(
    mut self,
    callback: impl IntoObserverSystem<Pointer<Over>, (), M> + Send + 'static,
) -> Self {
    self.observers.push(Box::new(move |ec: &mut EntityCommands| {
        ec.observe(callback);
    }));
    self
}

pub fn on_hover_out<M>(
    mut self,
    callback: impl IntoObserverSystem<Pointer<Out>, (), M> + Send + 'static,
) -> Self {
    self.observers.push(Box::new(move |ec: &mut EntityCommands| {
        ec.observe(callback);
    }));
    self
}

pub fn on_down<M>(
    mut self,
    callback: impl IntoObserverSystem<Pointer<Down>, (), M> + Send + 'static,
) -> Self {
    self.observers.push(Box::new(move |ec: &mut EntityCommands| {
        ec.observe(callback);
    }));
    self
}

pub fn on_up<M>(
    mut self,
    callback: impl IntoObserverSystem<Pointer<Up>, (), M> + Send + 'static,
) -> Self {
    self.observers.push(Box::new(move |ec: &mut EntityCommands| {
        ec.observe(callback);
    }));
    self
}
```

**Step 3: Wire observers into spawn methods**

In both `Div::spawn` and `Div::spawn_with_parent`, after spawning the entity and children, add:

```rust
for observer in self.observers {
    observer(&mut ec);
}
```

Note: `self.observers` must be consumed before the `with_children` closure captures `self.children`. Extract both fields before use:

```rust
let children = self.children;
let observers = self.observers;
// ... spawn, with_children using children ...
for observer in observers {
    observer(&mut ec);
}
```

**Step 4: Run cargo check**

Run: `cargo check`
Expected: Compiles. Exact observer trait bounds may need adjustment — fix any compilation errors.

**Step 5: Commit**

```bash
git add Cargo.toml src/events/mod.rs src/element/div.rs
git commit -m "add event observer methods (on_click, on_hover, etc.) to Div"
```

---

### Task 8: Events Module & Prelude Finalization

**Files:**
- Modify: `src/events/mod.rs`
- Modify: `src/lib.rs`
- Modify: `src/element/mod.rs`
- Modify: `src/style/mod.rs`

**Step 1: Re-export picking events in events module**

```rust
// src/events/mod.rs
// Re-export commonly used Bevy picking types for convenience
pub use bevy::picking::events::{Click, Down, Out, Over, Pointer, Up};
```

**Step 2: Ensure all module files have correct re-exports**

```rust
// src/element/mod.rs
pub mod div;
pub mod text;
pub use div::{div, Div, Element};
pub use text::{text, TextEl};

// src/style/mod.rs
pub mod styled;
pub mod values;
pub use styled::Styled;
pub use values::{pct, px};

// src/colors/mod.rs — already has everything at top level
```

**Step 3: Finalize lib.rs prelude**

```rust
// src/lib.rs
pub mod colors;
pub mod element;
pub mod events;
pub mod style;

pub mod prelude {
    pub use crate::colors::*;
    pub use crate::element::*;
    pub use crate::events::*;
    pub use crate::style::*;
}
```

**Step 4: Run all tests and cargo check**

Run: `cargo test && cargo check`
Expected: All tests pass, no compilation errors

**Step 5: Commit**

```bash
git add src/
git commit -m "finalize prelude and module re-exports"
```

---

### Task 9: Integration Smoke Test

**Files:**
- Create: `tests/smoke.rs`

**Step 1: Write a compile-time integration test that exercises the full API**

```rust
// tests/smoke.rs
use bevy::prelude::*;
use bevy_declarative::prelude::*;

/// This test verifies the full API compiles and chains correctly.
/// It does not run a Bevy app — it just exercises the builder types.
#[test]
fn full_api_smoke_test() {
    let _ui = div()
        .flex()
        .col()
        .gap_2()
        .p_4()
        .bg(slate_900())
        .rounded(px(8.0))
        .child(
            text("Hello World")
                .text_2xl()
                .color(white()),
        )
        .child(
            div()
                .flex()
                .row()
                .gap_4()
                .child(text("Score: 100").text_lg().color(green_500()))
                .child(text("Lives: 3").text_lg().color(red_500())),
        );

    // Verify we can still access builder fields after chaining
    let d = div().flex().col().p_4();
    assert_eq!(d.style_mut_ref().padding.top, Val::Px(16.0));
}
```

Note: We may need to add a `style_mut_ref(&self) -> &Node` or just use the existing test patterns. Adjust based on what the Div struct exposes. The key goal is that the full fluent API compiles and chains.

**Step 2: Run the integration test**

Run: `cargo test --test smoke`
Expected: Test passes

**Step 3: Commit**

```bash
git add tests/smoke.rs
git commit -m "add integration smoke test for full declarative UI API"
```
