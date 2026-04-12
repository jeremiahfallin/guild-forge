//! The guild management screen — view buildings and upgrade them.

use bevy::prelude::*;
use bevy_declarative::element::div::{Div, div};
use bevy_declarative::element::text::text;
use bevy_declarative::style::styled::Styled;
use bevy_declarative::style::values::px;

use crate::{
    buildings::{BuildingDatabase, BuildingType, GuildBuildings, UpgradeBuilding},
    economy::Gold,
    materials::{ConversionDatabase, ConvertMaterials, Materials},
    screens::GameTab,
    theme::{palette::*, widgets},
};

pub(super) fn plugin(app: &mut App) {
    app.add_systems(OnEnter(GameTab::Guild), spawn_guild_screen);
    app.add_systems(
        Update,
        rebuild_guild_screen
            .run_if(in_state(GameTab::Guild))
            .run_if(
                resource_changed::<GuildBuildings>
                    .or(resource_changed::<Gold>)
                    .or(resource_changed::<Materials>),
            ),
    );
}

#[derive(Component)]
struct GuildUi;

fn spawn_guild_screen(
    mut commands: Commands,
    gameplay_root: Query<Entity, With<widgets::GameplayRoot>>,
    buildings: Res<GuildBuildings>,
    building_db: Res<BuildingDatabase>,
    gold: Res<Gold>,
    materials: Res<Materials>,
    conversion_db: Res<ConversionDatabase>,
) {
    let Ok(root_entity) = gameplay_root.single() else { return };
    let root = build_guild_ui(&buildings, &building_db, &gold, &materials, &conversion_db);
    root.spawn_as_child_of(&mut commands, root_entity);
}

fn rebuild_guild_screen(
    mut commands: Commands,
    gameplay_root: Query<Entity, With<widgets::GameplayRoot>>,
    guild_ui: Query<Entity, With<GuildUi>>,
    buildings: Res<GuildBuildings>,
    building_db: Res<BuildingDatabase>,
    gold: Res<Gold>,
    materials: Res<Materials>,
    conversion_db: Res<ConversionDatabase>,
) {
    let Ok(root_entity) = gameplay_root.single() else { return };

    for entity in &guild_ui {
        commands.entity(entity).despawn();
    }

    let root = build_guild_ui(&buildings, &building_db, &gold, &materials, &conversion_db);
    root.spawn_as_child_of(&mut commands, root_entity);
}

