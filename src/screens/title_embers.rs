//! Drifting forge embers on the title screen background.
//! Density and warmth are controlled by the `EmberSettings` resource.

use bevy::prelude::*;
use bevy::window::PrimaryWindow;

#[derive(Resource, Debug, Clone, Reflect)]
#[reflect(Resource)]
pub struct EmberSettings {
    pub density: f32,
    pub warmth: f32,
}

impl Default for EmberSettings {
    fn default() -> Self {
        Self {
            density: 1.0,
            warmth: 1.0,
        }
    }
}

#[derive(Component)]
pub struct EmberParticle {
    pub vx: f32,
    pub vy: f32,
    pub life: f32,
    pub max_life: f32,
    pub flick: f32,
    pub hot: bool,
    pub radius: f32,
}

#[derive(Component)]
pub struct ForgeGlow;

#[derive(Resource, Clone)]
pub struct EmberGlowTexture(pub Handle<Image>);

impl FromWorld for EmberGlowTexture {
    fn from_world(world: &mut World) -> Self {
        let mut images = world.resource_mut::<Assets<Image>>();
        let handle = create_glow_texture(&mut images);
        Self(handle)
    }
}

pub fn plugin(app: &mut App) {
    app.init_resource::<EmberSettings>();
    app.init_resource::<EmberGlowTexture>();
    app.register_type::<EmberSettings>();

    app.add_systems(OnEnter(crate::screens::Screen::Title), setup_title_embers);
    app.add_systems(
        Update,
        (update_embers, update_forge_glow).run_if(in_state(crate::screens::Screen::Title)),
    );
}

fn create_glow_texture(images: &mut Assets<Image>) -> Handle<Image> {
    let size = 32;
    let mut data = vec![0u8; size * size * 4];
    for y in 0..size {
        for x in 0..size {
            let dx = (x as f32 - size as f32 / 2.0) / (size as f32 / 2.0);
            let dy = (y as f32 - size as f32 / 2.0) / (size as f32 / 2.0);
            let dist = (dx * dx + dy * dy).sqrt();
            let alpha = (1.0 - dist).clamp(0.0, 1.0);
            // exponential falloff for soft radial gradient look
            let alpha = alpha.powf(2.5);
            let idx = (y * size + x) * 4;
            data[idx] = 255;
            data[idx + 1] = 255;
            data[idx + 2] = 255;
            data[idx + 3] = (alpha * 255.0) as u8;
        }
    }
    images.add(Image::new_fill(
        bevy::render::render_resource::Extent3d {
            width: size as u32,
            height: size as u32,
            depth_or_array_layers: 1,
        },
        bevy::render::render_resource::TextureDimension::D2,
        &data,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::default(),
    ))
}

fn hsl_to_color(h: f32, s: f32, l: f32, a: f32) -> Color {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - (((h / 60.0) % 2.0) - 1.0).abs());
    let m = l - c / 2.0;
    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    Color::srgba(r + m, g + m, b + m, a)
}

fn spawn_ember(
    commands: &mut Commands,
    glow_texture: Handle<Image>,
    w: f32,
    h: f32,
    initial: bool,
) {
    let mut rng = rand::rng();
    use rand::Rng;

    let x = (rng.random::<f32>() - 0.5) * w;
    let y = if initial {
        (rng.random::<f32>() - 0.5) * h
    } else {
        -h / 2.0 - rng.random::<f32>() * 40.0
    };
    let radius = 0.6 + rng.random::<f32>() * 1.8;
    let vy = 15.0 + rng.random::<f32>() * 35.0;
    let vx = (rng.random::<f32>() - 0.5) * 15.0;
    let max_life = 4.0 + rng.random::<f32>() * 6.0;
    let flick = rng.random::<f32>() * std::f32::consts::TAU;
    let hot = rng.random::<f32>() < 0.35;

    let size = radius * 4.0;

    commands.spawn((
        Name::new("Ember"),
        EmberParticle {
            vx,
            vy,
            life: 0.0,
            max_life,
            flick,
            hot,
            radius,
        },
        Sprite {
            image: glow_texture,
            custom_size: Some(Vec2::new(size, size)),
            ..default()
        },
        Transform::from_xyz(x, y, 1.0),
        DespawnOnExit(crate::screens::Screen::Title),
    ));
}

