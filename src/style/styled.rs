use bevy::ui::{
    AlignItems, AlignSelf, Display, FlexDirection, FlexWrap, JustifyContent, Node, OverflowAxis,
    PositionType, Val,
};

use super::values::{pct, px};

/// Core trait for styling UI elements with a fluent, Tailwind-inspired API.
///
/// Every element builder (Div, Text, etc.) implements this trait to gain
/// chainable layout and styling methods.
pub trait Styled: Sized {
    /// Returns a mutable reference to the underlying `Node` component.
    fn style_mut(&mut self) -> &mut Node;

    // ── Display ──────────────────────────────────────────────────────

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

    // ── Flex direction ───────────────────────────────────────────────

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

    // ── Flex wrap ────────────────────────────────────────────────────

    fn flex_wrap(mut self) -> Self {
        self.style_mut().flex_wrap = FlexWrap::Wrap;
        self
    }

    fn flex_nowrap(mut self) -> Self {
        self.style_mut().flex_wrap = FlexWrap::NoWrap;
        self
    }

    // ── Flex properties ──────────────────────────────────────────────

    fn flex_grow(mut self, value: f32) -> Self {
        self.style_mut().flex_grow = value;
        self
    }

    fn flex_shrink(mut self, value: f32) -> Self {
        self.style_mut().flex_shrink = value;
        self
    }

    fn flex_basis(mut self, value: Val) -> Self {
        self.style_mut().flex_basis = value;
        self
    }

    /// `flex: 1 1 0%` — grow and shrink equally from zero basis.
    fn flex_1(mut self) -> Self {
        let s = self.style_mut();
        s.flex_grow = 1.0;
        s.flex_shrink = 1.0;
        s.flex_basis = pct(0.0);
        self
    }

    /// `flex: 1 1 auto` — grow and shrink from auto basis.
    fn flex_auto(mut self) -> Self {
        let s = self.style_mut();
        s.flex_grow = 1.0;
        s.flex_shrink = 1.0;
        s.flex_basis = Val::Auto;
        self
    }

    /// `flex: none` — don't grow or shrink.
    fn flex_none(mut self) -> Self {
        let s = self.style_mut();
        s.flex_grow = 0.0;
        s.flex_shrink = 0.0;
        s.flex_basis = Val::Auto;
        self
    }

    // ── Align items ──────────────────────────────────────────────────

    fn items_start(mut self) -> Self {
        self.style_mut().align_items = AlignItems::FlexStart;
        self
    }

