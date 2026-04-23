//! Persistent sidebar — spawned once on gameplay enter, lives across all GameTab transitions.

use bevy::prelude::*;
use bevy_declarative::InteractionPalette;
use bevy_declarative::element::div::div;
use bevy_declarative::element::text::text;
use bevy_declarative::style::styled::Styled;
use bevy_declarative::style::values::px;

use crate::{
    economy::Gold,
    hero::{Favorite, Hero},
    mission::{Mission, MissionInfo, MissionParty, MissionProgress, ViewedMission},
    reputation::Reputation,
    screens::GameTab,
    theme::{
        palette::*,
        widgets::{GameplayRoot, SidebarBankText, SidebarGoldText, SidebarMissionList, SidebarNavButton, SidebarRepText, SidebarRoot, SpeedButton},
    },
    time_bank::{GameSpeed, OfflineTimeBank, SetGameSpeed, format_banked_time},
};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(
        OnEnter(crate::screens::Screen::Gameplay),
        spawn_gameplay_root,
    );
    app.add_systems(
        Update,
        (
            update_gold_display.run_if(resource_changed::<Gold>),
            update_rep_display.run_if(resource_changed::<Reputation>),
            update_bank_display.run_if(resource_changed::<OfflineTimeBank>),
            update_active_tab_highlight.run_if(state_changed::<GameTab>),
            update_active_speed_highlight.run_if(resource_changed::<GameSpeed>),
            update_mission_list,
        )
            .run_if(in_state(crate::screens::Screen::Gameplay)),
    );
}

/// The sidebar width in pixels.
const SIDEBAR_WIDTH: f32 = 220.0;

fn spawn_gameplay_root(
    mut commands: Commands,
    gold: Option<Res<Gold>>,
    rep: Option<Res<Reputation>>,
    bank: Option<Res<OfflineTimeBank>>,
    speed: Option<Res<GameSpeed>>,
) {
    let gold_amount = gold.map_or(0, |g| g.0);
    let rep_amount = rep.map_or(0, |r| r.0);
    let banked = bank.map_or(0.0, |b| b.banked_seconds);
    let current_speed = speed.map_or(1.0, |s| s.0);

    // Gameplay root: row containing sidebar + content area
    let mut root = div()
        .absolute()
        .w_full()
        .h_full()
        .row()
        .insert((
            Name::new("Gameplay Root"),
            GameplayRoot,
            Pickable::IGNORE,
            DespawnOnExit(crate::screens::Screen::Gameplay),
        ));

    // Build sidebar
    let sidebar = build_sidebar(gold_amount, rep_amount, banked, current_speed);
    root = root.child(sidebar);

    root.spawn(&mut commands);
}

fn build_sidebar(gold_amount: u32, rep_amount: u32, banked: f32, speed: f32) -> bevy_declarative::element::div::Div {
    let mut sidebar = div()
        .col()
        .w(px(SIDEBAR_WIDTH))
        .h_full()
        .bg(Color::srgba(0.08, 0.08, 0.12, 0.95))
        .insert((Name::new("Sidebar"), SidebarRoot));

    // -- Pinned section --
    let pinned = div()
        .col()
        .w_full()
        .gap(px(8.0))
        .p(px(12.0))
        // Title
        .child(
            text("Guild Forge")
                .font_size(24.0)
                .color(HEADER_TEXT),
        )
        // Gold
        .child(
            text(format!("Gold: {gold_amount}"))
                .font_size(18.0)
                .color(Color::srgb(0.9, 0.8, 0.2))
                .insert(SidebarGoldText),
        )
        // Reputation
        .child(
            text(format!("Rep: {rep_amount}"))
                .font_size(16.0)
                .color(Color::srgb(0.6, 0.8, 0.9))
                .insert(SidebarRepText),
        )
        // Speed control
        .child(
            div()
                .row()
                .w_full()
                .gap(px(4.0))
                .items_center()
                .child(speed_btn(1.0, speed))
                .child(speed_btn(2.0, speed))
                .child(speed_btn(3.0, speed))
                .child(
                    text(format!("Bank: {}", format_banked_time(banked)))
                        .font_size(14.0)
                        .color(Color::srgb(0.7, 0.8, 0.9))
                        .insert(SidebarBankText),
                ),
        )
        // Divider
        .child(
            div()
                .w_full()
                .h(px(1.0))
                .bg(Color::srgba(0.4, 0.4, 0.5, 0.5)),
        )
        // Nav buttons
        .child(nav_button("Roster", GameTab::Roster))
        .child(nav_button("Missions", GameTab::Missions))
        .child(nav_button("Guild", GameTab::Guild))
        .child(nav_button("Armory", GameTab::Armory))
        .child(nav_button("Recruiting", GameTab::Recruiting))
        // Divider
        .child(
            div()
                .w_full()
                .h(px(1.0))
                .bg(Color::srgba(0.4, 0.4, 0.5, 0.5)),
        )
        // Active Missions header
        .child(
            text("Active Missions")
                .font_size(16.0)
                .color(LABEL_TEXT),
        );

    // -- Scrollable mission list --
    let mission_list = div()
        .col()
        .w_full()
        .flex_1()
        .gap(px(4.0))
        .p(px(12.0))
        .overflow_y_scroll()
        .insert((Name::new("Mission List"), SidebarMissionList));

    sidebar = sidebar.child(pinned).child(mission_list);
    sidebar
}

