//! Default UI font. Bevy 0.18's embedded FiraMono subset only covers basic
//! ASCII, so symbols like ★ ☆ · render as tofu. We load DejaVu Sans at startup
//! and patch any `TextFont` that still points at the default handle so the
//! whole UI picks up broader Unicode coverage automatically.

use bevy::prelude::*;
use bevy::text::TextFont;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(Startup, load_default_font);
    // Run in PostUpdate so text spawned during Update is patched before
    // layout/glyph computation later in PostUpdate — otherwise the default
    // FiraMono handle renders for one frame and the UI visibly flickers
    // whenever a screen rebuilds its text (e.g. Recruiting's 1s refresh).
    app.add_systems(PostUpdate, apply_default_font);
}

#[derive(Resource)]
pub struct DefaultUiFont(pub Handle<Font>);

fn load_default_font(mut commands: Commands, asset_server: Res<AssetServer>) {
    let handle: Handle<Font> = asset_server.load("fonts/DejaVuSans.ttf");
    commands.insert_resource(DefaultUiFont(handle));
}

/// Patch every `TextFont` whose `font` is still the default handle with our
/// loaded UI font. Runs every frame; cheap comparison, only writes on mismatch.
fn apply_default_font(
    font: Option<Res<DefaultUiFont>>,
    mut q: Query<&mut TextFont>,
) {
    let Some(font) = font else { return };
    let default = Handle::<Font>::default();
    for mut tf in &mut q {
        if tf.font == default {
            tf.font = font.0.clone();
        }
    }
}