    fn items_end(mut self) -> Self {
        self.style_mut().align_items = AlignItems::FlexEnd;
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

    // ── Justify content ──────────────────────────────────────────────

    fn justify_start(mut self) -> Self {
        self.style_mut().justify_content = JustifyContent::FlexStart;
        self
    }

    fn justify_end(mut self) -> Self {
        self.style_mut().justify_content = JustifyContent::FlexEnd;
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

    // ── Self alignment ───────────────────────────────────────────────

    fn self_start(mut self) -> Self {
        self.style_mut().align_self = AlignSelf::FlexStart;
        self
    }

    fn self_end(mut self) -> Self {
        self.style_mut().align_self = AlignSelf::FlexEnd;
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

    // ── Position ─────────────────────────────────────────────────────

    fn absolute(mut self) -> Self {
        self.style_mut().position_type = PositionType::Absolute;
        self
    }

    fn relative(mut self) -> Self {
        self.style_mut().position_type = PositionType::Relative;
        self
    }

    // ── Inset ────────────────────────────────────────────────────────

    fn top(mut self, value: Val) -> Self {
        self.style_mut().top = value;
        self
    }

    fn bottom(mut self, value: Val) -> Self {
        self.style_mut().bottom = value;
        self
    }

    fn left(mut self, value: Val) -> Self {
        self.style_mut().left = value;
        self
    }

    fn right(mut self, value: Val) -> Self {
        self.style_mut().right = value;
        self
    }

    fn inset(mut self, value: Val) -> Self {
        let s = self.style_mut();
        s.top = value;
        s.bottom = value;
        s.left = value;
        s.right = value;
        self
    }

    // ── Sizing ───────────────────────────────────────────────────────

    fn w(mut self, value: Val) -> Self {
        self.style_mut().width = value;
        self
    }

    fn h(mut self, value: Val) -> Self {
        self.style_mut().height = value;
        self
    }

    fn size(mut self, value: Val) -> Self {
        let s = self.style_mut();
        s.width = value;
        s.height = value;
        self
    }

    fn min_w(mut self, value: Val) -> Self {
        self.style_mut().min_width = value;
        self
    }

    fn min_h(mut self, value: Val) -> Self {
        self.style_mut().min_height = value;
        self
    }

    fn max_w(mut self, value: Val) -> Self {
        self.style_mut().max_width = value;
        self
    }

    fn max_h(mut self, value: Val) -> Self {
        self.style_mut().max_height = value;
        self
    }

    fn w_full(mut self) -> Self {
        self.style_mut().width = pct(100.0);
        self
    }

    fn h_full(mut self) -> Self {
        self.style_mut().height = pct(100.0);
        self
    }

    fn w_half(mut self) -> Self {
        self.style_mut().width = pct(50.0);
        self
    }

    fn h_half(mut self) -> Self {
        self.style_mut().height = pct(50.0);
        self
    }

    fn w_auto(mut self) -> Self {
        self.style_mut().width = Val::Auto;
        self
    }

    fn h_auto(mut self) -> Self {
        self.style_mut().height = Val::Auto;
        self
    }

    // ── Gap ──────────────────────────────────────────────────────────

    fn gap(mut self, value: Val) -> Self {
        let s = self.style_mut();
        s.column_gap = value;
        s.row_gap = value;
        self
    }

    fn gap_x(mut self, value: Val) -> Self {
        self.style_mut().column_gap = value;
        self
    }

    fn gap_y(mut self, value: Val) -> Self {
        self.style_mut().row_gap = value;
        self
    }

    // ── Padding (raw) ────────────────────────────────────────────────

    fn p(mut self, value: Val) -> Self {
        let s = self.style_mut();
        s.padding.top = value;
        s.padding.bottom = value;
        s.padding.left = value;
        s.padding.right = value;
        self
    }

    fn px_val(mut self, value: Val) -> Self {
        let s = self.style_mut();
        s.padding.left = value;
        s.padding.right = value;
        self
    }

    fn py(mut self, value: Val) -> Self {
        let s = self.style_mut();
        s.padding.top = value;
        s.padding.bottom = value;
        self
    }

    fn pt(mut self, value: Val) -> Self {
        self.style_mut().padding.top = value;
        self
    }

    fn pb(mut self, value: Val) -> Self {
        self.style_mut().padding.bottom = value;
        self
    }

    fn pl(mut self, value: Val) -> Self {
        self.style_mut().padding.left = value;
        self
    }

    fn pr(mut self, value: Val) -> Self {
        self.style_mut().padding.right = value;
        self
    }

    // ── Margin (raw) ─────────────────────────────────────────────────

    fn m(mut self, value: Val) -> Self {
        let s = self.style_mut();
        s.margin.top = value;
        s.margin.bottom = value;
        s.margin.left = value;
        s.margin.right = value;
        self
    }

    fn mx(mut self, value: Val) -> Self {
        let s = self.style_mut();
        s.margin.left = value;
        s.margin.right = value;
        self
    }

    fn my(mut self, value: Val) -> Self {
        let s = self.style_mut();
        s.margin.top = value;
        s.margin.bottom = value;
        self
    }

    fn mt(mut self, value: Val) -> Self {
        self.style_mut().margin.top = value;
        self
    }

    fn mb(mut self, value: Val) -> Self {
        self.style_mut().margin.bottom = value;
        self
    }

    fn ml(mut self, value: Val) -> Self {
        self.style_mut().margin.left = value;
        self
    }

    fn mr(mut self, value: Val) -> Self {
        self.style_mut().margin.right = value;
        self
    }

    fn mx_auto(mut self) -> Self {
        let s = self.style_mut();
        s.margin.left = Val::Auto;
        s.margin.right = Val::Auto;
        self
    }

    // ── Overflow ─────────────────────────────────────────────────────

    fn overflow_hidden(mut self) -> Self {
        let s = self.style_mut();
        s.overflow.x = OverflowAxis::Clip;
        s.overflow.y = OverflowAxis::Clip;
        self
    }

    fn overflow_visible(mut self) -> Self {
        let s = self.style_mut();
        s.overflow.x = OverflowAxis::Visible;
        s.overflow.y = OverflowAxis::Visible;
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

    // ── Aspect ratio ─────────────────────────────────────────────────

    fn aspect_ratio(mut self, ratio: f32) -> Self {
        self.style_mut().aspect_ratio = Some(ratio);
        self
    }

    fn aspect_square(self) -> Self {
        self.aspect_ratio(1.0)
    }

    fn aspect_video(self) -> Self {
        self.aspect_ratio(16.0 / 9.0)
    }

    // ── Tailwind gap presets (4px base) ──────────────────────────────

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

    // ── Tailwind padding presets (4px base) ──────────────────────────

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

    // ── Tailwind margin presets (4px base) ───────────────────────────

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

    // ── Tailwind width presets (4px base) ────────────────────────────

    fn w_0(self) -> Self { self.w(px(0.0)) }
    fn w_1(self) -> Self { self.w(px(4.0)) }
    fn w_2(self) -> Self { self.w(px(8.0)) }
    fn w_4(self) -> Self { self.w(px(16.0)) }
    fn w_8(self) -> Self { self.w(px(32.0)) }
    fn w_16(self) -> Self { self.w(px(64.0)) }
    fn w_32(self) -> Self { self.w(px(128.0)) }
    fn w_64(self) -> Self { self.w(px(256.0)) }

    // ── Tailwind height presets (4px base) ───────────────────────────

    fn h_0(self) -> Self { self.h(px(0.0)) }
    fn h_1(self) -> Self { self.h(px(4.0)) }
    fn h_2(self) -> Self { self.h(px(8.0)) }
    fn h_4(self) -> Self { self.h(px(16.0)) }
    fn h_8(self) -> Self { self.h(px(32.0)) }
    fn h_16(self) -> Self { self.h(px(64.0)) }
    fn h_32(self) -> Self { self.h(px(128.0)) }
    fn h_64(self) -> Self { self.h(px(256.0)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestElement {
        node: Node,
    }

    impl TestElement {
        fn new() -> Self {
            Self {
                node: Node::default(),
            }
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
        assert_eq!(el.node.column_gap, px(8.0));
        assert_eq!(el.node.row_gap, px(8.0));
    }

    #[test]
    fn p_4_sets_all_padding() {
        let el = TestElement::new().p_4();
        assert_eq!(el.node.padding.top, px(16.0));
        assert_eq!(el.node.padding.bottom, px(16.0));
        assert_eq!(el.node.padding.left, px(16.0));
        assert_eq!(el.node.padding.right, px(16.0));
    }

    #[test]
    fn w_full_sets_100_percent() {
        let el = TestElement::new().w_full();
        assert_eq!(el.node.width, pct(100.0));
    }

    #[test]
    fn chaining_works() {
        let el = TestElement::new()
            .flex()
            .col()
            .items_center()
            .justify_between()
            .gap_4()
            .p_2()
            .w_full();
        assert_eq!(el.node.display, Display::Flex);
        assert_eq!(el.node.flex_direction, FlexDirection::Column);
        assert_eq!(el.node.align_items, AlignItems::Center);
        assert_eq!(el.node.justify_content, JustifyContent::SpaceBetween);
        assert_eq!(el.node.column_gap, px(16.0));
        assert_eq!(el.node.padding.top, px(8.0));
        assert_eq!(el.node.width, pct(100.0));
    }

    #[test]
    fn absolute_and_inset() {
        let el = TestElement::new().absolute().inset(px(0.0));
        assert_eq!(el.node.position_type, PositionType::Absolute);
        assert_eq!(el.node.top, px(0.0));
        assert_eq!(el.node.bottom, px(0.0));
        assert_eq!(el.node.left, px(0.0));
        assert_eq!(el.node.right, px(0.0));
    }
}
