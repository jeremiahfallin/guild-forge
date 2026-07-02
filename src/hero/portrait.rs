use bevy::prelude::*;
use bevy::asset::RenderAssetUsages;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use serde::{Deserialize, Serialize};
use rand::Rng;

#[derive(Component, Clone, Debug, Serialize, Deserialize, Reflect, PartialEq)]
#[reflect(Component)]
pub struct HeroPortrait {
    pub base_idx: u32,
    pub hair_idx: u32,
    pub hair_color: Color,
    pub gear_idx: u32,
}

impl HeroPortrait {
    pub fn random(rng: &mut impl Rng) -> Self {
        let base_idx = rng.random_range(0..4);
        let hair_idx = rng.random_range(0..5);
        let hair_colors = [
            Color::srgb(0.9, 0.75, 0.15), // Blonde/Gold
            Color::srgb(0.75, 0.25, 0.1),  // Ginger/Red
            Color::srgb(0.35, 0.22, 0.12), // Brown
            Color::srgb(0.1, 0.08, 0.1),   // Dark/Black
            Color::srgb(0.65, 0.65, 0.68), // Grey/Silver
            Color::srgb(0.15, 0.55, 0.8),  // Magic Blue
            Color::srgb(0.75, 0.15, 0.55), // Vibrant Purple
        ];
        let hair_color = hair_colors[rng.random_range(0..hair_colors.len())];
        let gear_idx = 0; // Starts empty; equipped armor overrides this

        Self {
            base_idx,
            hair_idx,
            hair_color,
            gear_idx,
        }
    }
}

#[derive(Component, Clone, Debug, Reflect)]
pub struct HeroPortraitImage(pub Handle<Image>);

// ── 16x16 Pixel Art Sprite Assets ────────────────────────────────────

// Legends:
// . = Transparent
// O = Dark Outline [31, 27, 27, 255]
// s = Skin tone (dynamic or static based on base_idx)
// S = Skin shadow
// E = Eye white [255, 255, 255, 255]
// e = Eye pupil [30, 50, 110, 255]
// H = Hair pixels (tinted dynamically)
// G = Gear pixels (tinted dynamically)

const BASE_0: &str = "\
................\
......OOOO......\
....OOssssOO....\
...OssssssssO...\
..OssssssssssO..\
..OsEsessEsessO.\
..OssssssssssO..\
..OssssOssOssO..\
...OssssssssO...\
....OssssssO....\
.....OSSSSO.....\
....OssssssO....\
...OssssssssO...\
..OssssssssssO..\
.OssssssssssssO.\
OOOOOOOOOOOOOOOO";

const BASE_1: &str = "\
................\
......OOOO......\
...OOOssssOOO...\
..OssssssssssO..\
.OsssEsessEsessO\
.OssssssssssssO.\
..OssssssssssO..\
..OssssOssOssO..\
...OssssssssO...\
....OssssssO....\
.....OSSSSO.....\
....OssssssO....\
...OssssssssO...\
..OssssssssssO..\
.OssssssssssssO.\
OOOOOOOOOOOOOOOO";

const BASE_2: &str = "\
................\
.....OOOOOO.....\
...OOssssssOO...\
..OssssssssssO..\
..OsEsessEsessO.\
..OssssssssssO..\
..OssssssssssO..\
..OssssOssOssO..\
...OssssssssO...\
....OssssssO....\
.....OSSSSO.....\
...OOssssssOO...\
..OOssssssssOO..\
.OOssssssssssOO.\
.OOOOOOOOOOOOOO.\
................";

const BASE_3: &str = "\
................\
......OOOO......\
....OOssssOO....\
...OssssssssO...\
..OssssssssssO..\
..OsEsessEsessO.\
..OssssssssssO..\
..OssssOssOssO..\
...OssssssssO...\
....OssssssO....\
.....OSSSSO.....\
....OssssssO....\
...OssssssssO...\
..OssssssssssO..\
.OssssssssssssO.\
OOOOOOOOOOOOOOOO";

const HAIR_0: &str = "\
......OOOO......\
....OOHHHHOO....\
...OHHHHHHHHO...\
..OHHHHHHHHHHO..\
..OHHOOOOOOHHO..\
..OHO......OHO..\
................\
................\
................\
................\
................\
................\
................\
................\
................\
................";

