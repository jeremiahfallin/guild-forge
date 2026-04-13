//! The armory screen — craft and upgrade hero equipment.

use bevy::prelude::*;
use bevy_declarative::element::div::{Div, div};
use bevy_declarative::element::text::text;
use bevy_declarative::style::styled::Styled;
use bevy_declarative::style::values::{pct, px};

use crate::{
    buildings::{BuildingType, GuildBuildings},
    economy::Gold,
    equipment::{CraftGear, EquipmentDatabase, GearSlot, HeroEquipment},
    hero::{Hero, HeroInfo},
    materials::Materials,
    mission::OnMission,
    screens::GameTab,
    theme::{palette::*, widgets},
};

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<SelectedArmoryHero>();
    app.add_systems(OnEnter(GameTab::Armory), spawn_armory_screen);
    app.add_systems(
        Update,
        rebuild_armory_screen
            .run_if(in_state(GameTab::Armory))
            .run_if(
                resource_changed::<SelectedArmoryHero>
                    .or(resource_changed::<GuildBuildings>)
                    .or(resource_changed::<Gold>)
                    .or(resource_changed::<Materials>),
            ),
    );
    app.add_systems(OnExit(GameTab::Armory), clear_selection);
}

#[derive(Resource, Default, Debug)]
struct SelectedArmoryHero(Option<Entity>);

#[derive(Component)]
struct ArmoryUi;

fn spawn_armory_screen(
    mut commands: Commands,
    gameplay_root: Query<Entity, With<widgets::GameplayRoot>>,
    heroes: Query<(Entity, &HeroInfo, Option<&OnMission>), With<Hero>>,
    selected: Res<SelectedArmoryHero>,
    equipment_db: Res<EquipmentDatabase>,
    hero_equip_query: Query<(&HeroInfo, &HeroEquipment), With<Hero>>,
    buildings: Res<GuildBuildings>,
) {
    let Ok(root_entity) = gameplay_root.single() else { return };
    let root = build_armory_ui(&heroes, &selected, &equipment_db, &hero_equip_query, &buildings);
    root.spawn_as_child_of(&mut commands, root_entity);
}

fn rebuild_armory_screen(
    mut commands: Commands,
    gameplay_root: Query<Entity, With<widgets::GameplayRoot>>,
    armory_ui: Query<Entity, With<ArmoryUi>>,
    heroes: Query<(Entity, &HeroInfo, Option<&OnMission>), With<Hero>>,
    selected: Res<SelectedArmoryHero>,
    equipment_db: Res<EquipmentDatabase>,
    hero_equip_query: Query<(&HeroInfo, &HeroEquipment), With<Hero>>,
    buildings: Res<GuildBuildings>,
) {
    let Ok(root_entity) = gameplay_root.single() else { return };

    for entity in &armory_ui {
        commands.entity(entity).despawn();
    }

    let root = build_armory_ui(&heroes, &selected, &equipment_db, &hero_equip_query, &buildings);
    root.spawn_as_child_of(&mut commands, root_entity);
}

fn build_armory_ui(
    heroes: &Query<(Entity, &HeroInfo, Option<&OnMission>), With<Hero>>,
    selected: &SelectedArmoryHero,
    equipment_db: &EquipmentDatabase,
    hero_equip_query: &Query<(&HeroInfo, &HeroEquipment), With<Hero>>,
    buildings: &GuildBuildings,
) -> Div {
    let mut root = widgets::content_area("Armory Screen")
        .insert((DespawnOnExit(GameTab::Armory), ArmoryUi));

    let top_bar = div()
        .row()
        .w_full()
        .items_center()
        .p(px(16.0))
        .child(widgets::header("Armory"));

    let hero_list = build_hero_list(heroes, selected);
    let detail = build_gear_panel(selected, equipment_db, hero_equip_query, buildings);

    let content = div()
        .row()
        .w_full()
        .flex_1()
        .gap(px(16.0))
        .p(px(16.0))
        .child(hero_list)
        .child(detail);

    root = root.child(top_bar).child(content);
    root
}

fn build_hero_list(
    heroes: &Query<(Entity, &HeroInfo, Option<&OnMission>), With<Hero>>,
    selected: &SelectedArmoryHero,
) -> Div {
    let mut list = div()
        .col()
        .w(pct(30.0))
        .gap(px(8.0))
        .overflow_y_hidden()
        .insert(Name::new("Armory Hero List"));

    list = list.child(
        text("Heroes")
            .font_size(28.0)
            .color(HEADER_TEXT),
    );

    for (entity, info, on_mission) in heroes.iter() {
        if on_mission.is_some() {
            continue; // Filter out deployed heroes
        }

        let is_selected = selected.0 == Some(entity);
        let bg_color = if is_selected {
            Color::srgba(0.275, 0.400, 0.750, 0.8)
        } else {
            Color::srgba(0.2, 0.2, 0.3, 0.6)
        };

        list = list.child(
            div()
                .row()
                .w_full()
                .p(px(12.0))
                .gap(px(12.0))
                .items_center()
                .bg(bg_color)
                .rounded(px(6.0))
                .insert(SelectArmoryHeroButton(entity))
                .on_click(select_armory_hero)
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
                            text(format!("Lv.{} {}", info.level, info.class))
                                .font_size(16.0)
                                .color(LABEL_TEXT),
                        ),
                ),
        );
    }

    list
}

