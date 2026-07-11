//! Party selection screen — pick heroes for a mission then dispatch.

use bevy::prelude::*;
use bevy_declarative::element::div::div;
use bevy_declarative::element::text::text;
use bevy_declarative::style::styled::Styled;
use bevy_declarative::style::values::{pct, px};
use rand::Rng;

use crate::{
    localization::{tr, trf},
    hero::{Hero, HeroInfo, HeroStats},
    mission::{
        Mission, MissionDungeon, MissionInfo, MissionParty, MissionProgress, OnMission,
        ViewedMission,
        data::{EnemyDatabase, MissionTemplateDatabase},
        dungeon::generate_dungeon,
        entities::RoomStatus,
    },
    screens::{
        GameTab,
        missions::SelectedMission,
    },
    theme::{palette::*, widgets},
};

const MAX_PARTY_SIZE: usize = 4;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(GameTab::PartySelect), init_party_select);
    app.add_systems(
        Update,
        refresh_party_select.run_if(in_state(GameTab::PartySelect)),
    );
    app.add_systems(OnExit(GameTab::PartySelect), (cleanup_party_select, crate::screens::missions::clear_selection));
}

/// Tracks which heroes have been selected for the party.
#[derive(Resource, Default, Debug)]
pub struct SelectedParty(pub Vec<Entity>);

/// Marker for the party select UI root.
#[derive(Component)]
struct PartySelectUi;

/// Component on hero buttons in the available list.
#[derive(Component)]
struct AddHeroButton(Entity);

/// Component on hero buttons in the selected list.
#[derive(Component)]
struct RemoveHeroButton(Entity);

/// Marker for the dispatch button.
#[derive(Component)]
struct DispatchButton;

fn init_party_select(mut commands: Commands) {
    commands.init_resource::<SelectedParty>();
}

fn build_available_panel(
    heroes: &Query<(Entity, &HeroInfo), (With<Hero>, Without<OnMission>, Without<crate::hero::status::Missing>)>,
    selected_entities: &[Entity],
) -> bevy_declarative::element::div::Div {
    let mut panel = div()
        .col()
        .w(pct(50.0))
        .h_full()
        .min_h(px(0.0))
        .gap(px(8.0))
        .p(px(16.0))
        .bg(Color::srgba(0.15, 0.15, 0.25, 0.6))
        .rounded(px(8.0))
        .overflow_y_scroll()
        .insert((Name::new("Available Heroes"), ScrollPosition::default()));

    panel = panel.child(
        text(tr("party.available_heroes"))
            .font_size(24.0)
            .color(HEADER_TEXT),
    );

    for (entity, info) in heroes.iter() {
        let already_selected = selected_entities.contains(&entity);
        let bg_color = if already_selected {
            Color::srgba(0.3, 0.3, 0.3, 0.4) // Grayed out
        } else {
            Color::srgba(0.2, 0.2, 0.3, 0.6)
        };
        let text_color = if already_selected {
            Color::srgba(0.5, 0.5, 0.5, 0.6)
        } else {
            HEADER_TEXT
        };

        let mut row = div()
            .row()
            .w_full()
            .p(px(10.0))
            .gap(px(10.0))
            .items_center()
            .bg(bg_color)
            .rounded(px(6.0));

        if !already_selected {
            row = row
                .insert(AddHeroButton(entity))
                .on_click(add_hero_to_party);
        }

        panel = panel.child(
            row.child(
                div()
                    .col()
                    .flex_1()
                    .child(
                        text(&info.name).font_size(20.0).color(text_color),
                    )
                    .child(
                        text(trf("common.hero_level_class", &[("level", &info.level.to_string()), ("class", &info.class.to_string())]))
                            .font_size(14.0)
                            .color(LABEL_TEXT),
                    ),
            ),
        );
    }

    if heroes.is_empty() {
        panel = panel.child(
            text(tr("party.no_heroes"))
                .font_size(16.0)
                .color(Color::srgba(0.6, 0.6, 0.6, 0.8)),
        );
    }

    panel
}

