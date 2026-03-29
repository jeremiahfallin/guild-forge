//! The hero roster screen — view and manage your guild's heroes.

use bevy::prelude::*;
use bevy_declarative::element::div::{Div, div};
use bevy_declarative::element::text::text;
use bevy_declarative::style::styled::Styled;
use bevy_declarative::style::values::{pct, px};

use crate::{
    hero::{Hero, HeroInfo, HeroStats, HeroTraits, data::*},
    screens::GameTab,
    theme::{palette::*, widgets},
};

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<SelectedHero>();
    app.add_systems(OnEnter(GameTab::Roster), spawn_roster);
    app.add_systems(
        Update,
        refresh_roster_on_selection_change.run_if(
            in_state(GameTab::Roster).and(resource_changed::<SelectedHero>),
        ),
    );
    app.add_systems(OnExit(GameTab::Roster), clear_selection);
}

/// Tracks which hero is currently selected in the roster.
#[derive(Resource, Default, Debug)]
pub struct SelectedHero(pub Option<Entity>);

/// Marker for the roster UI root so we can despawn and rebuild it.
#[derive(Component)]
struct RosterUi;

/// Marker for the detail panel so we can rebuild it on selection change.
#[derive(Component)]
struct DetailPanel;

fn spawn_roster(
    mut commands: Commands,
    heroes: Query<(Entity, &HeroInfo), With<Hero>>,
    selected: Res<SelectedHero>,
    trait_db: Res<TraitDatabase>,
    hero_query: Query<(&HeroInfo, &HeroStats, &HeroTraits), With<Hero>>,
) {
    let mut root = widgets::ui_root("Roster Screen")
        .insert((DespawnOnExit(GameTab::Roster), RosterUi));

    // Top bar: title + back button
    let top_bar = div()
        .row()
        .w_full()
        .justify_between()
        .items_center()
        .p(px(16.0))
        .child(widgets::header("Roster"))
        .child(widgets::game_button("Back", go_back));

    // Main content: two-panel layout
    let hero_list = build_hero_list(&heroes, &selected);
    let detail = build_detail_panel(&selected, &hero_query, &trait_db);

    let content = div()
        .row()
        .w_full()
        .flex_1()
        .gap(px(16.0))
        .p(px(16.0))
        .child(hero_list)
        .child(detail);

    root = root.child(top_bar).child(content);
    root.spawn(&mut commands);
}

fn build_hero_list(
    heroes: &Query<(Entity, &HeroInfo), With<Hero>>,
    selected: &SelectedHero,
) -> Div {
    let mut list = div()
        .col()
        .w(pct(30.0))
        .gap(px(8.0))
        .overflow_y_hidden()
        .insert(Name::new("Hero List"));

    list = list.child(
        text("Heroes")
            .font_size(28.0)
            .color(HEADER_TEXT),
    );

    for (entity, info) in heroes.iter() {
        let is_selected = selected.0 == Some(entity);
        let bg_color = if is_selected {
            Color::srgba(0.275, 0.400, 0.750, 0.8)
        } else {
            Color::srgba(0.2, 0.2, 0.3, 0.6)
        };

        let class_text = format!("Lv.{} {}", info.level, info.class);

        list = list.child(
            div()
                .row()
                .w_full()
                .p(px(12.0))
                .gap(px(12.0))
                .items_center()
                .bg(bg_color)
                .rounded(px(6.0))
                .insert(SelectHeroButton(entity))
                .on_click(select_hero)
                .child(
                    div()
                        .col()
                        .flex_1()
                        .child(
                            text(&info.name)
                                .font_size(22.0)
                                .color(HEADER_TEXT),
                        )
                        .child(
                            text(class_text)
                                .font_size(16.0)
                                .color(LABEL_TEXT),
                        ),
                ),
        );
    }

    list
}

fn build_detail_panel(
    selected: &SelectedHero,
    hero_query: &Query<(&HeroInfo, &HeroStats, &HeroTraits), With<Hero>>,
    trait_db: &TraitDatabase,
) -> Div {
    let panel = div()
        .col()
        .flex_1()
        .p(px(20.0))
        .gap(px(16.0))
        .bg(Color::srgba(0.15, 0.15, 0.25, 0.6))
        .rounded(px(8.0))
        .insert((Name::new("Detail Panel"), DetailPanel));

    let Some(entity) = selected.0 else {
        return panel.child(
            text("Select a hero to view details")
                .font_size(24.0)
                .color(Color::srgba(0.6, 0.6, 0.6, 0.8)),
        );
    };

    let Ok((info, stats, traits)) = hero_query.get(entity) else {
        return panel.child(
            text("Hero not found")
                .font_size(24.0)
                .color(Color::srgba(0.8, 0.3, 0.3, 1.0)),
        );
    };

    // Hero header
    let header = div()
        .col()
        .gap(px(4.0))
        .child(
            text(&info.name).font_size(36.0).color(HEADER_TEXT),
        )
        .child(
            text(format!("Level {} {}", info.level, info.class))
                .font_size(20.0)
                .color(LABEL_TEXT),
        )
        .child(
            text(format!("XP: {} / {}", info.xp, info.xp_to_next))
                .font_size(16.0)
                .color(LABEL_TEXT),
        );

    // Stats section
    let stats_section = build_stats_section(stats);

    // Traits section
    let traits_section = build_traits_section(&traits.0, trait_db);

    panel
        .child(header)
        .child(stats_section)
        .child(traits_section)
}

