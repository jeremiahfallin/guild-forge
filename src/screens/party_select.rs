//! Party selection screen — pick heroes for a mission then dispatch.

use bevy::prelude::*;
use bevy_declarative::element::div::div;
use bevy_declarative::element::text::text;
use bevy_declarative::style::styled::Styled;
use bevy_declarative::style::values::{pct, px};
use rand::Rng;

use crate::{
    hero::{Hero, HeroInfo},
    mission::{
        Mission, MissionInfo, MissionParty, MissionProgress, OnMission,
        data::MissionTemplateDatabase,
        dungeon::generate_dungeon,
    },
    screens::{
        GameTab,
        missions::SelectedMission,
        mission_view::ActiveDungeon,
    },
    theme::{palette::*, widgets},
};

const MAX_PARTY_SIZE: usize = 4;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(GameTab::PartySelect), spawn_party_select);
    app.add_systems(
        Update,
        refresh_party_select.run_if(
            in_state(GameTab::PartySelect).and(resource_changed::<SelectedParty>),
        ),
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

fn spawn_party_select(
    mut commands: Commands,
    selected_mission: Option<Res<SelectedMission>>,
    templates: Option<Res<MissionTemplateDatabase>>,
    heroes: Query<(Entity, &HeroInfo), (With<Hero>, Without<OnMission>)>,
) {
    commands.init_resource::<SelectedParty>();

    let mission_name = selected_mission
        .as_ref()
        .and_then(|sm| sm.0)
        .and_then(|idx| templates.as_ref().map(|t| t.0.get(idx)))
        .flatten()
        .map(|t| t.name.as_str())
        .unwrap_or("Unknown Mission");

    let mut root = widgets::content_area("Party Select")
        .insert((DespawnOnExit(GameTab::PartySelect), PartySelectUi));

    // Top bar
    let top_bar = div()
        .row()
        .w_full()
        .justify_between()
        .items_center()
        .p(px(16.0))
        .child(widgets::header(format!("Select Party — {mission_name}")))
        .child(widgets::game_button("Cancel", go_back_to_missions));

    // Two-panel layout: available heroes (left) + selected party (right)
    let available_panel = build_available_panel(&heroes, &[]);

    let party_panel = div()
        .col()
        .w(pct(50.0))
        .gap(px(8.0))
        .p(px(16.0))
        .bg(Color::srgba(0.15, 0.2, 0.15, 0.6))
        .rounded(px(8.0))
        .child(
            text("Selected Party (0/4)")
                .font_size(24.0)
                .color(HEADER_TEXT),
        )
        .child(
            text("Click heroes on the left to add them")
                .font_size(16.0)
                .color(LABEL_TEXT),
        );

    let content = div()
        .row()
        .w_full()
        .flex_1()
        .gap(px(16.0))
        .p(px(16.0))
        .child(available_panel)
        .child(party_panel);

    // Bottom: dispatch button (disabled until party has at least 1 hero)
    let bottom = div()
        .row()
        .w_full()
        .justify_center()
        .p(px(16.0))
        .child(
            div()
                .w(px(380.0))
                .h(px(80.0))
                .items_center()
                .justify_center()
                .bg(Color::srgba(0.3, 0.3, 0.3, 0.5))
                .border_radius(BorderRadius::MAX)
                .insert(Name::new("Dispatch Disabled"))
                .child(
                    text("Select at least 1 hero")
                        .font_size(28.0)
                        .color(Color::srgba(0.6, 0.6, 0.6, 0.8)),
                ),
        );

    root = root.child(top_bar).child(content).child(bottom);
    root.spawn(&mut commands);
}

fn build_available_panel(
    heroes: &Query<(Entity, &HeroInfo), (With<Hero>, Without<OnMission>)>,
    selected_entities: &[Entity],
) -> bevy_declarative::element::div::Div {
    let mut panel = div()
        .col()
        .w(pct(50.0))
        .gap(px(8.0))
        .p(px(16.0))
        .bg(Color::srgba(0.15, 0.15, 0.25, 0.6))
        .rounded(px(8.0))
        .insert(Name::new("Available Heroes"));

    panel = panel.child(
        text("Available Heroes")
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
                        text(format!("Lv.{} {}", info.level, info.class))
                            .font_size(14.0)
                            .color(LABEL_TEXT),
                    ),
            ),
        );
    }

    if heroes.is_empty() {
        panel = panel.child(
            text("No heroes available")
                .font_size(16.0)
                .color(Color::srgba(0.6, 0.6, 0.6, 0.8)),
        );
    }

    panel
}