const HAIR_1: &str = "\
......OOOO......\
....OOHHHHOO....\
...OHHHHHHHHO...\
..OHHHHHHHHHHO..\
..OHH......OHH..\
..OHH......OHH..\
..OHH......OHH..\
..OHH......OHH..\
..OHH......OHH..\
...OHH....OHH...\
...OHH....OHH...\
....OHH..OHH....\
................\
................\
................\
................";

const HAIR_2: &str = "\
......OO......\
......OHO.....\
.....OHHO.....\
....OHHHHO....\
...OHHHHHHO...\
...OHHHHHHO...\
...OHH..OHH...\
..............\
..............\
..............\
..............\
..............\
..............\
..............\
..............\
..............";

const HAIR_3: &str = "\
................\
................\
................\
................\
................\
................\
..OHH......OHH..\
..OHHHHHHHHHHO..\
..OOHHHHHHHHOO..\
....OHHHHHHO....\
.....OHHHHO.....\
......OHHO......\
................\
................\
................\
................";

const HAIR_4: &str = "\
................\
................\
................\
................\
................\
................\
................\
................\
................\
................\
................\
................\
................\
................\
................\
................";

const GEAR_0: &str = "\
................\
................\
................\
................\
................\
................\
................\
................\
................\
................\
................\
................\
................\
................\
................\
................";

const GEAR_1: &str = "\
......OOOO......\
....OOGGGGOO....\
...OGGGGGGGGO...\
..OGGGGGGGGGGO..\
..OGGGOOOOGGGO..\
..OGGGO..OGGGO..\
..OGGGGGGGGGGO..\
..OGGGGGGGGGGO..\
...OGGGGGGGGO...\
....OGGGGGGO....\
................\
................\
................\
................\
................\
................";

const GEAR_2: &str = "\
......OO......\
.....OGGO.....\
.....OGGO.....\
....OGGGGO....\
...OGGGGGGO...\
..OGGGGGGGO...\
.OGGGGGGGGGO..\
OOOOOOOOOOOOOO\
.OGGGGGGGGGO..\
..OGGGGGGGO...\
..............\
..............\
..............\
..............\
..............\
..............";

const GEAR_3: &str = "\
......OOOO......\
....OOGGGGOO....\
...OGGGGGGGGO...\
..OGGGGGGGGGGO..\
..OGG......OGG..\
..OGG......OGG..\
..OGG......OGG..\
..OGGGGGGGGGGO..\
...OGGGGGGGGO...\
....OGGGGGGO....\
.....OGGGGO.....\
....OGGGGGGO....\
...OGGGGGGGGO...\
..OGGGGGGGGGGO..\
.OGGGGGGGGGGGGO.\
OOOOOOOOOOOOOOOO";

const GEAR_4: &str = "\
...O..O..O..O...\
...OGOGOGOGO...\
...OGGGGGGGO...\
...OOOOOOOOO...\
................\
................\
................\
................\
................\
................\
................\
................\
................\
................\
................\
................";

fn get_base_sprite(idx: u32) -> &'static str {
    match idx {
        1 => BASE_1,
        2 => BASE_2,
        3 => BASE_3,
        _ => BASE_0,
    }
}

fn get_hair_sprite(idx: u32) -> &'static str {
    match idx {
        0 => HAIR_0,
        1 => HAIR_1,
        2 => HAIR_2,
        3 => HAIR_3,
        _ => HAIR_4,
    }
}

fn get_gear_sprite(idx: u32) -> &'static str {
    match idx {
        1 => GEAR_1,
        2 => GEAR_2,
        3 => GEAR_3,
        4 => GEAR_4,
        _ => GEAR_0,
    }
}