fn refresh_party_select(
    mut commands: Commands,
    mut timer: Local<f32>,
    time: Res<Time<Virtual>>,
    gameplay_root: Query<Entity, With<widgets::GameplayRoot>>,
    ui_q: Query<Entity, With<PartySelectUi>>,
    selected_party: Res<SelectedParty>,
    selected_mission: Option<Res<SelectedMission>>,
    templates: Option<Res<MissionTemplateDatabase>>,
    heroes: Query<(Entity, &HeroInfo), (With<Hero>, Without<OnMission>, Without<crate::hero::status::Missing>)>,
    hero_info: Query<&HeroInfo, With<Hero>>,
    board: Option<Res<crate::screens::missions::MissionBoard>>,
    buildings: Res<crate::buildings::GuildBuildings>,
    mission_q: Query<&MissionProgress, With<Mission>>,
    mut last_active: Local<Option<usize>>,
) {
    let has_ui = !ui_q.is_empty();
    let is_rescue = selected_mission
        .as_ref()
        .map(|sm| sm.is_rescue)
        .unwrap_or(false);

    let mut should_rebuild = !has_ui || selected_party.is_changed();

    let active = crate::mission::active_mission_count(&mission_q);
    if *last_active != Some(active) {
        *last_active = Some(active);
        should_rebuild = true;
    }

    if has_ui && is_rescue {
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

    // Despawn old UI
    for entity in &ui_q {
        commands.entity(entity).despawn();
    }

    let mission_name = selected_mission
        .as_ref()
        .and_then(|sm| sm.index)
        .and_then(|idx| {
            if let Some(selected_sm) = selected_mission.as_ref() {
                if selected_sm.is_rescue {
                    board.as_ref().and_then(|b| b.rescue_offers.get(idx)).map(|o| o.template_idx)
                } else {
                    board.as_ref().and_then(|b| b.offers.get(idx)).map(|o| o.template_idx)
                }
            } else {
                None
            }
        })
        .and_then(|template_idx| templates.as_ref().and_then(|t| t.0.get(template_idx)))
        .map(|t| t.name.as_str())
        .unwrap_or(tr("party.unknown_mission"));

    let mut root = widgets::content_area("Party Select")
        .insert((DespawnOnExit(GameTab::PartySelect), PartySelectUi));

    // Top bar
    let mut title_row = div().row().items_center().gap(px(12.0))
        .child(widgets::header(trf("party.header", &[("mission", mission_name)])));

    if let Some(selected_sm) = selected_mission.as_ref() {
        if selected_sm.is_rescue {
            if let Some(mission_idx) = selected_sm.index {
                if let Some(board) = board.as_ref() {
                    if let Some(offer) = board.rescue_offers.get(mission_idx) {
                        let remaining_secs = (offer.expires_at - time.elapsed_secs_f64()).max(0.0);
                        let countdown_str = crate::hero::status::format_countdown(remaining_secs);
                        title_row = title_row.child(
                            text(trf("party.remaining", &[("time", &countdown_str)]))
                                .font_size(24.0)
                                .color(Color::srgb(1.0, 0.4, 0.4))
                        );
                    } else {
                        title_row = title_row.child(
                            text(tr("party.expired"))
                                .font_size(24.0)
                                .color(Color::srgb(1.0, 0.4, 0.4))
                        );
                    }
                }
            }
        }
    }

    let top_bar = div()
        .row()
        .w_full()
        .justify_between()
        .items_center()
        .p(px(16.0))
        .child(title_row)
        .child(widgets::game_button(tr("party.cancel"), go_back_to_missions));

    // Available heroes panel
    let available_panel = build_available_panel(&heroes, &selected_party.0);

    // Selected party panel
    let mut party_panel = div()
        .col()
        .w(pct(50.0))
        .h_full()
        .min_h(px(0.0))
        .gap(px(8.0))
        .p(px(16.0))
        .bg(Color::srgba(0.15, 0.2, 0.15, 0.6))
        .rounded(px(8.0))
        .overflow_y_scroll()
        .insert((Name::new("Selected Party"), ScrollPosition::default()));

    party_panel = party_panel.child(
        text(trf(
            "party.selected_party",
            &[
                ("count", &selected_party.0.len().to_string()),
                ("max", &MAX_PARTY_SIZE.to_string()),
            ],
        ))
        .font_size(24.0)
        .color(HEADER_TEXT),
    );

    for &entity in &selected_party.0 {
        if let Ok(info) = hero_info.get(entity) {
            party_panel = party_panel.child(
                div()
                    .row()
                    .w_full()
                    .p(px(10.0))
                    .gap(px(10.0))
                    .items_center()
                    .bg(Color::srgba(0.2, 0.35, 0.2, 0.6))
                    .rounded(px(6.0))
                    .insert(RemoveHeroButton(entity))
                    .on_click(remove_hero_from_party)
                    .child(
                        div()
                            .col()
                            .flex_1()
                            .child(
                                text(&info.name).font_size(20.0).color(HEADER_TEXT),
                            )
                            .child(
                                text(trf("common.hero_level_class", &[("level", &info.level.to_string()), ("class", &info.class.to_string())]))
                                    .font_size(14.0)
                                    .color(LABEL_TEXT),
                            ),
                    )
                    .child(
                        text("✕")
                            .font_size(20.0)
                            .color(Color::srgb(0.8, 0.3, 0.3)),
                    ),
            );
        }
    }

    if selected_party.0.is_empty() {
        party_panel = party_panel.child(
            text(tr("party.click_to_add"))
                .font_size(16.0)
                .color(Color::srgba(0.6, 0.6, 0.6, 0.8)),
        );
    }

    let content = div()
        .row()
        .w_full()
        .flex_1()
        .min_h(px(0.0))
        .gap(px(16.0))
        .p(px(16.0))
        .child(available_panel)
        .child(party_panel);

    // Dispatch button — enabled if party is not empty
    let bottom = div()
        .row()
        .w_full()
        .justify_center()
        .p(px(16.0));

    let cap = buildings.mission_cap();
    let at_cap = !buildings.can_dispatch(active);

    let bottom = if at_cap || selected_party.0.is_empty() {
        let msg = if at_cap {
            trf("party.war_room_full", &[("active", &active.to_string()), ("cap", &cap.to_string())])
        } else {
            tr("party.select_one").to_string()
        };
        bottom.child(
            div()
                .w(px(380.0))
                .h(px(80.0))
                .items_center()
                .justify_center()
                .bg(Color::srgba(0.3, 0.3, 0.3, 0.5))
                .border_radius(BorderRadius::MAX)
                .child(
                    text(msg)
                        .font_size(28.0)
                        .color(Color::srgba(0.6, 0.6, 0.6, 0.8)),
                ),
        )
    } else {
        bottom.child(widgets::game_button(
            trf("party.dispatch", &[("count", &selected_party.0.len().to_string())]),
            dispatch_mission,
        ))
    };

    root = root.child(top_bar).child(content).child(bottom);
    root.spawn_as_child_of(&mut commands, root_entity);
}

fn add_hero_to_party(
    click: On<Pointer<Click>>,
    buttons: Query<&AddHeroButton>,
    mut party: ResMut<SelectedParty>,
) {
    if party.0.len() >= MAX_PARTY_SIZE {
        return;
    }
    if let Ok(button) = buttons.get(click.event_target()) {
        if !party.0.contains(&button.0) {
            party.0.push(button.0);
        }
    }
}

fn remove_hero_from_party(
    click: On<Pointer<Click>>,
    buttons: Query<&RemoveHeroButton>,
    mut party: ResMut<SelectedParty>,
) {
    if let Ok(button) = buttons.get(click.event_target()) {
        party.0.retain(|&e| e != button.0);
    }
}

fn dispatch_mission(
    _: On<Pointer<Click>>,
    mut commands: Commands,
    party: Res<SelectedParty>,
    selected_mission: Option<Res<SelectedMission>>,
    templates: Option<Res<MissionTemplateDatabase>>,
    enemy_db: Option<Res<EnemyDatabase>>,
    hero_q: Query<(
        &HeroInfo,
        &HeroStats,
        Option<&crate::equipment::HeroEquipment>,
        &crate::hero::Fatigue,
        Option<&crate::mission::entities::MoveRange>,
        Option<&crate::hero::Epithet>,
        Option<&crate::hero::history::HeroHistory>,
    ), With<Hero>>,
    equipment_db: Option<Res<crate::equipment::EquipmentDatabase>>,
    injured_q: Query<(), With<crate::hero::status::Injured>>,
    mut next_tab: ResMut<NextState<GameTab>>,
    board: Option<Res<crate::screens::missions::MissionBoard>>,
    class_db: Option<Res<crate::hero::data::ClassDatabase>>,
    buildings: Res<crate::buildings::GuildBuildings>,
    mission_q: Query<&MissionProgress, With<Mission>>,
) {
    let Some(selected_sm) = selected_mission.as_ref() else {
        warn!("No mission selected for dispatch");
        return;
    };
    let Some(mission_idx) = selected_sm.index else {
        warn!("No mission index selected for dispatch");
        return;
    };
    let Some(templates) = templates else { return };
    let Some(board) = board else { return };
    let Some(enemy_db) = enemy_db else { return };
    let Some(equipment_db) = equipment_db else { return };
    if party.0.is_empty() {
        warn!("Cannot dispatch with empty party");
        return;
    }

    let active = crate::mission::active_mission_count(&mission_q);
    if !buildings.can_dispatch(active) {
        let cap = buildings.mission_cap();
        warn!("Dispatch refused: War Room at capacity ({active}/{cap})");
        commands.trigger(crate::ui::toast::ToastEvent {
            title: tr("party.war_room_toast").into(),
            body: trf(
                "party.war_room_toast_body",
                &[("active", &active.to_string()), ("cap", &cap.to_string())],
            ),
            kind: crate::ui::toast::ToastKind::Failure,
            action: None,
        });
        return;
    }

    let (template, modifiers) = if selected_sm.is_rescue {
        let Some(offer) = board.rescue_offers.get(mission_idx) else {
            warn!("Invalid rescue offer index: {mission_idx}");
            return;
        };
        let Some(temp) = templates.0.get(offer.template_idx) else { return };
        (temp, &offer.modifiers)
    } else {
        let Some(offer) = board.offers.get(mission_idx) else {
            warn!("Invalid standard offer index: {mission_idx}");
            return;
        };
        let Some(temp) = templates.0.get(offer.template_idx) else { return };
        (temp, &offer.modifiers)
    };

    // Generate dungeon for this mission
    let mut rng = rand::rng();
    let rooms = rng.random_range(template.rooms_min..=template.rooms_max);
    let map = generate_dungeon(40, 30, rooms, &mut rng);

    // Create mission entity with dungeon and room status
    let mission_entity = commands
        .spawn((
            Name::new(format!("Mission: {}", template.name)),
            Mission,
            MissionInfo {
                template_id: template.id.clone(),
                name: template.name.clone(),
                difficulty: template.difficulty,
                modifiers: modifiers.clone(),
                biome: template.biome,
            },
            MissionProgress::InProgress,
            MissionParty(party.0.clone()),
            MissionDungeon(map.clone()),
            RoomStatus::new_for_dungeon(&map),
            crate::mission::mission_sim_bundle(),
        ))
        .id();

    // Spawn logical hero/enemy tokens as children of the mission
    let mission_party = MissionParty(party.0.clone());
    crate::mission::entities::spawn_tokens_for_mission(
        &mut commands,
        mission_entity,
        &map,
        &mission_party,
        &hero_q,
        &equipment_db,
        &templates,
        &enemy_db,
        &template.id,
        &injured_q,
        class_db.as_ref().map(|db| db.as_ref()),
        modifiers,
    );

    // Mark heroes as on-mission
    for &hero_entity in &party.0 {
        commands.entity(hero_entity).insert(OnMission(mission_entity));
    }

    // Track which mission we're viewing
    commands.insert_resource(ViewedMission(mission_entity));

    info!(
        "Dispatched mission '{}' with {} heroes",
        template.name,
        party.0.len()
    );

    next_tab.set(GameTab::MissionView);
}

fn go_back_to_missions(_: On<Pointer<Click>>, mut next_tab: ResMut<NextState<GameTab>>) {
    next_tab.set(GameTab::Missions);
}

fn cleanup_party_select(mut commands: Commands) {
    commands.remove_resource::<SelectedParty>();
}