fn refresh_party_select(
    mut commands: Commands,
    ui_q: Query<Entity, With<PartySelectUi>>,
    selected_party: Res<SelectedParty>,
    selected_mission: Option<Res<SelectedMission>>,
    templates: Option<Res<MissionTemplateDatabase>>,
    heroes: Query<(Entity, &HeroInfo), (With<Hero>, Without<OnMission>)>,
    hero_info: Query<&HeroInfo, With<Hero>>,
) {
    // Despawn old UI
    for entity in &ui_q {
        commands.entity(entity).despawn();
    }

    let mission_name = selected_mission
        .as_ref()
        .and_then(|sm| sm.0)
        .and_then(|idx| templates.as_ref().map(|t| t.0.get(idx)))
        .flatten()
        .map(|t| t.name.as_str())
        .unwrap_or("Unknown Mission");

    let mut root = widgets::content_area("Party Select")
        .insert((DespawnOnExit(GameTab::PartySelect), PartySelectUi));

    // Top bar
    let top_bar = div()
        .row()
        .w_full()
        .justify_between()
        .items_center()
        .p(px(16.0))
        .child(widgets::header(format!("Select Party — {mission_name}")))
        .child(widgets::game_button("Cancel", go_back_to_missions));

    // Available heroes panel
    let available_panel = build_available_panel(&heroes, &selected_party.0);

    // Selected party panel
    let mut party_panel = div()
        .col()
        .w(pct(50.0))
        .gap(px(8.0))
        .p(px(16.0))
        .bg(Color::srgba(0.15, 0.2, 0.15, 0.6))
        .rounded(px(8.0))
        .insert(Name::new("Selected Party"));

    party_panel = party_panel.child(
        text(format!(
            "Selected Party ({}/{})",
            selected_party.0.len(),
            MAX_PARTY_SIZE
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
                                text(format!("Lv.{} {}", info.level, info.class))
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
            text("Click heroes on the left to add them")
                .font_size(16.0)
                .color(Color::srgba(0.6, 0.6, 0.6, 0.8)),
        );
    }

    let content = div()
        .row()
        .w_full()
        .flex_1()
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

    let bottom = if selected_party.0.is_empty() {
        bottom.child(
            div()
                .w(px(380.0))
                .h(px(80.0))
                .items_center()
                .justify_center()
                .bg(Color::srgba(0.3, 0.3, 0.3, 0.5))
                .border_radius(BorderRadius::MAX)
                .child(
                    text("Select at least 1 hero")
                        .font_size(28.0)
                        .color(Color::srgba(0.6, 0.6, 0.6, 0.8)),
                ),
        )
    } else {
        bottom.child(widgets::game_button(
            format!("Dispatch! ({})", selected_party.0.len()),
            dispatch_mission,
        ))
    };

    root = root.child(top_bar).child(content).child(bottom);
    root.spawn(&mut commands);
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
    mut next_tab: ResMut<NextState<GameTab>>,
) {
    let Some(mission_idx) = selected_mission.as_ref().and_then(|sm| sm.0) else {
        warn!("No mission selected for dispatch");
        return;
    };
    let Some(template) = templates.as_ref().and_then(|t| t.0.get(mission_idx)) else {
        warn!("Invalid mission template index: {mission_idx}");
        return;
    };
    if party.0.is_empty() {
        warn!("Cannot dispatch with empty party");
        return;
    }

    // Generate dungeon for this mission
    let mut rng = rand::rng();
    let rooms = rng.random_range(template.rooms_min..=template.rooms_max);
    let map = generate_dungeon(40, 30, rooms, &mut rng);

    // Create mission entity
    let mission_entity = commands
        .spawn((
            Name::new(format!("Mission: {}", template.name)),
            Mission,
            MissionInfo {
                template_id: template.id.clone(),
                name: template.name.clone(),
                difficulty: template.difficulty,
            },
            MissionProgress::InProgress,
            MissionParty(party.0.clone()),
        ))
        .id();

    // Mark heroes as on-mission
    for &hero_entity in &party.0 {
        commands.entity(hero_entity).insert(OnMission(mission_entity));
    }

    // Store dungeon for rendering
    commands.insert_resource(ActiveDungeon(map));

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