// Draw a 16x16 sprite layout onto the byte buffer
fn draw_sprite_layer(
    pixels: &mut [u8],
    sprite: &str,
    tint: Color,
    base_idx: u32,
    _is_base_layer: bool,
) {
    let tint_rgba = tint.to_srgba();
    let r_tint = tint_rgba.red;
    let g_tint = tint_rgba.green;
    let b_tint = tint_rgba.blue;

    let chars: Vec<char> = sprite.chars().filter(|c| *c != '\n' && *c != '\r').collect();
    for y in 0..16 {
        for x in 0..16 {
            let idx = y * 16 + x;
            if idx >= chars.len() {
                break;
            }
            let c = chars[idx];
            let pixel_idx = idx * 4;

            match c {
                '.' => {} // Transparent
                'O' => {
                    // Outline: Dark grey/black
                    pixels[pixel_idx] = 25;
                    pixels[pixel_idx + 1] = 22;
                    pixels[pixel_idx + 2] = 22;
                    pixels[pixel_idx + 3] = 255;
                }
                's' => {
                    // Skin color based on base index
                    let skin = match base_idx {
                        1 => [245, 215, 195], // Pale rosy/elf
                        2 => [215, 155, 115], // Golden tan/dwarf
                        3 => [135, 165, 140], // Pale undead green
                        _ => [225, 175, 145], // Warm peach
                    };
                    pixels[pixel_idx] = skin[0];
                    pixels[pixel_idx + 1] = skin[1];
                    pixels[pixel_idx + 2] = skin[2];
                    pixels[pixel_idx + 3] = 255;
                }
                'S' => {
                    // Shadowed skin
                    let shadow = match base_idx {
                        1 => [195, 165, 145],
                        2 => [165, 115, 85],
                        3 => [95, 125, 100],
                        _ => [175, 125, 95],
                    };
                    pixels[pixel_idx] = shadow[0];
                    pixels[pixel_idx + 1] = shadow[1];
                    pixels[pixel_idx + 2] = shadow[2];
                    pixels[pixel_idx + 3] = 255;
                }
                'E' => {
                    // Sclera / eye white
                    pixels[pixel_idx] = 255;
                    pixels[pixel_idx + 1] = 255;
                    pixels[pixel_idx + 2] = 255;
                    pixels[pixel_idx + 3] = 255;
                }
                'e' => {
                    // Iris / pupil (Red for undead/base 3, blue otherwise)
                    if base_idx == 3 {
                        pixels[pixel_idx] = 220;
                        pixels[pixel_idx + 1] = 40;
                        pixels[pixel_idx + 2] = 40;
                    } else {
                        pixels[pixel_idx] = 40;
                        pixels[pixel_idx + 1] = 80;
                        pixels[pixel_idx + 2] = 160;
                    }
                    pixels[pixel_idx + 3] = 255;
                }
                'H' => {
                    // Hair body (tinted)
                    pixels[pixel_idx] = (r_tint * 255.0).clamp(0.0, 255.0) as u8;
                    pixels[pixel_idx + 1] = (g_tint * 255.0).clamp(0.0, 255.0) as u8;
                    pixels[pixel_idx + 2] = (b_tint * 255.0).clamp(0.0, 255.0) as u8;
                    pixels[pixel_idx + 3] = 255;
                }
                'G' => {
                    // Gear/helmet body (tinted)
                    pixels[pixel_idx] = (r_tint * 255.0).clamp(0.0, 255.0) as u8;
                    pixels[pixel_idx + 1] = (g_tint * 255.0).clamp(0.0, 255.0) as u8;
                    pixels[pixel_idx + 2] = (b_tint * 255.0).clamp(0.0, 255.0) as u8;
                    pixels[pixel_idx + 3] = 255;
                }
                _ => {}
            }
        }
    }
}