fn build_stats_section(stats: &HeroStats) -> Div {
    let stat_data = [
        ("STR", stats.strength),
        ("DEX", stats.dexterity),
        ("CON", stats.constitution),
        ("INT", stats.intelligence),
        ("WIS", stats.wisdom),
        ("CHA", stats.charisma),
    ];

    let mut section = div()
        .col()
        .gap(px(6.0))
        .child(
            text("Stats").font_size(24.0).color(HEADER_TEXT),
        );

    for (name, value) in stat_data {
        let bar_width = (value as f32 / 20.0 * 100.0).clamp(0.0, 100.0);

        section = section.child(
            div()
                .row()
                .items_center()
                .gap(px(8.0))
                .child(
                    text(name).font_size(16.0).color(LABEL_TEXT).w(px(40.0)),
                )
                .child(
                    text(format!("{value:2}"))
                        .font_size(16.0)
                        .color(HEADER_TEXT)
                        .w(px(28.0)),
                )
                .child(
                    div()
                        .flex_1()
                        .h(px(12.0))
                        .bg(Color::srgba(0.1, 0.1, 0.15, 0.8))
                        .rounded(px(3.0))
                        .child(
                            div()
                                .w(pct(bar_width))
                                .h_full()
                                .bg(stat_bar_color(value))
                                .rounded(px(3.0)),
                        ),
                ),
        );
    }

    section
}

fn build_traits_section(hero_traits: &[HeroTrait], trait_db: &TraitDatabase) -> Div {
    let mut section = div()
        .col()
        .gap(px(6.0))
        .child(
            text("Traits").font_size(24.0).color(HEADER_TEXT),
        );

    if hero_traits.is_empty() {
        return section.child(
            text("None")
                .font_size(16.0)
                .color(Color::srgba(0.5, 0.5, 0.5, 0.8)),
        );
    }

    for hero_trait in hero_traits {
        let (name, description) = trait_db
            .get(*hero_trait)
            .map(|def| (def.name.as_str(), def.description.as_str()))
            .unwrap_or(("Unknown", ""));

        section = section.child(
            div()
                .row()
                .gap(px(8.0))
                .p(px(8.0))
                .bg(Color::srgba(0.2, 0.2, 0.35, 0.5))
                .rounded(px(4.0))
                .child(
                    div()
                        .col()
                        .child(
                            text(name)
                                .font_size(18.0)
                                .color(BUTTON_TEXT),
                        )
                        .child(
                            text(description)
                                .font_size(14.0)
                                .color(LABEL_TEXT),
                        ),
                ),
        );
    }

    section
}

/// Returns a color for the stat bar based on the stat value.
fn stat_bar_color(value: i32) -> Color {
    if value >= 14 {
        Color::srgb(0.2, 0.7, 0.3) // Green — strong
    } else if value >= 10 {
        Color::srgb(0.3, 0.5, 0.8) // Blue — average
    } else {
        Color::srgb(0.7, 0.3, 0.2) // Red — weak
    }
}

/// Component on hero list entries, storing which entity they represent.
#[derive(Component)]
struct SelectHeroButton(Entity);

fn select_hero(
    click: On<Pointer<Click>>,
    buttons: Query<&SelectHeroButton>,
    mut selected: ResMut<SelectedHero>,
) {
    if let Ok(button) = buttons.get(click.event_target()) {
        selected.0 = Some(button.0);
    }
}

fn refresh_roster_on_selection_change(
    mut commands: Commands,
    roster_ui: Query<Entity, With<RosterUi>>,
    heroes: Query<(Entity, &HeroInfo), With<Hero>>,
    selected: Res<SelectedHero>,
    trait_db: Res<TraitDatabase>,
    hero_query: Query<(&HeroInfo, &HeroStats, &HeroTraits), With<Hero>>,
) {
    // Despawn old roster UI and rebuild
    for entity in &roster_ui {
        commands.entity(entity).despawn();
    }

    let mut root = widgets::ui_root("Roster Screen")
        .insert((DespawnOnExit(GameTab::Roster), RosterUi));

    let top_bar = div()
        .row()
        .w_full()
        .justify_between()
        .items_center()
        .p(px(16.0))
        .child(widgets::header("Roster"))
        .child(widgets::game_button("Back", go_back));

    let hero_list = build_hero_list(&heroes, &selected);
    let detail = build_detail_panel(&selected, &hero_query, &trait_db);

    let content = div()
        .row()
        .w_full()
        .flex_1()
        .gap(px(16.0))
        .p(px(16.0))
        .child(hero_list)
        .child(detail);

    root = root.child(top_bar).child(content);
    root.spawn(&mut commands);
}

fn go_back(_: On<Pointer<Click>>, mut next_tab: ResMut<NextState<GameTab>>) {
    next_tab.set(GameTab::Hub);
}

fn clear_selection(mut selected: ResMut<SelectedHero>) {
    selected.0 = None;
}
