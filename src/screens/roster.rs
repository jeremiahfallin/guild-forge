//! The hero roster screen — view and manage your guild's heroes.

use bevy::prelude::*;
use bevy_declarative::element::div::{Div, div};
use bevy_declarative::element::text::text;
use bevy_declarative::style::styled::Styled;
use bevy_declarative::style::values::{pct, px};

use crate::{
    hero::{
        Favorite, Hero, HeroInfo, HeroStats, HeroTraits, PersonallyManaged, Fatigue, data::*,
        status::{Injured, Missing, format_countdown},
        status_tick::StatusTickSet,
        HeroHistory,
        Epithet, format_hero_name, portrait::HeroPortraitImage,
    },
    mission::OnMission,
    screens::GameTab,
    theme::{palette::*, widgets},
};

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<SelectedHero>();
    app.add_systems(OnEnter(GameTab::Roster), spawn_roster);
    app.add_systems(
        Update,
        (
            refresh_roster_on_selection_change.run_if(resource_changed::<SelectedHero>),
            detect_mission_status_changes,
            tick_status_countdown_refresh,
        )
            // Run after the Missing/Injured tick so the roster sees the
            // post-transition state on the same frame status flips.
            .after(StatusTickSet)
            .run_if(in_state(GameTab::Roster)),
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

#[derive(Component)]
struct RosterListScroll;

#[derive(Component)]
struct DetailPanelScroll;

fn spawn_roster(
    mut commands: Commands,
    gameplay_root: Query<Entity, With<widgets::GameplayRoot>>,
    heroes: Query<
        (
            Entity,
            &HeroInfo,
            &HeroStats,
            Option<&OnMission>,
            Has<Favorite>,
            Has<PersonallyManaged>,
            Option<&Missing>,
            Option<&Injured>,
            &Fatigue,
            Option<&Epithet>,
            Option<&HeroPortraitImage>,
        ),
        With<Hero>,
    >,
    selected: Res<SelectedHero>,
    trait_db: Res<TraitDatabase>,
    hero_query: Query<(&HeroInfo, &HeroStats, &HeroTraits, Has<Favorite>, Has<PersonallyManaged>, &Fatigue, Option<&HeroHistory>, Option<&Epithet>, Option<&HeroPortraitImage>), With<Hero>>,
    time: Res<Time<Virtual>>,
) {
    let Ok(root_entity) = gameplay_root.single() else { return };
    let mut root = widgets::content_area("Roster Screen")
        .insert((DespawnOnExit(GameTab::Roster), RosterUi));

    let top_bar = div()
        .row()
        .w_full()
        .items_center()
        .p(px(16.0))
        .child(widgets::header("Roster"));

    // Main content: two-panel layout
    let hero_list = build_hero_list(&heroes, &selected, time.elapsed_secs_f64(), 0.0);
    let detail = build_detail_panel(&selected, &hero_query, &trait_db, 0.0);

    let content = div()
        .row()
        .w_full()
        .flex_1()
        .min_h(px(0.0))
        .gap(px(16.0))
        .p(px(16.0))
        .child(hero_list)
        .child(detail);

    root = root.child(top_bar).child(content);
    root.spawn_as_child_of(&mut commands, root_entity);
}

/// Stable-sort helper: return the input indices reordered so favorites come first,
/// preserving original order within each group. The input is `(is_favorite, original_index)`.
fn sort_favorites_first(entries: &[(bool, usize)]) -> Vec<usize> {
    let mut indexed: Vec<(bool, usize)> = entries.to_vec();
    // Stable sort: `true` (favorite) should come before `false`. Rust bool
    // sorts false-before-true naturally, so invert with `!`.
    indexed.sort_by_key(|(is_fav, _)| !*is_fav);
    indexed.into_iter().map(|(_, idx)| idx).collect()
}

fn build_hero_list(
    heroes: &Query<
        (
            Entity,
            &HeroInfo,
            &HeroStats,
            Option<&OnMission>,
            Has<Favorite>,
            Has<PersonallyManaged>,
            Option<&Missing>,
            Option<&Injured>,
            &Fatigue,
            Option<&Epithet>,
            Option<&HeroPortraitImage>,
        ),
        With<Hero>,
    >,
    selected: &SelectedHero,
    now: f64,
    saved_scroll_y: f32,
) -> Div {
    let mut list = div()
        .col()
        .w(px(380.0))
        .h_full()
        .min_h(px(0.0))
        .gap(px(8.0))
        .overflow_y_scroll()
        .insert((Name::new("Hero List"), RosterListScroll, ScrollPosition(Vec2::new(0.0, saved_scroll_y))));

    list = list.child(
        text("Heroes")
            .font_size(28.0)
            .color(HEADER_TEXT),
    );

    // Collect hero iteration with favorite flag, then sort favorites to the top.
    let hero_vec: Vec<(
        Entity,
        &HeroInfo,
        &HeroStats,
        Option<&OnMission>,
        bool,
        bool,
        Option<Missing>,
        Option<Injured>,
        &Fatigue,
        Option<&Epithet>,
        Option<&HeroPortraitImage>,
    )> = heroes
        .iter()
        .map(|(e, i, s, om, is_fav, is_managed, missing, injured, f, ep, port)| {
            (e, i, s, om, is_fav, is_managed, missing.cloned(), injured.copied(), f, ep, port)
        })
        .collect();
    let indexed: Vec<(bool, usize)> = hero_vec
        .iter()
        .enumerate()
        .map(|(i, (_, _, _, _, is_fav, ..))| (*is_fav, i))
        .collect();
    let order = sort_favorites_first(&indexed);

    for i in order {
        let (entity, info, stats, on_mission, is_favorite, is_managed, missing, injured, fatigue, epithet, portrait_img) = hero_vec[i].clone();
        let is_selected = selected.0 == Some(entity);
        let is_on_mission = on_mission.is_some();

        let bg_color = if missing.is_some() {
            Color::srgba(0.35, 0.15, 0.15, 0.6) // dim red-gray
        } else if injured.is_some() {
            Color::srgba(0.32, 0.22, 0.12, 0.6) // dim amber
        } else if is_on_mission {
            Color::srgba(0.18, 0.18, 0.22, 0.45) // Grayed out
        } else {
            CARD_BACKGROUND
        };

        let border_color = if is_selected {
            BORDER_GOLD
        } else {
            BORDER_IRON
        };

        let name_color = if is_on_mission {
            Color::srgba(0.5, 0.5, 0.5, 0.6)
        } else {
            HEADER_TEXT
        };

        let class_text = if let Some(m) = missing {
            format!(
                "Lv.{} {} — MISSING {}",
                info.level,
                info.class,
                format_countdown(m.expires_at - now)
            )
        } else if let Some(inj) = injured {
            format!(
                "Lv.{} {} — INJURED {}",
                info.level,
                info.class,
                format_countdown(inj.expires_at - now)
            )
        } else if is_on_mission {
            format!("Lv.{} {} (On Mission)", info.level, info.class)
        } else {
            format!("Lv.{} {}", info.level, info.class)
        };

        // DejaVu Sans (loaded as default UI font) covers ★ ☆ ⚑ ⚐ and more.
        // No emoji though — pin uses a flag glyph instead of 📌.
        let star_glyph = if is_favorite { "★" } else { "☆" };
        let star_color = if is_favorite {
            Color::srgb(1.0, 0.85, 0.2)
        } else {
            Color::srgba(0.5, 0.5, 0.5, 0.7)
        };
        let pin_glyph = if is_managed { "⚑" } else { "⚐" };
        let pin_color = if is_managed {
            Color::srgb(0.5, 0.8, 1.0)
        } else {
            Color::srgba(0.5, 0.5, 0.5, 0.5)
        };

        let max_stamina = fatigue.max(info.level, stats.constitution);
        let stamina_pct = (fatigue.current / max_stamina * 100.0).clamp(0.0, 100.0);
        let is_exhausted = fatigue.current <= 0.0;
        let stamina_bar_color = if is_exhausted {
            Color::srgb(0.9, 0.2, 0.2)
        } else if fatigue.current < 25.0 {
            Color::srgb(0.9, 0.6, 0.2)
        } else {
            Color::srgb(0.2, 0.8, 0.4)
        };

        let mut card = div()
            .row()
            .w_full()
            .p(px(12.0))
            .gap(px(12.0))
            .items_center()
            .bg(bg_color)
            .rounded(px(8.0))
            .insert((SelectHeroButton(entity), BorderColor::all(border_color)))
            .on_click(select_hero);
        card.style_mut().border = UiRect::all(Val::Px(1.5));

        if let Some(portrait_image) = portrait_img {
            card = card.child(
                div()
                    .size(px(44.0))
                    .bg(Color::srgb(0.08, 0.08, 0.1))
                    .rounded(px(4.0))
                    .insert(ImageNode {
                        image: portrait_image.0.clone(),
                        ..default()
                    })
            );
        }

        card = card
            .child(
                div()
                    .col()
                    .flex_1()
                    .child(
                        text(format_hero_name(&info.name, epithet))
                            .font_size(22.0)
                            .color(name_color),
                    )
                    .child(
                        text(class_text)
                            .font_size(16.0)
                            .color(LABEL_TEXT),
                    )
                    .child(
                        div()
                            .row()
                            .items_center()
                            .gap(px(6.0))
                            .child(
                                text(format!("Stamina: {:.0}%", stamina_pct))
                                    .font_size(12.0)
                                    .color(LABEL_TEXT)
                                    .w(px(85.0)),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .h(px(6.0))
                                    .bg(Color::srgba(0.1, 0.1, 0.15, 0.8))
                                    .rounded(px(2.0))
                                    .child(
                                        div()
                                            .w(pct(stamina_pct))
                                            .h_full()
                                            .bg(stamina_bar_color)
                                            .rounded(px(2.0)),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .col()
                    .gap(px(4.0))
                    .items_center()
                    .child(
                        div()
                            .p(px(4.0))
                            .rounded(px(4.0))
                            .items_center()
                            .justify_center()
                            .insert((Button, ToggleFavoriteButton(entity)))
                            .on_click(toggle_favorite)
                            .interaction_palette(
                                Color::NONE,
                                Color::srgba(1.0, 1.0, 1.0, 0.10),
                                Color::srgba(1.0, 1.0, 1.0, 0.18),
                            )
                            .child(
                                text(star_glyph)
                                    .font_size(20.0)
                                    .color(star_color)
                                    .insert(Pickable::IGNORE),
                            ),
                    )
                    .child(
                        div()
                            .p(px(4.0))
                            .rounded(px(4.0))
                            .items_center()
                            .justify_center()
                            .insert((Button, ToggleManagedButton(entity)))
                            .on_click(toggle_managed)
                            .interaction_palette(
                                Color::NONE,
                                Color::srgba(1.0, 1.0, 1.0, 0.10),
                                Color::srgba(1.0, 1.0, 1.0, 0.18),
                            )
                            .child(
                                text(pin_glyph)
                                    .font_size(16.0)
                                    .color(pin_color)
                                    .insert(Pickable::IGNORE),
                            ),
                    ),
            );

        list = list.child(card);
    }

    list
}

fn build_detail_panel(
    selected: &SelectedHero,
    hero_query: &Query<
        (
            &HeroInfo,
            &HeroStats,
            &HeroTraits,
            Has<Favorite>,
            Has<PersonallyManaged>,
            &Fatigue,
            Option<&HeroHistory>,
            Option<&Epithet>,
            Option<&HeroPortraitImage>,
        ),
        With<Hero>,
    >,
    trait_db: &TraitDatabase,
    saved_scroll_y: f32,
) -> Div {
    let mut panel = div()
        .col()
        .flex_1()
        .h_full()
        .min_h(px(0.0))
        .p(px(20.0))
        .gap(px(16.0))
        .bg(Color::srgba(0.12, 0.13, 0.18, 0.95))
        .rounded(px(8.0))
        .overflow_y_scroll()
        .insert((Name::new("Detail Panel"), DetailPanel, DetailPanelScroll, ScrollPosition(Vec2::new(0.0, saved_scroll_y)), BorderColor::all(BORDER_BRONZE)));
    panel.style_mut().border = UiRect::all(Val::Px(1.5));

    let Some(entity) = selected.0 else {
        return panel.child(
            text("Select a hero to view details")
                .font_size(24.0)
                .color(Color::srgba(0.6, 0.6, 0.6, 0.8)),
        );
    };

    let Ok((info, stats, traits, is_favorite, is_managed, fatigue, history, epithet, portrait_img)) = hero_query.get(entity) else {
        return panel.child(
            text("Hero not found")
                .font_size(24.0)
                .color(Color::srgba(0.8, 0.3, 0.3, 1.0)),
        );
    };

    // Hero header
    let mut header_row = div()
        .row()
        .gap(px(16.0))
        .items_center();

    if let Some(portrait_image) = portrait_img {
        header_row = header_row.child(
            div()
                .size(px(80.0))
                .bg(Color::srgb(0.08, 0.08, 0.1))
                .rounded(px(6.0))
                .insert(ImageNode {
                    image: portrait_image.0.clone(),
                    ..default()
                })
        );
    }

    let header_details = div()
        .col()
        .gap(px(4.0))
        .child(
            text(format_hero_name(&info.name, epithet)).font_size(36.0).color(HEADER_TEXT),
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
        )
        .child({
            let status_parts: Vec<&str> = [
                is_favorite.then_some("★ Favorite"),
                is_managed.then_some("⚑ Personally Managed"),
            ]
            .into_iter()
            .flatten()
            .collect();
            let status_text = if status_parts.is_empty() {
                String::new()
            } else {
                status_parts.join("   ")
            };
            text(status_text).font_size(14.0).color(Color::srgb(0.9, 0.85, 0.4))
        });

    header_row = header_row.child(header_details);

    // Stamina section
    let max_stamina = fatigue.max(info.level, stats.constitution);
    let stamina_pct = (fatigue.current / max_stamina * 100.0).clamp(0.0, 100.0);
    let is_exhausted = fatigue.current <= 0.0;
    
    let stamina_bar_color = if is_exhausted {
        Color::srgb(0.9, 0.2, 0.2)
    } else if fatigue.current < 25.0 {
        Color::srgb(0.9, 0.6, 0.2)
    } else {
        Color::srgb(0.2, 0.8, 0.4)
    };

    let stamina_section = div()
        .col()
        .gap(px(6.0))
        .child(
            text("Stamina").font_size(24.0).color(HEADER_TEXT),
        )
        .child(
            div()
                .row()
                .items_center()
                .gap(px(8.0))
                .child(
                    text(format!("{:.0} / {:.0}", fatigue.current, max_stamina))
                        .font_size(16.0)
                        .color(LABEL_TEXT)
                        .w(px(100.0)),
                )
                .child(
                    div()
                        .flex_1()
                        .h(px(12.0))
                        .bg(Color::srgba(0.1, 0.1, 0.15, 0.8))
                        .rounded(px(4.0))
                        .child(
                            div()
                                .w(pct(stamina_pct))
                                .h_full()
                                .bg(stamina_bar_color)
                                .rounded(px(4.0)),
                        ),
                )
                .child(
                    if is_exhausted {
                        text("EXHAUSTED!").font_size(14.0).color(Color::srgb(0.9, 0.2, 0.2))
                    } else {
                        text("").font_size(14.0)
                    }
                )
        );

    // Stats section
    let stats_section = build_stats_section(stats);

    // Traits section
    let traits_section = build_traits_section(&traits.0, trait_db);

    // Veteran Perks section
    let perks_section = build_perks_section(history);

    // History section
    let history_section = build_history_section(history);

    panel
        .child(header_row)
        .child(stamina_section)
        .child(stats_section)
        .child(traits_section)
        .child(perks_section)
        .child(history_section)
}

fn build_history_section(history: Option<&HeroHistory>) -> Div {
    let default_history = HeroHistory::default();
    let history = history.unwrap_or(&default_history);

    let mut section = div()
        .col()
        .gap(px(12.0))
        .child(
            text("Career History").font_size(24.0).color(HEADER_TEXT),
        );

    // Grid of stats
    let stats_list = vec![
        ("Missions Run", history.missions_run.to_string()),
        ("Kills", history.kills.to_string()),
        ("Near-Deaths", history.near_deaths.to_string()),
        ("Lifetime Gold", history.lifetime_gold.to_string()),
        ("Rescues Given", history.rescues_given.to_string()),
        ("Rescues Recv", history.rescues_received.to_string()),
    ];

    let mut stats_grid = div().col().gap(px(6.0));
    for chunk in stats_list.chunks(2) {
        let mut row = div().row().w_full().gap(px(6.0));
        for (label, val) in chunk {
            let mut cell = div()
                .row()
                .flex_1()
                .justify_between()
                .items_center()
                .p(px(8.0))
                .bg(Color::srgba(0.2, 0.2, 0.35, 0.3))
                .rounded(px(4.0))
                .insert(BorderColor::all(BORDER_IRON));
            cell.style_mut().border = UiRect::all(Val::Px(1.0));
            
            cell = cell
                .child(text(*label).font_size(15.0).color(LABEL_TEXT))
                .child(text(val.clone()).font_size(15.0).color(BUTTON_TEXT));
            row = row.child(cell);
        }
        if chunk.len() == 1 {
            row = row.child(div().flex_1());
        }
        stats_grid = stats_grid.child(row);
    }
    section = section.child(stats_grid);

    // Timeline section
    let mut timeline_col = div().col().gap(px(6.0));
    
    timeline_col = timeline_col.child(
        text("Timeline").font_size(18.0).color(HEADER_TEXT)
    );

    let mut list_col = div().col().gap(px(6.0));
    if history.timeline.is_empty() {
        list_col = list_col.child(
            text("No recorded history.")
                .font_size(14.0)
                .color(Color::srgba(0.5, 0.5, 0.5, 0.8))
        );
    } else {
        for entry in &history.timeline {
            list_col = list_col.child(
                div()
                    .row()
                    .items_start()
                    .gap(px(8.0))
                    .child(
                        text("•")
                            .font_size(16.0)
                            .color(BORDER_GOLD)
                    )
                    .child(
                        text(entry)
                            .font_size(15.0)
                            .color(BUTTON_TEXT)
                    )
            );
        }
    }
    timeline_col = timeline_col.child(list_col);
    section = section.child(timeline_col);

    section
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

fn build_perks_section(history: Option<&HeroHistory>) -> Div {
    let mut section = div()
        .col()
        .gap(px(6.0))
        .child(
            text("Veteran Perks").font_size(24.0).color(HEADER_TEXT),
        );

    let earned = history
        .map(|h| crate::hero::perk::get_earned_perks(h))
        .unwrap_or_default();

    if earned.is_empty() {
        return section.child(
            text("None (Earned via career milestones)")
                .font_size(16.0)
                .color(Color::srgba(0.5, 0.5, 0.5, 0.8)),
        );
    }

    for perk in earned {
        section = section.child(
            div()
                .row()
                .gap(px(8.0))
                .p(px(8.0))
                .bg(Color::srgba(0.35, 0.28, 0.15, 0.35)) // Warm bronze gold backdrop
                .rounded(px(4.0))
                .child(
                    div()
                        .col()
                        .child(
                            text(perk.name())
                                .font_size(18.0)
                                .color(BORDER_GOLD), // Gold-tinted title
                        )
                        .child(
                            text(perk.description())
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
    gameplay_root: Query<Entity, With<widgets::GameplayRoot>>,
    roster_ui: Query<Entity, With<RosterUi>>,
    scroll_list_q: Query<&ScrollPosition, With<RosterListScroll>>,
    scroll_detail_q: Query<&ScrollPosition, With<DetailPanelScroll>>,
    heroes: Query<
        (
            Entity,
            &HeroInfo,
            &HeroStats,
            Option<&OnMission>,
            Has<Favorite>,
            Has<PersonallyManaged>,
            Option<&Missing>,
            Option<&Injured>,
            &Fatigue,
            Option<&Epithet>,
            Option<&HeroPortraitImage>,
        ),
        With<Hero>,
    >,
    selected: Res<SelectedHero>,
    trait_db: Res<TraitDatabase>,
    hero_query: Query<(&HeroInfo, &HeroStats, &HeroTraits, Has<Favorite>, Has<PersonallyManaged>, &Fatigue, Option<&HeroHistory>, Option<&Epithet>, Option<&HeroPortraitImage>), With<Hero>>,
    time: Res<Time<Virtual>>,
) {
    let Ok(root_entity) = gameplay_root.single() else { return };

    // Read old scroll positions before despawning
    let saved_list_y = scroll_list_q.iter().next().map(|s| s.0.y).unwrap_or(0.0);
    let saved_detail_y = scroll_detail_q.iter().next().map(|s| s.0.y).unwrap_or(0.0);

    // Despawn old roster UI and rebuild
    for entity in &roster_ui {
        commands.entity(entity).despawn();
    }

    let mut root = widgets::content_area("Roster Screen")
        .insert((DespawnOnExit(GameTab::Roster), RosterUi));

    let top_bar = div()
        .row()
        .w_full()
        .items_center()
        .p(px(16.0))
        .child(widgets::header("Roster"));

    let hero_list = build_hero_list(&heroes, &selected, time.elapsed_secs_f64(), saved_list_y);
    let detail = build_detail_panel(&selected, &hero_query, &trait_db, saved_detail_y);

    let content = div()
        .row()
        .w_full()
        .flex_1()
        .min_h(px(0.0))
        .gap(px(16.0))
        .p(px(16.0))
        .child(hero_list)
        .child(detail);

    root = root.child(top_bar).child(content);
    root.spawn_as_child_of(&mut commands, root_entity);
}

fn clear_selection(mut selected: ResMut<SelectedHero>) {
    selected.0 = None;
}

/// Force a roster rebuild ~once per game-second while any hero has a
/// Missing or Injured timer running, so countdown labels visibly tick down
/// instead of sitting stale until the next selection or status change.
///
/// Uses `Time<Virtual>` so paused/sped-up game time is respected, and
/// touches `SelectedHero` to reuse the existing rebuild path.
fn tick_status_countdown_refresh(
    mut timer: Local<f32>,
    time: Res<Time<Virtual>>,
    has_status: Query<(), Or<(With<Missing>, With<Injured>)>>,
    mut selected: ResMut<SelectedHero>,
) {
    if has_status.is_empty() {
        *timer = 0.0;
        return;
    }
    *timer += time.delta_secs();
    if *timer >= 1.0 {
        *timer = 0.0;
        selected.set_changed();
    }
}

/// Detect when heroes gain or lose `OnMission` and force a roster rebuild
/// by touching the `SelectedHero` resource (triggers change detection).
fn detect_mission_status_changes(
    heroes: Query<(Entity, Option<&OnMission>), With<Hero>>,
    mut last_on_mission: Local<Vec<Entity>>,
    mut selected: ResMut<SelectedHero>,
) {
    let mut current: Vec<Entity> = heroes
        .iter()
        .filter_map(|(e, om)| om.map(|_| e))
        .collect();
    current.sort();

    if *last_on_mission != current {
        *last_on_mission = current;
        // Touch the resource to trigger refresh_roster_on_selection_change
        selected.set_changed();
    }
}

/// Component on the star icon inside a hero row; toggles `Favorite` on click.
#[derive(Component)]
struct ToggleFavoriteButton(Entity);

/// Component on the pin icon inside a hero row; toggles `PersonallyManaged` on click.
#[derive(Component)]
struct ToggleManagedButton(Entity);

fn toggle_favorite(
    click: On<Pointer<Click>>,
    buttons: Query<&ToggleFavoriteButton>,
    favorites: Query<(), With<Favorite>>,
    mut commands: Commands,
    mut selected: ResMut<SelectedHero>,
) {
    let Ok(button) = buttons.get(click.event_target()) else { return };
    if favorites.get(button.0).is_ok() {
        commands.entity(button.0).remove::<Favorite>();
    } else {
        commands.entity(button.0).insert(Favorite);
    }
    // Force a roster rebuild so the sort and icon state update.
    selected.set_changed();
}

fn toggle_managed(
    click: On<Pointer<Click>>,
    buttons: Query<&ToggleManagedButton>,
    managed: Query<(), With<PersonallyManaged>>,
    mut commands: Commands,
    mut selected: ResMut<SelectedHero>,
) {
    let Ok(button) = buttons.get(click.event_target()) else { return };
    if managed.get(button.0).is_ok() {
        commands.entity(button.0).remove::<PersonallyManaged>();
    } else {
        commands.entity(button.0).insert(PersonallyManaged);
    }
    selected.set_changed();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sort_with_favorites_first_puts_favorite_entries_before_non_favorites() {
        let input: Vec<(bool, usize)> = vec![
            (false, 0),
            (true, 1),
            (false, 2),
            (true, 3),
            (false, 4),
        ];
        let sorted = sort_favorites_first(&input);
        // Favorites (index 1, 3) come first in their original order;
        // non-favorites (0, 2, 4) follow in their original order.
        assert_eq!(sorted, vec![1, 3, 0, 2, 4]);
    }

    #[test]
    fn sort_with_no_favorites_preserves_input_order() {
        let input: Vec<(bool, usize)> = vec![
            (false, 0),
            (false, 1),
            (false, 2),
        ];
        let sorted = sort_favorites_first(&input);
        assert_eq!(sorted, vec![0, 1, 2]);
    }

    #[test]
    fn sort_with_all_favorites_preserves_input_order() {
        let input: Vec<(bool, usize)> = vec![
            (true, 0),
            (true, 1),
            (true, 2),
        ];
        let sorted = sort_favorites_first(&input);
        assert_eq!(sorted, vec![0, 1, 2]);
    }
}
