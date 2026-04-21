use bevy::prelude::*;
use bevy_declarative::InteractionPalette;

use crate::{asset_tracking::LoadResource, audio::sound_effect};

pub(super) fn plugin(app: &mut App) {
    app.load_resource::<InteractionAssets>();
    app.add_observer(play_sound_effect_on_click);
    app.add_observer(play_sound_effect_on_over);
}

#[derive(Resource, Asset, Clone, Reflect)]
#[reflect(Resource)]
struct InteractionAssets {
    #[dependency]
    hover: Handle<AudioSource>,
    #[dependency]
    click: Handle<AudioSource>,
}

impl FromWorld for InteractionAssets {
    fn from_world(world: &mut World) -> Self {
        let assets = world.resource::<AssetServer>();
        Self {
            hover: assets.load("audio/sound_effects/button_hover.ogg"),
            click: assets.load("audio/sound_effects/button_click.ogg"),
        }
    }
}

fn play_sound_effect_on_click(
    on: On<Pointer<Click>>,
    interaction_assets: If<Res<InteractionAssets>>,
    interaction_entities: Query<Entity, With<InteractionPalette>>,
    mut commands: Commands,
) {
    if interaction_entities.contains(on.event_target()) {
        commands.spawn(sound_effect(interaction_assets.click.clone()));
    }
}

fn play_sound_effect_on_over(
    on: On<Pointer<Over>>,
    interaction_assets: If<Res<InteractionAssets>>,
    interaction_entities: Query<Entity, With<InteractionPalette>>,
    mut commands: Commands,
) {
    if interaction_entities.contains(on.event_target()) {
        commands.spawn(sound_effect(interaction_assets.hover.clone()));
    }
}
