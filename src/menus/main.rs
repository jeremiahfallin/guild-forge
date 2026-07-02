//! The main menu (seen on the title screen).

use bevy::ecs::system::IntoObserverSystem;
use bevy::prelude::*;
use bevy_declarative::element::div::{Div, div};
use bevy_declarative::element::text::text;
use bevy_declarative::style::styled::Styled;
use bevy_declarative::style::values::px;

use crate::{asset_tracking::ResourceHandles, menus::Menu, screens::Screen, theme::widgets};

#[derive(Component)]
struct TitleText {
    phase: f32,
}

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(Menu::Main), spawn_main_menu);
    app.add_systems(
        Update,
        animate_title_text.run_if(in_state(Menu::Main)),
    );
}

fn spawn_main_menu(
    mut commands: Commands,
    glow_texture: Res<crate::screens::EmberGlowTexture>,
) {
    let mut menu = widgets::ui_root("Main Menu")
        .insert((GlobalZIndex(2), DespawnOnExit(Menu::Main)))
        .gap(px(24.0))
        // Title & Logo Block
        .child(
            div()
                .col()
                .items_center()
                .gap(px(15.0))
                // Hot glowing emblem
                .child(
                    div()
                        .w(px(80.0))
                        .h(px(80.0))
                        .insert(ImageNode {
                            image: glow_texture.0.clone(),
                            color: Color::srgba(0.95, 0.45, 0.05, 0.85),
                            ..default()
                        })
                )
                // Pulsing game title
                .child(
                    text("GUILD FORGE")
                        .font_size(68.0)
                        .insert(TitleText { phase: 0.0 })
                )
                // Tagline
                .child(
                    text("Forge your heroes. Command your guild.")
                        .font_size(22.0)
                        .color(Color::srgba(0.65, 0.65, 0.65, 0.7))
                )
        )
        // Divider line
        .child(
            div()
                .w(px(240.0))
                .h(px(2.0))
                .bg(Color::srgba(0.85, 0.25, 0.05, 0.35))
        )
        // Forge themed buttons
        .child(forge_button("Play", enter_loading_or_gameplay_screen))
        .child(forge_button("Settings", open_settings_menu))
        .child(forge_button("Credits", open_credits_menu));

    #[cfg(not(target_family = "wasm"))]
    {
        menu = menu.child(forge_button("Exit", exit_app));
    }

    // Absolute positioned footer
    menu = menu.child(
        div()
            .absolute()
            .bottom(px(20.0))
            .w_full()
            .row()
            .justify_between()
            .pad_x(px(40.0))
            .child(
                text("v0.1.0")
                    .font_size(16.0)
                    .color(Color::srgba(0.5, 0.5, 0.5, 0.5))
            )
            .child(
                text("© 2026 Jeremiah Fallin")
                    .font_size(16.0)
                    .color(Color::srgba(0.5, 0.5, 0.5, 0.5))
            )
    );

    menu.spawn(&mut commands);
}

fn forge_button<B: Bundle, M>(
    label: impl Into<String>,
    action: impl IntoObserverSystem<Pointer<Click>, B, M> + Sync + 'static,
) -> Div {
    let base_bg = Color::srgba(0.12, 0.12, 0.16, 0.85); // dark steel
    let hover_bg = Color::srgba(0.16, 0.16, 0.22, 0.95); // glowing steel
    let press_bg = Color::srgba(0.08, 0.08, 0.12, 0.95);
    
    div()
        .w(px(380.0))
        .h(px(72.0))
        .items_center()
        .justify_center()
        .bg(base_bg)
        .border_radius(BorderRadius::all(Val::Px(12.0)))
        .insert((
            Name::new("Forge Button"),
            Button,
        ))
        .interaction_palette(base_bg, hover_bg, press_bg)
        .on_click(action)
        .child(
            text(label)
                .font_size(32.0)
                .color(Color::srgba(0.95, 0.65, 0.15, 0.9)) // Warm gold text
                .insert(Pickable::IGNORE),
        )
}

fn animate_title_text(
    time: Res<Time>,
    mut query: Query<(&mut TextColor, &mut TitleText)>,
) {
    let dt = time.delta_secs();
    for (mut color, mut title) in &mut query {
        title.phase += dt * 2.0;
        let pulse = 0.85 + 0.15 * title.phase.sin();
        color.0 = Color::srgba(0.95 * pulse, 0.65 * pulse, 0.15 * pulse, 1.0);
    }
}

fn enter_loading_or_gameplay_screen(
    _: On<Pointer<Click>>,
    resource_handles: Res<ResourceHandles>,
    mut next_screen: ResMut<NextState<Screen>>,
) {
    if resource_handles.is_all_done() {
        next_screen.set(Screen::Gameplay);
    } else {
        next_screen.set(Screen::Loading);
    }
}

fn open_settings_menu(_: On<Pointer<Click>>, mut next_menu: ResMut<NextState<Menu>>) {
    next_menu.set(Menu::Settings);
}

fn open_credits_menu(_: On<Pointer<Click>>, mut next_menu: ResMut<NextState<Menu>>) {
    next_menu.set(Menu::Credits);
}

#[cfg(not(target_family = "wasm"))]
fn exit_app(_: On<Pointer<Click>>, mut app_exit: MessageWriter<AppExit>) {
    app_exit.write(AppExit::Success);
}