fn build_gear_panel(
    selected: &SelectedArmoryHero,
    equipment_db: &EquipmentDatabase,
    hero_equip_query: &Query<(&HeroInfo, &HeroEquipment), With<Hero>>,
    buildings: &GuildBuildings,
) -> Div {
    let panel = div()
        .col()
        .flex_1()
        .p(px(20.0))
        .gap(px(16.0))
        .bg(Color::srgba(0.15, 0.15, 0.25, 0.6))
        .rounded(px(8.0))
        .insert(Name::new("Gear Panel"));

    let Some(entity) = selected.0 else {
        return panel.child(
            text("Select a hero to view equipment")
                .font_size(24.0)
                .color(Color::srgba(0.6, 0.6, 0.6, 0.8)),
        );
    };

    let Ok((info, equipment)) = hero_equip_query.get(entity) else {
        return panel.child(
            text("Hero not found")
                .font_size(24.0)
                .color(Color::srgba(0.8, 0.3, 0.3, 1.0)),
        );
    };

    let armory_level = buildings.level(BuildingType::Armory);

    let mut result = panel.child(
        text(format!("{} - Equipment", info.name))
            .font_size(28.0)
            .color(HEADER_TEXT),
    );

    for &slot in GearSlot::ALL {
        let current_tier = equipment.tier(slot);
        let path = equipment_db.get_path(info.class, slot);

        let mut card = div()
            .col()
            .w_full()
            .p(px(12.0))
            .gap(px(6.0))
            .bg(Color::srgba(0.2, 0.2, 0.35, 0.5))
            .rounded(px(6.0));

        // Current gear name
        let current_name = if current_tier == 0 {
            "None".to_string()
        } else if let Some(p) = path {
            p.tiers
                .get((current_tier - 1) as usize)
                .map(|t| t.name.clone())
                .unwrap_or_else(|| format!("Tier {}", current_tier))
        } else {
            "None".to_string()
        };

        card = card.child(
            div()
                .row()
                .items_center()
                .justify_between()
                .child(
                    text(format!("{slot}"))
                        .font_size(20.0)
                        .color(HEADER_TEXT),
                )
                .child(
                    text(&current_name)
                        .font_size(18.0)
                        .color(LABEL_TEXT),
                ),
        );

        // Next tier info + craft button
        if let Some(p) = path {
            let next_tier = current_tier + 1;
            if let Some(tier_def) = p.tiers.get((next_tier - 1) as usize) {
                let mut cost_str = format!("Next: {} ({}g", tier_def.name, tier_def.gold_cost);
                for &(mat, amt) in &tier_def.material_cost {
                    cost_str.push_str(&format!(", {} {}", amt, mat.name()));
                }
                cost_str.push(')');

                if tier_def.armory_level_required > armory_level {
                    cost_str.push_str(&format!(
                        " [Armory Lv {} req]",
                        tier_def.armory_level_required
                    ));
                }

                card = card.child(
                    text(&cost_str)
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
                            Name::new("Craft Button"),
                            Button,
                            CraftButton { hero: entity, slot },
                        ))
                        .interaction_palette(BUTTON_BACKGROUND, BUTTON_HOVERED_BACKGROUND, BUTTON_PRESSED_BACKGROUND)
                        .on_click(on_craft_click)
                        .child(
                            text("Craft")
                                .font_size(16.0)
                                .color(BUTTON_TEXT)
                                .insert(Pickable::IGNORE),
                        ),
                );
            } else {
                card = card.child(
                    text("Max Tier")
                        .font_size(14.0)
                        .color(Color::srgba(0.5, 0.8, 0.5, 1.0)),
                );
            }
        }

        result = result.child(card);
    }

    result
}

#[derive(Component)]
struct SelectArmoryHeroButton(Entity);

fn select_armory_hero(
    click: On<Pointer<Click>>,
    buttons: Query<&SelectArmoryHeroButton>,
    mut selected: ResMut<SelectedArmoryHero>,
) {
    if let Ok(button) = buttons.get(click.event_target()) {
        selected.0 = Some(button.0);
    }
}

#[derive(Component)]
struct CraftButton {
    hero: Entity,
    slot: GearSlot,
}

fn on_craft_click(
    click: On<Pointer<Click>>,
    buttons: Query<&CraftButton>,
    mut commands: Commands,
) {
    if let Ok(button) = buttons.get(click.event_target()) {
        commands.trigger(CraftGear {
            hero: button.hero,
            slot: button.slot,
        });
    }
}

fn clear_selection(mut selected: ResMut<SelectedArmoryHero>) {
    selected.0 = None;
}