fn build_guild_ui(
    buildings: &GuildBuildings,
    building_db: &BuildingDatabase,
    _gold: &Gold,
    materials: &Materials,
    conversion_db: &ConversionDatabase,
) -> Div {
    let mut root = widgets::content_area("Guild Screen")
        .insert((DespawnOnExit(GameTab::Guild), GuildUi));

    let top_bar = div()
        .row()
        .w_full()
        .items_center()
        .p(px(16.0))
        .child(widgets::header("Guild Hall"));

    let mut content = div()
        .col()
        .w_full()
        .flex_1()
        .gap(px(12.0))
        .p(px(16.0))
        .overflow_y_hidden();

    // Building cards
    for &building_type in BuildingType::ALL {
        let current_level = buildings.level(building_type);
        let def = building_db.get(building_type);
        let max_level = def.map(|d| d.max_level).unwrap_or(0);

        let mut card = div()
            .col()
            .w_full()
            .p(px(12.0))
            .gap(px(6.0))
            .bg(Color::srgba(0.2, 0.2, 0.3, 0.6))
            .rounded(px(6.0));

        // Header row: name + level
        card = card.child(
            div()
                .row()
                .w_full()
                .items_center()
                .justify_between()
                .child(
                    text(building_type.name())
                        .font_size(24.0)
                        .color(HEADER_TEXT),
                )
                .child(
                    text(format!("Lv {} / {}", current_level, max_level))
                        .font_size(18.0)
                        .color(LABEL_TEXT),
                ),
        );

        // Description
        card = card.child(
            text(building_type.description())
                .font_size(16.0)
                .color(LABEL_TEXT),
        );

        // Upgrade cost + button
        if current_level < max_level {
            if let Some(def) = def {
                let cost = &def.level_costs[current_level as usize];
                let mut cost_str = format!("Next: {}g", cost.gold);
                for &(mat, amt) in &cost.materials {
                    cost_str.push_str(&format!(" + {} {}", amt, mat.name()));
                }

                card = card.child(
                    text(cost_str)
                        .font_size(14.0)
                        .color(LABEL_TEXT),
                );

                card = card.child(
                    div()
                        .row()
                        .p(px(6.0))
                        .items_center()
                        .justify_center()
                        .bg(BUTTON_BACKGROUND)
                        .rounded(px(4.0))
                        .insert((
                            Name::new("Upgrade Button"),
                            Button,
                            UpgradeBuildingButton(building_type),
                            crate::theme::interaction::InteractionPalette {
                                none: BUTTON_BACKGROUND,
                                hovered: BUTTON_HOVERED_BACKGROUND,
                                pressed: BUTTON_PRESSED_BACKGROUND,
                            },
                        ))
                        .on_click(on_upgrade_click)
                        .child(
                            text("Upgrade")
                                .font_size(18.0)
                                .color(BUTTON_TEXT)
                                .insert(Pickable::IGNORE),
                        ),
                );
            }
        } else {
            card = card.child(
                text("Max Level")
                    .font_size(14.0)
                    .color(Color::srgba(0.5, 0.8, 0.5, 1.0)),
            );
        }

        content = content.child(card);
    }

    // Workshop conversions section
    let workshop_level = buildings.level(BuildingType::Workshop);
    if workshop_level >= 1 {
        content = content.child(
            div()
                .w_full()
                .h(px(2.0))
                .bg(Color::srgba(0.4, 0.4, 0.5, 0.6)),
        );

        content = content.child(
            text("Workshop Conversions")
                .font_size(24.0)
                .color(HEADER_TEXT),
        );

        for (idx, recipe) in conversion_db.0.iter().enumerate() {
            if recipe.workshop_level_required > workshop_level {
                continue;
            }

            let recipe_text = format!(
                "{} {} -> {} {}",
                recipe.input_count,
                recipe.input_type.name(),
                recipe.output_count,
                recipe.output_type.name(),
            );

            let available = materials.get(recipe.input_type);
            let can_convert = available >= recipe.input_count;

            let bg = if can_convert {
                Color::srgba(0.2, 0.25, 0.3, 0.6)
            } else {
                Color::srgba(0.2, 0.2, 0.2, 0.4)
            };

            content = content.child(
                div()
                    .row()
                    .w_full()
                    .p(px(8.0))
                    .gap(px(12.0))
                    .items_center()
                    .justify_between()
                    .bg(bg)
                    .rounded(px(4.0))
                    .child(
                        text(recipe_text)
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
                                Name::new("Convert Button"),
                                Button,
                                ConvertButton(idx),
                                crate::theme::interaction::InteractionPalette {
                                    none: BUTTON_BACKGROUND,
                                    hovered: BUTTON_HOVERED_BACKGROUND,
                                    pressed: BUTTON_PRESSED_BACKGROUND,
                                },
                            ))
                            .on_click(on_convert_click)
                            .child(
                                text("Convert")
                                    .font_size(16.0)
                                    .color(BUTTON_TEXT)
                                    .insert(Pickable::IGNORE),
                            ),
                    ),
            );
        }
    }

    root = root.child(top_bar).child(content);
    root
}

#[derive(Component)]
struct UpgradeBuildingButton(BuildingType);

fn on_upgrade_click(
    click: On<Pointer<Click>>,
    buttons: Query<&UpgradeBuildingButton>,
    mut commands: Commands,
) {
    if let Ok(button) = buttons.get(click.event_target()) {
        commands.trigger(UpgradeBuilding(button.0));
    }
}

#[derive(Component)]
struct ConvertButton(usize);

fn on_convert_click(
    click: On<Pointer<Click>>,
    buttons: Query<&ConvertButton>,
    mut commands: Commands,
) {
    if let Ok(button) = buttons.get(click.event_target()) {
        commands.trigger(ConvertMaterials {
            recipe_index: button.0,
            quantity: 999,
        });
    }
}
