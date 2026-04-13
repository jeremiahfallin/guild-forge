//! The recruiting screen — view and hire applicants.

use bevy::prelude::*;
use bevy_declarative::element::div::{Div, div};
use bevy_declarative::element::text::text;
use bevy_declarative::style::styled::Styled;
use bevy_declarative::style::values::px;

use crate::{
    buildings::GuildBuildings,
    hero::Hero,
    recruiting::{ApplicantBoard, HireApplicant},
    screens::GameTab,
    theme::{palette::*, widgets},
};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(GameTab::Recruiting), spawn_recruiting_screen);
    app.add_systems(
        Update,
        rebuild_recruiting_screen
            .run_if(in_state(GameTab::Recruiting))
            .run_if(
                resource_changed::<ApplicantBoard>
                    .or(timer_tick),
            ),
    );
}

#[derive(Component)]
struct RecruitingUi;

/// Tick roughly every second to update timers.
fn timer_tick(mut timer: Local<f32>, time: Res<Time>) -> bool {
    *timer += time.delta_secs();
    if *timer >= 1.0 {
        *timer = 0.0;
        true
    } else {
        false
    }
}

fn spawn_recruiting_screen(
    mut commands: Commands,
    gameplay_root: Query<Entity, With<widgets::GameplayRoot>>,
    board: Res<ApplicantBoard>,
    buildings: Res<GuildBuildings>,
    heroes: Query<(), With<Hero>>,
) {
    let Ok(root_entity) = gameplay_root.single() else { return };
    let hero_count = heroes.iter().count() as u32;
    let roster_cap = buildings.roster_cap();
    let root = build_recruiting_ui(&board, hero_count, roster_cap);
    root.spawn_as_child_of(&mut commands, root_entity);
}

fn rebuild_recruiting_screen(
    mut commands: Commands,
    gameplay_root: Query<Entity, With<widgets::GameplayRoot>>,
    recruiting_ui: Query<Entity, With<RecruitingUi>>,
    board: Res<ApplicantBoard>,
    buildings: Res<GuildBuildings>,
    heroes: Query<(), With<Hero>>,
) {
    let Ok(root_entity) = gameplay_root.single() else { return };

    for entity in &recruiting_ui {
        commands.entity(entity).despawn();
    }

    let hero_count = heroes.iter().count() as u32;
    let roster_cap = buildings.roster_cap();
    let root = build_recruiting_ui(&board, hero_count, roster_cap);
    root.spawn_as_child_of(&mut commands, root_entity);
}

fn build_recruiting_ui(
    board: &ApplicantBoard,
    hero_count: u32,
    roster_cap: u32,
) -> Div {
    let mut root = widgets::content_area("Recruiting Screen")
        .insert((DespawnOnExit(GameTab::Recruiting), RecruitingUi));

    let top_bar = div()
        .row()
        .w_full()
        .items_center()
        .justify_between()
        .p(px(16.0))
        .child(widgets::header("Applicant Board"))
        .child(
            text(format!("Heroes: {} / {}", hero_count, roster_cap))
                .font_size(24.0)
                .color(LABEL_TEXT),
        );

    let mut content = div()
        .col()
        .w_full()
        .flex_1()
        .gap(px(12.0))
        .p(px(16.0))
        .overflow_y_hidden();

    if board.applicants.is_empty() {
        content = content.child(
            text("No applicants available. Check back later!")
                .font_size(20.0)
                .color(Color::srgba(0.6, 0.6, 0.6, 0.8)),
        );
    }

    for (idx, applicant) in board.applicants.iter().enumerate() {
        let traits_str = if applicant.traits.is_empty() {
            "None".to_string()
        } else {
            applicant
                .traits
                .iter()
                .map(|t| format!("{t}"))
                .collect::<Vec<_>>()
                .join(", ")
        };

        let stats = &applicant.stats;
        let stats_str = format!(
            "STR {}  DEX {}  CON {}  INT {}  WIS {}  CHA {}",
            stats.strength,
            stats.dexterity,
            stats.constitution,
            stats.intelligence,
            stats.wisdom,
            stats.charisma,
        );

        let time_str = format_time(applicant.time_remaining);

        let mut card = div()
            .col()
            .w_full()
            .p(px(12.0))
            .gap(px(6.0))
            .bg(Color::srgba(0.2, 0.2, 0.3, 0.6))
            .rounded(px(6.0));

        // Name and class
        card = card.child(
            text(format!("{} - {}", applicant.name, applicant.class))
                .font_size(22.0)
                .color(HEADER_TEXT),
        );

        // Traits
        card = card.child(
            text(format!("Traits: {}", traits_str))
                .font_size(16.0)
                .color(LABEL_TEXT),
        );

        // Stats
        card = card.child(
            text(stats_str)
                .font_size(14.0)
                .color(LABEL_TEXT),
        );

        // Cost and timer row + hire button
        card = card.child(
            div()
                .row()
                .w_full()
                .items_center()
                .justify_between()
                .child(
                    text(format!("Cost: {}g      Leaves in: {}", applicant.hire_cost, time_str))
                        .font_size(16.0)
                        .color(LABEL_TEXT),
                )
                .child(
                    div()
                        .p(px(6.0))
                        .items_center()
                        .justify_center()
                        .bg(BUTTON_BACKGROUND)
                        .rounded(px(4.0))
                        .insert((
                            Name::new("Hire Button"),
                            Button,
                            HireButton(idx),
                        ))
                        .interaction_palette(BUTTON_BACKGROUND, BUTTON_HOVERED_BACKGROUND, BUTTON_PRESSED_BACKGROUND)
                        .on_click(on_hire_click)
                        .child(
                            text("Hire")
                                .font_size(18.0)
                                .color(BUTTON_TEXT)
                                .insert(Pickable::IGNORE),
                        ),
                ),
        );

        content = content.child(card);
    }

    root = root.child(top_bar).child(content);
    root
}

/// Format seconds into "Xh Ym" for readability.
fn format_time(seconds: f32) -> String {
    let total_minutes = (seconds / 60.0).ceil() as u32;
    let hours = total_minutes / 60;
    let minutes = total_minutes % 60;
    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}

#[derive(Component)]
struct HireButton(usize);

fn on_hire_click(
    click: On<Pointer<Click>>,
    buttons: Query<&HireButton>,
    mut commands: Commands,
) {
    if let Ok(button) = buttons.get(click.event_target()) {
        commands.trigger(HireApplicant(button.0));
    }
}