fn nav_button(label: &str, tab: GameTab) -> bevy_declarative::element::div::Div {
    div()
        .w_full()
        .h(px(40.0))
        .items_center()
        .justify_center()
        .bg(BUTTON_BACKGROUND)
        .rounded(px(4.0))
        .insert((
            Name::new(format!("Nav: {label}")),
            Button,
            SidebarNavButton(tab),
        ))
        .interaction_palette(BUTTON_BACKGROUND, BUTTON_HOVERED_BACKGROUND, BUTTON_PRESSED_BACKGROUND)
        .on_click(nav_click)
        .child(
            text(label)
                .font_size(18.0)
                .color(BUTTON_TEXT)
                .insert(Pickable::IGNORE),
        )
}

fn nav_click(
    click: On<Pointer<Click>>,
    buttons: Query<&SidebarNavButton>,
    current_tab: Res<State<GameTab>>,
    mut next_tab: ResMut<NextState<GameTab>>,
) {
    if let Ok(nav) = buttons.get(click.event_target()) {
        if nav.0 != **current_tab {
            next_tab.set(nav.0);
        }
    }
}

fn speed_btn(multiplier: f32, current_speed: f32) -> bevy_declarative::element::div::Div {
    let label = format!("{}x", multiplier as u32);
    let is_active = (current_speed - multiplier).abs() < 0.01;
    let bg = if is_active {
        Color::srgb(0.3, 0.5, 0.7)
    } else {
        BUTTON_BACKGROUND
    };

    div()
        .p(px(4.0))
        .items_center()
        .justify_center()
        .bg(bg)
        .rounded(px(3.0))
        .insert((
            Button,
            SpeedButton(multiplier),
        ))
        .interaction_palette(bg, BUTTON_HOVERED_BACKGROUND, BUTTON_PRESSED_BACKGROUND)
        .on_click(on_speed_click)
        .child(
            text(label)
                .font_size(14.0)
                .color(BUTTON_TEXT)
                .insert(Pickable::IGNORE),
        )
}

fn on_speed_click(
    click: On<Pointer<Click>>,
    buttons: Query<&SpeedButton>,
    mut commands: Commands,
) {
    if let Ok(btn) = buttons.get(click.event_target()) {
        commands.trigger(SetGameSpeed(btn.0));
    }
}

// -- Reactive update systems --

fn update_gold_display(
    gold: Res<Gold>,
    mut texts: Query<&mut Text, With<SidebarGoldText>>,
) {
    for mut t in &mut texts {
        **t = format!("Gold: {}", gold.0);
    }
}

fn update_rep_display(
    rep: Res<Reputation>,
    mut texts: Query<&mut Text, With<SidebarRepText>>,
) {
    for mut t in &mut texts {
        **t = format!("Rep: {} (Tier {})", rep.0, rep.tier());
    }
}

fn update_bank_display(
    bank: Res<OfflineTimeBank>,
    mut texts: Query<&mut Text, With<SidebarBankText>>,
) {
    for mut t in &mut texts {
        **t = format!("Bank: {}", format_banked_time(bank.banked_seconds));
    }
}

