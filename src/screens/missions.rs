//! The mission board screen — select and dispatch missions.

use bevy::prelude::*;
use bevy_declarative::element::div::div;
use bevy_declarative::element::text::text;
use bevy_declarative::style::styled::Styled;
use bevy_declarative::style::values::px;

use crate::{
    mission::data::MissionTemplateDatabase,
    screens::GameTab,
    theme::{palette::*, widgets},
};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(GameTab::Missions), spawn_mission_board);
}

/// Tracks which mission template the player selected.
#[derive(Resource, Default, Debug)]
pub struct SelectedMission(pub Option<usize>);

/// Component on mission list buttons, storing which template index they represent.
#[derive(Component)]
struct SelectMissionButton(usize);

fn spawn_mission_board(
    mut commands: Commands,
    gameplay_root: Query<Entity, With<widgets::GameplayRoot>>,
    templates: Option<Res<MissionTemplateDatabase>>,
    reputation: Res<crate::reputation::Reputation>,
) {
    let Ok(root_entity) = gameplay_root.single() else { return };
    commands.init_resource::<SelectedMission>();

    let mut root = widgets::content_area("Mission Board")
        .insert(DespawnOnExit(GameTab::Missions));

    // Top bar
    let top_bar = div()
        .row()
        .w_full()
        .items_center()
        .p(px(16.0))
        .child(widgets::header("Mission Board"));

    root = root.child(top_bar);

    // Mission list
    if let Some(templates) = templates {
        let mut list = div()
            .col()
            .w_full()
            .gap(px(12.0))
            .p(px(16.0))
            .items_center()
            .insert(Name::new("Mission List"));

        for (idx, template) in templates.0.iter().enumerate() {
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

            list = list.child(
                div()
                    .row()
                    .w(px(700.0))
                    .p(px(16.0))
                    .gap(px(16.0))
                    .items_center()
                    .bg(Color::srgba(0.2, 0.2, 0.3, 0.6))
                    .rounded(px(8.0))
                    .insert(SelectMissionButton(idx))
                    .on_click(select_mission)
                    .child(
                        div()
                            .col()
                            .flex_1()
                            .gap(px(4.0))
                            .child(
                                text(&template.name)
                                    .font_size(26.0)
                                    .color(HEADER_TEXT),
                            )
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
        selected.0 = Some(button.0);
        next_tab.set(GameTab::PartySelect);
    }
}

pub fn clear_selection(mut commands: Commands) {
    commands.remove_resource::<SelectedMission>();
}
