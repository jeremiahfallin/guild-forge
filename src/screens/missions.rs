//! The mission board screen — select and dispatch missions.

use bevy::prelude::*;
use bevy_declarative::element::div::div;
use bevy_declarative::element::text::text;
use bevy_declarative::style::styled::Styled;
use bevy_declarative::style::values::px;

use crate::{
    mission::data::{MissionTemplateDatabase, MissionModifier},
    mission::dungeon::DungeonMap,
    screens::GameTab,
    theme::{palette::*, widgets},
};
use rand::Rng;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<MissionBoard>();
    app.add_systems(Update, update_mission_board.run_if(in_state(GameTab::Missions)));
}

/// Active mission board offers.
#[derive(Resource, Default, Debug, Clone)]
pub struct MissionBoard {
    pub offers: Vec<MissionOffer>,
    pub rescue_offers: Vec<RescueOffer>,
}

#[derive(Debug, Clone)]
pub struct RescueOffer {
    pub template_idx: usize,
    pub modifiers: Vec<MissionModifier>,
    pub map: DungeonMap,
    pub rescue_heroes: Vec<Entity>,
    pub expires_at: f64,
}

#[derive(Debug, Clone)]
pub struct MissionOffer {
    pub template_idx: usize,
    pub modifiers: Vec<MissionModifier>,
}

/// Tracks which mission offer index in MissionBoard the player selected.
#[derive(Resource, Default, Debug)]
pub struct SelectedMission {
    pub index: Option<usize>,
    pub is_rescue: bool,
}

/// Component on mission list buttons, storing which offer index they represent.
#[derive(Component)]
struct SelectMissionButton(usize);

/// Component on rescue mission buttons, storing which rescue offer index they represent.
#[derive(Component)]
struct SelectRescueButton(usize);

/// Marker for the mission board UI root.
#[derive(Component)]
struct MissionBoardUi;

