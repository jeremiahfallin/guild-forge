use bevy::prelude::*;
// Import our prelude, but explicitly import `px` to avoid ambiguity with bevy::prelude::px
use bevy_declarative::prelude::*;
use bevy_declarative::style::values::px;

/// Verifies the full fluent API compiles and chains correctly.
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
}

/// Verifies Styled trait methods work on Div.
#[test]
fn styled_methods_on_div() {
    let d = div()
        .flex()
        .col()
        .items_center()
        .justify_between()
        .gap_4()
        .p_6()
        .w_full()
        .h_full()
        .absolute()
        .top(px(0.0))
        .left(px(0.0));

    // Just verifying it compiles and chains — builder is consumed by spawn()
    let _ = d;
}

/// Verifies Styled trait methods work on TextEl.
#[test]
fn styled_methods_on_text() {
    let t = text("Hello")
        .text_xl()
        .color(blue_500())
        .p_2()
        .m_1();

    let _ = t;
}

/// Verifies event methods compile on Div.
#[test]
fn event_methods_compile() {
    let _d = div()
        .p_4()
        .bg(blue_500())
        .on_click(|_ev: On<Pointer<Click>>| {})
        .on_hover(|_ev: On<Pointer<Over>>| {})
        .on_hover_out(|_ev: On<Pointer<Out>>| {})
        .on_press(|_ev: On<Pointer<Press>>| {})
        .on_release(|_ev: On<Pointer<Release>>| {})
        .child(text("Click me"));
}