pub fn composite_portrait(
    portrait: &HeroPortrait,
    equipment: Option<&crate::equipment::HeroEquipment>,
) -> Image {
    let mut pixels = vec![0u8; 16 * 16 * 4];

    // 1. Draw Base
    let base_sprite = get_base_sprite(portrait.base_idx);
    draw_sprite_layer(&mut pixels, base_sprite, Color::WHITE, portrait.base_idx, true);

    // 2. Draw Hair
    let hair_sprite = get_hair_sprite(portrait.hair_idx);
    draw_sprite_layer(&mut pixels, hair_sprite, portrait.hair_color, portrait.base_idx, false);

    // 3. Draw Gear / Helmet
    // Gear index is dynamically overridden if the hero has equipped armor
    let gear_idx = if let Some(equip) = equipment {
        if equip.armor_tier > 0 {
            // Determine gear representation based on armor tier
            if equip.armor_tier >= 3 {
                4 // Crown for high tier veterans
            } else {
                // Return 1, 2, or 3 based on portrait's default or simple tier mappings
                let default_gear = portrait.gear_idx;
                if default_gear == 0 {
                    1 // Default to knight helmet if none chosen
                } else {
                    default_gear
                }
            }
        } else {
            0 // No armor, empty gear slot
        }
    } else {
        portrait.gear_idx
    };

    let gear_color = if let Some(equip) = equipment {
        match equip.armor_rarity {
            crate::equipment::GearRarity::Common => Color::srgb(0.7, 0.7, 0.72),
            crate::equipment::GearRarity::Uncommon => Color::srgb(0.2, 0.75, 0.25),
            crate::equipment::GearRarity::Rare => Color::srgb(0.15, 0.5, 0.85),
            crate::equipment::GearRarity::Epic => Color::srgb(0.65, 0.15, 0.75),
            crate::equipment::GearRarity::Legendary => Color::srgb(0.95, 0.45, 0.05),
        }
    } else {
        Color::WHITE
    };

    let gear_sprite = get_gear_sprite(gear_idx);
    draw_sprite_layer(&mut pixels, gear_sprite, gear_color, portrait.base_idx, false);

    // Build the Bevy Image
    Image::new(
        Extent3d {
            width: 16,
            height: 16,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

// ── Bevy Portrait Update System ──────────────────────────────────────

fn update_hero_portrait_images_system(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    query: Query<
        (
            Entity,
            &HeroPortrait,
            &crate::equipment::HeroEquipment,
            Option<&HeroPortraitImage>,
        ),
        (
            With<crate::hero::Hero>,
            Or<(Changed<HeroPortrait>, Changed<crate::equipment::HeroEquipment>)>,
        ),
    >,
) {
    for (entity, portrait, equipment, _opt_portrait_image) in &query {
        let image = composite_portrait(portrait, Some(equipment));
        let handle = images.add(image);
        commands.entity(entity).insert(HeroPortraitImage(handle));
    }
}

pub struct PortraitPlugin;

impl Plugin for PortraitPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<HeroPortrait>();
        app.add_systems(Update, update_hero_portrait_images_system);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::equipment::{HeroEquipment, GearRarity};

    #[test]
    fn test_portrait_generation() {
        let portrait = HeroPortrait {
            base_idx: 0,
            hair_idx: 1,
            hair_color: Color::srgb(1.0, 0.0, 0.0),
            gear_idx: 0,
        };
        let img = composite_portrait(&portrait, None);
        assert_eq!(img.texture_descriptor.size.width, 16);
        assert_eq!(img.texture_descriptor.size.height, 16);
        assert_eq!(img.texture_descriptor.format, TextureFormat::Rgba8UnormSrgb);
        // The transparent color on the edges should remain transparent (alpha = 0)
        assert_eq!(img.data.as_ref().unwrap()[3], 0); // top-left pixel alpha
    }

    #[test]
    fn test_portrait_equipment_override() {
        let portrait = HeroPortrait {
            base_idx: 0,
            hair_idx: 4, // Bald
            hair_color: Color::srgb(1.0, 0.0, 0.0),
            gear_idx: 0,
        };

        // Without armor equipped, gear should be empty
        let img_no_armor = composite_portrait(&portrait, None);
        // Check that row 1 column 4 is empty (outside base head outline, no hair)
        let idx = (1 * 16 + 4) * 4;
        assert_eq!(img_no_armor.data.as_ref().unwrap()[idx + 3], 0);

        // With armor equipped, knight helmet should overlay
        let equipment = HeroEquipment {
            armor_tier: 1,
            armor_rarity: GearRarity::Common,
            ..Default::default()
        };
        let img_with_armor = composite_portrait(&portrait, Some(&equipment));
        // Knight helmet GEAR_1 has 'O' at row 1, column 4
        assert_eq!(img_with_armor.data.as_ref().unwrap()[idx + 3], 255);
    }
}