fn update_mission_board(
    mut commands: Commands,
    mut timer: Local<f32>,
    time: Res<Time<Virtual>>,
    gameplay_root: Query<Entity, With<widgets::GameplayRoot>>,
    templates: Option<Res<MissionTemplateDatabase>>,
    reputation: Res<crate::reputation::Reputation>,
    mut board: ResMut<MissionBoard>,
    board_ui: Query<Entity, With<MissionBoardUi>>,
    hero_infos: Query<(&crate::hero::HeroInfo, Option<&crate::hero::Epithet>)>,
    buildings: Res<crate::buildings::GuildBuildings>,
    mission_q: Query<&crate::mission::MissionProgress, With<crate::mission::Mission>>,
    mut last_active: Local<Option<usize>>,
) {
    let has_ui = !board_ui.is_empty();
    let mut should_rebuild = !has_ui;

    let active = crate::mission::active_mission_count(&mission_q);
    if *last_active != Some(active) {
        *last_active = Some(active);
        should_rebuild = true;
    }

    if has_ui && !board.rescue_offers.is_empty() {
        *timer += time.delta_secs();
        if *timer >= 1.0 {
            *timer = 0.0;
            should_rebuild = true;
        }
    } else {
        *timer = 0.0;
    }

    if !should_rebuild {
        return;
    }

    let Ok(root_entity) = gameplay_root.single() else { return };
    commands.init_resource::<SelectedMission>();

    // Clean up old UI
    for entity in &board_ui {
        commands.entity(entity).despawn();
    }

    if let Some(templates) = &templates {
        if board.offers.len() != templates.0.len() {
            board.offers.clear();
            let mut rng = rand::rng();
            for (idx, template) in templates.0.iter().enumerate() {
                let mut modifiers = Vec::new();
                if !template.allowed_modifiers.is_empty() {
                    let num_modifiers = rng.random_range(0..=2);
                    if num_modifiers > 0 {
                        let mut pool = template.allowed_modifiers.clone();
                        for _ in 0..num_modifiers {
                            if pool.is_empty() {
                                break;
                            }
                            let p_idx = rng.random_range(0..pool.len());
                            modifiers.push(pool.remove(p_idx));
                        }
                    }
                }
                board.offers.push(MissionOffer {
                    template_idx: idx,
                    modifiers,
                });
            }
        }
    }

    let mut root = widgets::content_area("Mission Board")
        .insert((DespawnOnExit(GameTab::Missions), MissionBoardUi));

    // Top bar
    let cap = buildings.mission_cap();
    let counter_color = if buildings.can_dispatch(active) {
        LABEL_TEXT
    } else {
        Color::srgb(0.9, 0.35, 0.25)
    };
    let top_bar = div()
        .row()
        .w_full()
        .justify_between()
        .items_center()
        .p(px(16.0))
        .child(widgets::header("Mission Board"))
        .child(
            text(format!("Underway: {active}/{cap}"))
                .font_size(22.0)
                .color(counter_color),
        );

    root = root.child(top_bar);

    // Mission list
    if let Some(templates) = templates {
        let mut list = div()
            .col()
            .w_full()
            .flex_1()
            .min_h(px(0.0))
            .gap(px(12.0))
            .p(px(16.0))
            .items_center()
            .overflow_y_scroll()
            .insert((Name::new("Mission List"), ScrollPosition::default()));

        // Render rescue offers pinned to the top
        for (idx, offer) in board.rescue_offers.iter().enumerate() {
            let template = &templates.0[offer.template_idx];
            let remaining_secs = (offer.expires_at - time.elapsed_secs_f64()).max(0.0);
            let countdown_str = crate::hero::status::format_countdown(remaining_secs);

            let rescued_names: Vec<String> = offer
                .rescue_heroes
                .iter()
                .filter_map(|e| hero_infos.get(*e).ok().map(|(hi, ep)| crate::hero::format_hero_name(&hi.name, ep)))
                .collect();
            let rescued_names_str = rescued_names.join(", ");

            let info_row = div()
                .row()
                .gap(px(16.0))
                .child(
                    text(format!("Rescue Window: {countdown_str}"))
                        .font_size(16.0)
                        .color(Color::srgb(1.0, 0.4, 0.4)),
                )
                .child(
                    text(format!("Rescuing: {rescued_names_str}"))
                        .font_size(16.0)
                        .color(Color::srgb(0.9, 0.7, 0.7)),
                );

            let mut card = div()
                .row()
                .w(px(700.0))
                .p(px(16.0))
                .gap(px(16.0))
                .items_center()
                .bg(Color::srgba(0.25, 0.08, 0.08, 0.6))
                .rounded(px(8.0))
                .insert((
                    SelectRescueButton(idx),
                    BorderColor::all(Color::srgb(0.9, 0.2, 0.2)),
                    bevy_declarative::InteractionPalette {
                        none: Color::srgba(0.25, 0.08, 0.08, 0.6),
                        hovered: Color::srgba(0.35, 0.12, 0.12, 0.8),
                        pressed: Color::srgba(0.20, 0.05, 0.05, 0.9),
                    }
                ))
                .on_click(select_rescue_mission);
            card.style_mut().border = UiRect::all(Val::Px(2.5));

            let mut name_row = div().row().items_center().gap(px(8.0))
                .child(
                    text("⚠️ RESCUE REQUIRED:")
                        .font_size(18.0)
                        .color(Color::srgb(1.0, 0.3, 0.3))
                )
                .child(
                    text(&template.name)
                        .font_size(22.0)
                        .color(Color::srgb(1.0, 0.8, 0.8)),
                );

            for modifier in &offer.modifiers {
                let badge_bg = match modifier {
                    MissionModifier::Bountiful => Color::srgb(0.1, 0.5, 0.2),
                    MissionModifier::CursedGround => Color::srgb(0.6, 0.1, 0.1),
                    MissionModifier::Infested => Color::srgb(0.7, 0.5, 0.1),
                    MissionModifier::Trapped => Color::srgb(0.5, 0.1, 0.5),
                    MissionModifier::Foggy => Color::srgb(0.4, 0.4, 0.4),
                };
                let badge_text = modifier.to_string();
                name_row = name_row.child(
                    div()
                        .p(px(4.0))
                        .bg(badge_bg)
                        .rounded(px(4.0))
                        .child(
                            text(badge_text)
                                .font_size(12.0)
                                .color(Color::srgb(1.0, 1.0, 1.0)),
                        )
                );
            }

            list = list.child(
                card.child(
                    div()
                        .col()
                        .flex_1()
                        .gap(px(4.0))
                        .child(name_row)
                        .child(
                            text(&template.description)
                                .font_size(16.0)
                                .color(Color::srgb(0.9, 0.8, 0.8)),
                        )
                        .child(info_row),
                ),
            );
        }

        // Render standard offers
        for (idx, offer) in board.offers.iter().enumerate() {
            let template = &templates.0[offer.template_idx];
            if reputation.0 < template.reputation_required {
                continue;
            }
            let difficulty_stars = "★".repeat(template.difficulty as usize);
            let gold_range = format!(
                "Gold: {}-{}",
                template.gold_reward.min, template.gold_reward.max
            );

            // Material drops summary
            let drops_text: String = template
                .material_drops
                .iter()
                .map(|(mat, min, max)| {
                    if min == max {
                        format!("{} {}", min, mat.name())
                    } else {
                        format!("{}-{} {}", min, max, mat.name())
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");

            let mut info_row = div()
                .row()
                .gap(px(16.0))
                .child(
                    text(format!("Difficulty: {difficulty_stars}"))
                        .font_size(16.0)
                        .color(Color::srgb(0.9, 0.7, 0.2)),
                )
                .child(
                    text(gold_range)
                        .font_size(16.0)
                        .color(Color::srgb(0.8, 0.7, 0.3)),
                );
            if template.reputation_required > 0 {
                info_row = info_row.child(
                    text(format!("Req: {} rep", template.reputation_required))
                        .font_size(16.0)
                        .color(Color::srgb(0.6, 0.8, 0.9)),
                );
            }

            let drops_row = div()
                .row()
                .gap(px(4.0))
                .child(
                    text("Drops:")
                        .font_size(14.0)
                        .color(Color::srgb(0.6, 0.7, 0.6)),
                )
                .child(
                    text(drops_text)
                        .font_size(14.0)
                        .color(Color::srgb(0.5, 0.7, 0.5)),
                );

            let mut card = div()
                .row()
                .w(px(700.0))
                .p(px(16.0))
                .gap(px(16.0))
                .items_center()
                .bg(CARD_BACKGROUND)
                .rounded(px(8.0))
                .insert((SelectMissionButton(idx), BorderColor::all(BORDER_IRON)))
                .on_click(select_mission);
            card.style_mut().border = UiRect::all(Val::Px(1.5));

            let mut name_row = div().row().items_center().gap(px(8.0)).child(
                text(&template.name)
                    .font_size(22.0)
                    .color(HEADER_TEXT),
            );

            for modifier in &offer.modifiers {
                let badge_bg = match modifier {
                    MissionModifier::Bountiful => Color::srgb(0.1, 0.5, 0.2),
                    MissionModifier::CursedGround => Color::srgb(0.6, 0.1, 0.1),
                    MissionModifier::Infested => Color::srgb(0.7, 0.5, 0.1),
                    MissionModifier::Trapped => Color::srgb(0.5, 0.1, 0.5),
                    MissionModifier::Foggy => Color::srgb(0.4, 0.4, 0.4),
                };
                let badge_text = modifier.to_string();
                name_row = name_row.child(
                    div()
                        .p(px(4.0))
                        .bg(badge_bg)
                        .rounded(px(4.0))
                        .child(
                            text(badge_text)
                                .font_size(12.0)
                                .color(Color::srgb(1.0, 1.0, 1.0)),
                        )
                );
            }

            list = list.child(
                card.child(
                    div()
                        .col()
                        .flex_1()
                        .gap(px(4.0))
                        .child(name_row)
                        .child(
                            text(&template.description)
                                .font_size(16.0)
                                .color(LABEL_TEXT),
                        )
                        .child(info_row)
                        .child(drops_row),
                ),
            );
        }

        root = root.child(list);
    } else {
        root = root.child(widgets::label("Loading missions..."));
    }

    root.spawn_as_child_of(&mut commands, root_entity);
}

fn select_mission(
    click: On<Pointer<Click>>,
    buttons: Query<&SelectMissionButton>,
    mut selected: ResMut<SelectedMission>,
    mut next_tab: ResMut<NextState<GameTab>>,
) {
    if let Ok(button) = buttons.get(click.event_target()) {
        selected.index = Some(button.0);
        selected.is_rescue = false;
        next_tab.set(GameTab::PartySelect);
    }
}

fn select_rescue_mission(
    click: On<Pointer<Click>>,
    buttons: Query<&SelectRescueButton>,
    mut selected: ResMut<SelectedMission>,
    mut next_tab: ResMut<NextState<GameTab>>,
) {
    if let Ok(button) = buttons.get(click.event_target()) {
        selected.index = Some(button.0);
        selected.is_rescue = true;
        next_tab.set(GameTab::PartySelect);
    }
}

pub fn clear_selection(mut commands: Commands) {
    commands.remove_resource::<SelectedMission>();
}
