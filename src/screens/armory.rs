//! The armory screen — craft and upgrade hero equipment.

use bevy::prelude::*;
use bevy_declarative::element::div::{Div, div};
use bevy_declarative::element::text::text;
use bevy_declarative::style::styled::Styled;
use bevy_declarative::style::values::px;

use crate::{
    localization::{tr, trf},
    buildings::{BuildingType, GuildBuildings},
    economy::Gold,
    equipment::{CraftGear, EquipmentDatabase, GearSlot, HeroEquipment},
    hero::{Hero, HeroInfo, Epithet, format_hero_name, portrait::HeroPortraitImage},
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
    heroes: Query<(Entity, &HeroInfo, Option<&OnMission>, Option<&Epithet>, Option<&HeroPortraitImage>), With<Hero>>,
    selected: Res<SelectedArmoryHero>,
    equipment_db: Res<EquipmentDatabase>,
    hero_equip_query: Query<(&HeroInfo, &HeroEquipment, Option<&Epithet>, Option<&HeroPortraitImage>), With<Hero>>,
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
    heroes: Query<(Entity, &HeroInfo, Option<&OnMission>, Option<&Epithet>, Option<&HeroPortraitImage>), With<Hero>>,
    selected: Res<SelectedArmoryHero>,
    equipment_db: Res<EquipmentDatabase>,
    hero_equip_query: Query<(&HeroInfo, &HeroEquipment, Option<&Epithet>, Option<&HeroPortraitImage>), With<Hero>>,
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
    heroes: &Query<(Entity, &HeroInfo, Option<&OnMission>, Option<&Epithet>, Option<&HeroPortraitImage>), With<Hero>>,
    selected: &SelectedArmoryHero,
    equipment_db: &EquipmentDatabase,
    hero_equip_query: &Query<(&HeroInfo, &HeroEquipment, Option<&Epithet>, Option<&HeroPortraitImage>), With<Hero>>,
    buildings: &GuildBuildings,
) -> Div {
    let mut root = widgets::content_area("Armory Screen")
        .insert((DespawnOnExit(GameTab::Armory), ArmoryUi));

    let top_bar = div()
        .row()
        .w_full()
        .items_center()
        .p(px(16.0))
        .child(widgets::header(tr("armory.header")));

    let hero_list = build_hero_list(heroes, selected);
    let detail = build_gear_panel(selected, equipment_db, hero_equip_query, buildings);

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
    root
}

fn build_hero_list(
    heroes: &Query<(Entity, &HeroInfo, Option<&OnMission>, Option<&Epithet>, Option<&HeroPortraitImage>), With<Hero>>,
    selected: &SelectedArmoryHero,
) -> Div {
    let mut list = div()
        .col()
        .w(px(360.0))
        .h_full()
        .min_h(px(0.0))
        .gap(px(8.0))
        .overflow_y_scroll()
        .insert((Name::new("Armory Hero List"), ScrollPosition::default()));

    list = list.child(
        text(tr("armory.heroes"))
            .font_size(28.0)
            .color(HEADER_TEXT),
    );

    for (entity, info, on_mission, epithet, portrait_img) in heroes.iter() {
        if on_mission.is_some() {
            continue; // Filter out deployed heroes
        }

        let is_selected = selected.0 == Some(entity);
        let border_color = if is_selected {
            BORDER_GOLD
        } else {
            BORDER_IRON
        };

        let mut card = div()
            .row()
            .w_full()
            .p(px(12.0))
            .gap(px(12.0))
            .items_center()
            .bg(CARD_BACKGROUND)
            .rounded(px(8.0))
            .insert((SelectArmoryHeroButton(entity), BorderColor::all(border_color)))
            .on_click(select_armory_hero);
        card.style_mut().border = UiRect::all(Val::Px(1.5));

        if let Some(portrait_image) = portrait_img {
            card = card.child(
                div()
                    .size(px(40.0))
                    .bg(Color::srgb(0.08, 0.08, 0.1))
                    .rounded(px(4.0))
                    .insert(ImageNode {
                        image: portrait_image.0.clone(),
                        ..default()
                    })
            );
        }

        card = card.child(
            div()
                .col()
                .flex_1()
                .child(
                    text(format_hero_name(&info.name, epithet))
                        .font_size(22.0)
                        .color(HEADER_TEXT),
                )
                .child(
                    text(trf("common.hero_level_class", &[("level", &info.level.to_string()), ("class", &info.class.to_string())]))
                        .font_size(16.0)
                        .color(LABEL_TEXT),
                ),
        );

        list = list.child(card);
    }

    list
}

fn build_gear_panel(
    selected: &SelectedArmoryHero,
    equipment_db: &EquipmentDatabase,
    hero_equip_query: &Query<(&HeroInfo, &HeroEquipment, Option<&Epithet>, Option<&HeroPortraitImage>), With<Hero>>,
    buildings: &GuildBuildings,
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
        .insert((Name::new("Gear Panel"), ScrollPosition::default(), BorderColor::all(BORDER_BRONZE)));
    panel.style_mut().border = UiRect::all(Val::Px(1.5));

    let Some(entity) = selected.0 else {
        return panel.child(
            text(tr("armory.select_prompt"))
                .font_size(24.0)
                .color(Color::srgba(0.6, 0.6, 0.6, 0.8)),
        );
    };

    let Ok((info, equipment, epithet, portrait_img)) = hero_equip_query.get(entity) else {
        return panel.child(
            text(tr("armory.hero_not_found"))
                .font_size(24.0)
                .color(Color::srgba(0.8, 0.3, 0.3, 1.0)),
        );
    };

    let armory_level = buildings.level(BuildingType::Armory);

    let mut header_row = div().row().items_center().gap(px(12.0));
    if let Some(portrait_image) = portrait_img {
        header_row = header_row.child(
            div()
                .size(px(40.0))
                .bg(Color::srgb(0.08, 0.08, 0.1))
                .rounded(px(4.0))
                .insert(ImageNode {
                    image: portrait_image.0.clone(),
                    ..default()
                })
        );
    }
    header_row = header_row.child(
        text(trf("armory.equipment_header", &[("name", &format_hero_name(&info.name, epithet))]))
            .font_size(28.0)
            .color(HEADER_TEXT),
    );

    let mut result = panel.child(header_row);

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
            tr("armory.no_gear").to_string()
        } else if let Some(p) = path {
            p.tiers
                .get((current_tier - 1) as usize)
                .map(|t| t.name.clone())
                .unwrap_or_else(|| trf("armory.tier_n", &[("tier", &current_tier.to_string())]))
        } else {
            tr("armory.no_gear").to_string()
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
                let mut cost_str = trf("armory.next_tier", &[("name", &tier_def.name), ("gold", &tier_def.gold_cost.to_string())]);
                for &(mat, amt) in &tier_def.material_cost {
                    cost_str.push_str(&trf("armory.cost_material", &[("amount", &amt.to_string()), ("name", mat.name())]));
                }
                cost_str.push(')');

                if tier_def.armory_level_required > armory_level {
                    cost_str.push_str(&trf(
                        "armory.level_req",
                        &[("level", &tier_def.armory_level_required.to_string())],
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
                            text(tr("armory.craft"))
                                .font_size(16.0)
                                .color(BUTTON_TEXT)
                                .insert(Pickable::IGNORE),
                        ),
                );
            } else {
                card = card.child(
                    text(tr("armory.max_tier"))
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