fn setup_title_embers(
    mut commands: Commands,
    window_q: Query<&Window, With<PrimaryWindow>>,
    settings: Res<EmberSettings>,
    glow_texture: Res<EmberGlowTexture>,
) {
    let Ok(window) = window_q.single() else {
        return;
    };
    let w = window.width();
    let h = window.height();

    // Spawn bottom forge glow
    commands.spawn((
        Name::new("Forge Glow"),
        ForgeGlow,
        Sprite {
            image: glow_texture.0.clone(),
            color: Color::srgba(0.85, 0.25, 0.05, 0.25),
            custom_size: Some(Vec2::new(w * 1.5, h * 0.45)),
            ..default()
        },
        Transform::from_xyz(0.0, -h / 2.0, 0.5),
        DespawnOnExit(crate::screens::Screen::Title),
    ));

    // Spawn initial particles
    let target_count = ((w * h) / 26000.0 * settings.density).round() as usize;
    for _ in 0..target_count {
        spawn_ember(&mut commands, glow_texture.0.clone(), w, h, true);
    }
}

fn update_embers(
    mut commands: Commands,
    time: Res<Time>,
    window_q: Query<&Window, With<PrimaryWindow>>,
    settings: Res<EmberSettings>,
    glow_texture: Res<EmberGlowTexture>,
    mut ember_q: Query<(Entity, &mut EmberParticle, &mut Transform, &mut Sprite)>,
) {
    let Ok(window) = window_q.single() else {
        return;
    };
    let w = window.width();
    let h = window.height();
    let dt = time.delta_secs();

    let mut current_count = 0;
    let mut rng = rand::rng();
    use rand::Rng;

    for (_, mut p, mut transform, mut sprite) in &mut ember_q {
        current_count += 1;

        p.life += dt;
        p.flick += 9.0 * dt;

        transform.translation.y += p.vy * dt;
        transform.translation.x += (p.vx + (p.life * 1.6).sin() * 7.2) * dt;

        // Recycle if off screen or too old
        if transform.translation.y > h / 2.0 + 10.0 || p.life > p.max_life {
            p.life = 0.0;
            p.max_life = 4.0 + rng.random::<f32>() * 6.0;
            p.radius = 0.6 + rng.random::<f32>() * 1.8;
            p.vy = 15.0 + rng.random::<f32>() * 35.0;
            p.vx = (rng.random::<f32>() - 0.5) * 15.0;
            p.flick = rng.random::<f32>() * std::f32::consts::TAU;
            p.hot = rng.random::<f32>() < 0.35;

            let size = p.radius * 4.0;
            sprite.custom_size = Some(Vec2::new(size, size));

            transform.translation.x = (rng.random::<f32>() - 0.5) * w;
            transform.translation.y = -h / 2.0 - rng.random::<f32>() * 40.0;
        }

        let life_t = p.life / p.max_life;
        let fade = (life_t.min(1.0) * std::f32::consts::PI).sin();
        let tw = 0.6 + 0.4 * p.flick.sin();
        let alpha = (fade * tw).max(0.0)
            * (if p.hot { 0.95 } else { 0.6 })
            * (0.5 + 0.5 * settings.warmth);

        let hue = if p.hot {
            32.0 + 8.0 * settings.warmth
        } else {
            44.0
        };

        let lightness = if p.hot { 0.66 } else { 0.72 };
        sprite.color = hsl_to_color(hue, 0.95, lightness, alpha);
    }

    let target_count = ((w * h) / 26000.0 * settings.density).round() as usize;

    if current_count < target_count {
        let spawn_num = target_count - current_count;
        for _ in 0..spawn_num {
            spawn_ember(&mut commands, glow_texture.0.clone(), w, h, false);
        }
    } else if current_count > target_count {
        let despawn_num = current_count - target_count;
        let entities_to_despawn: Vec<Entity> = ember_q
            .iter()
            .take(despawn_num)
            .map(|(e, _, _, _)| e)
            .collect();
        for entity in entities_to_despawn {
            commands.entity(entity).despawn();
        }
    }
}

fn update_forge_glow(
    window_q: Query<&Window, With<PrimaryWindow>>,
    mut glow_q: Query<(&mut Transform, &mut Sprite), With<ForgeGlow>>,
) {
    let Ok(window) = window_q.single() else {
        return;
    };
    let w = window.width();
    let h = window.height();
    if let Ok((mut transform, mut sprite)) = glow_q.single_mut() {
        transform.translation.y = -h / 2.0;
        sprite.custom_size = Some(Vec2::new(w * 1.5, h * 0.45));
    }
}