fn update_active_speed_highlight(
    speed: Res<GameSpeed>,
    mut buttons: Query<(&SpeedButton, &mut BackgroundColor, &mut InteractionPalette)>,
) {
    const ACTIVE: Color = Color::srgb(0.3, 0.5, 0.7);
    for (btn, mut bg, mut palette) in &mut buttons {
        let resting = if (speed.0 - btn.0).abs() < 0.01 {
            ACTIVE
        } else {
            BUTTON_BACKGROUND
        };
        *bg = BackgroundColor(resting);
        palette.none = resting;
    }
}

fn update_active_tab_highlight(
    tab: Res<State<GameTab>>,
    mut buttons: Query<(&SidebarNavButton, &mut BackgroundColor)>,
) {
    for (nav, mut bg) in &mut buttons {
        if nav.0 == **tab {
            *bg = BackgroundColor(BUTTON_HOVERED_BACKGROUND);
        } else {
            *bg = BackgroundColor(BUTTON_BACKGROUND);
        }
    }
}

fn update_mission_list(
    mut commands: Commands,
    list_q: Query<Entity, With<SidebarMissionList>>,
    missions: Query<(Entity, &MissionInfo, &MissionProgress, &MissionParty), With<Mission>>,
    favorite_heroes: Query<(), (With<Hero>, With<Favorite>)>,
    children_q: Query<&Children>,
    mut last_snapshot: Local<Vec<(Entity, MissionProgress, bool)>>,
) {
    let Ok(list_entity) = list_q.single() else {
        return;
    };

    // Build a snapshot of current mission state to detect changes
    let mut snapshot: Vec<(Entity, MissionProgress, bool)> = missions
        .iter()
        .map(|(e, _, p, party)| {
            let has_favorite = party.0.iter().any(|h| favorite_heroes.get(*h).is_ok());
            (e, *p, has_favorite)
        })
        .collect();
    snapshot.sort_by_key(|(e, _, _)| *e);

    if *last_snapshot == snapshot {
        return; // Nothing changed — keep existing UI with its click observers
    }
    *last_snapshot = snapshot;

    // Despawn existing children
    if let Ok(children) = children_q.get(list_entity) {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }

    // Rebuild mission entries
    for (mission_entity, info, progress, party) in &missions {
        let has_favorite = party.0.iter().any(|h| favorite_heroes.get(*h).is_ok());

        let status_text = match progress {
            MissionProgress::InProgress => "In Progress",
            MissionProgress::Complete => "Complete",
            MissionProgress::Failed => "Failed",
        };

        let base_bg = match progress {
            MissionProgress::InProgress => Color::srgba(0.2, 0.25, 0.35, 0.8),
            MissionProgress::Complete => Color::srgba(0.15, 0.35, 0.15, 0.8),
            MissionProgress::Failed => Color::srgba(0.35, 0.15, 0.15, 0.8),
        };
        let bg_color = if has_favorite {
            let c = base_bg.to_srgba();
            Color::srgba(c.red + 0.15, c.green + 0.10, c.blue, 0.9)
        } else {
            base_bg
        };

        let name_text = if has_favorite {
            format!("* {}", info.name)
        } else {
            info.name.clone()
        };

        let entry = div()
            .col()
            .w_full()
            .p(px(8.0))
            .gap(px(2.0))
            .bg(bg_color)
            .rounded(px(4.0))
            .insert(WatchMissionButton(mission_entity))
            .on_click(watch_mission)
            .child(
                text(name_text)
                    .font_size(14.0)
                    .color(HEADER_TEXT),
            )
            .child(
                text(status_text)
                    .font_size(12.0)
                    .color(LABEL_TEXT),
            );

        entry.spawn_as_child_of(&mut commands, list_entity);
    }
}

/// Component on mission entries in the sidebar.
#[derive(Component)]
struct WatchMissionButton(Entity);

fn watch_mission(
    click: On<Pointer<Click>>,
    mut commands: Commands,
    buttons: Query<&WatchMissionButton>,
    current_tab: Res<State<GameTab>>,
    mut next_tab: ResMut<NextState<GameTab>>,
) {
    if let Ok(button) = buttons.get(click.event_target()) {
        // Set which mission we're viewing — change detection in
        // rebuild_view_on_viewed_change handles in-place view swap.
        commands.insert_resource(ViewedMission(button.0));

        if **current_tab != GameTab::MissionView {
            next_tab.set(GameTab::MissionView);
        }
    }
}
