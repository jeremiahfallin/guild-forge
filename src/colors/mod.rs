use bevy::color::Color;

/// Convert a hex color (e.g. 0xRRGGBB) to a Bevy `Color`.
pub fn rgb(hex: u32) -> Color {
    let r = ((hex >> 16) & 0xFF) as f32 / 255.0;
    let g = ((hex >> 8) & 0xFF) as f32 / 255.0;
    let b = (hex & 0xFF) as f32 / 255.0;
    Color::srgb(r, g, b)
}

/// Create a color from RGBA float components (0.0 - 1.0).
pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Color {
    Color::srgba(r, g, b, a)
}

// ---------------------------------------------------------------------------
// Basic colors
// ---------------------------------------------------------------------------

pub fn white() -> Color {
    rgb(0xFFFFFF)
}
pub fn black() -> Color {
    rgb(0x000000)
}
pub fn transparent() -> Color {
    rgba(0.0, 0.0, 0.0, 0.0)
}

// ---------------------------------------------------------------------------
// Tailwind Slate
// ---------------------------------------------------------------------------

pub fn slate_50() -> Color { rgb(0xf8fafc) }
pub fn slate_100() -> Color { rgb(0xf1f5f9) }
pub fn slate_200() -> Color { rgb(0xe2e8f0) }
pub fn slate_300() -> Color { rgb(0xcbd5e1) }
pub fn slate_400() -> Color { rgb(0x94a3b8) }
pub fn slate_500() -> Color { rgb(0x64748b) }
pub fn slate_600() -> Color { rgb(0x475569) }
pub fn slate_700() -> Color { rgb(0x334155) }
pub fn slate_800() -> Color { rgb(0x1e293b) }
pub fn slate_900() -> Color { rgb(0x0f172a) }
pub fn slate_950() -> Color { rgb(0x020617) }

// ---------------------------------------------------------------------------
// Tailwind Red
// ---------------------------------------------------------------------------

pub fn red_50() -> Color { rgb(0xfef2f2) }
pub fn red_100() -> Color { rgb(0xfee2e2) }
pub fn red_200() -> Color { rgb(0xfecaca) }
pub fn red_300() -> Color { rgb(0xfca5a5) }
pub fn red_400() -> Color { rgb(0xf87171) }
pub fn red_500() -> Color { rgb(0xef4444) }
pub fn red_600() -> Color { rgb(0xdc2626) }
pub fn red_700() -> Color { rgb(0xb91c1c) }
pub fn red_800() -> Color { rgb(0x991b1b) }
pub fn red_900() -> Color { rgb(0x7f1d1d) }
pub fn red_950() -> Color { rgb(0x450a0a) }

// ---------------------------------------------------------------------------
// Tailwind Green
// ---------------------------------------------------------------------------

pub fn green_50() -> Color { rgb(0xf0fdf4) }
pub fn green_100() -> Color { rgb(0xdcfce7) }
pub fn green_200() -> Color { rgb(0xbbf7d0) }
pub fn green_300() -> Color { rgb(0x86efac) }
pub fn green_400() -> Color { rgb(0x4ade80) }
pub fn green_500() -> Color { rgb(0x22c55e) }
pub fn green_600() -> Color { rgb(0x16a34a) }
pub fn green_700() -> Color { rgb(0x15803d) }
pub fn green_800() -> Color { rgb(0x166534) }
pub fn green_900() -> Color { rgb(0x14532d) }
pub fn green_950() -> Color { rgb(0x052e16) }

// ---------------------------------------------------------------------------
// Tailwind Blue
// ---------------------------------------------------------------------------

pub fn blue_50() -> Color { rgb(0xeff6ff) }
pub fn blue_100() -> Color { rgb(0xdbeafe) }
pub fn blue_200() -> Color { rgb(0xbfdbfe) }
pub fn blue_300() -> Color { rgb(0x93c5fd) }
pub fn blue_400() -> Color { rgb(0x60a5fa) }
pub fn blue_500() -> Color { rgb(0x3b82f6) }
pub fn blue_600() -> Color { rgb(0x2563eb) }
pub fn blue_700() -> Color { rgb(0x1d4ed8) }
pub fn blue_800() -> Color { rgb(0x1e40af) }
pub fn blue_900() -> Color { rgb(0x1e3a8a) }
pub fn blue_950() -> Color { rgb(0x172554) }

// ---------------------------------------------------------------------------
// Tailwind Yellow
// ---------------------------------------------------------------------------

pub fn yellow_50() -> Color { rgb(0xfefce8) }
pub fn yellow_100() -> Color { rgb(0xfef9c3) }
pub fn yellow_200() -> Color { rgb(0xfef08a) }
pub fn yellow_300() -> Color { rgb(0xfde047) }
pub fn yellow_400() -> Color { rgb(0xfacc15) }
pub fn yellow_500() -> Color { rgb(0xeab308) }
pub fn yellow_600() -> Color { rgb(0xca8a04) }
pub fn yellow_700() -> Color { rgb(0xa16207) }
pub fn yellow_800() -> Color { rgb(0x854d0e) }
pub fn yellow_900() -> Color { rgb(0x713f12) }
pub fn yellow_950() -> Color { rgb(0x422006) }

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::color::Srgba;

    #[test]
    fn rgb_converts_hex_to_color() {
        let color = rgb(0xFF8000);
        let expected = Color::srgb(1.0, 128.0 / 255.0, 0.0);
        // Compare via Srgba to avoid floating point format issues
        let c: Srgba = color.into();
        let e: Srgba = expected.into();
        assert!((c.red - e.red).abs() < 0.001);
        assert!((c.green - e.green).abs() < 0.001);
        assert!((c.blue - e.blue).abs() < 0.001);
    }

    #[test]
    fn rgba_creates_color_with_alpha() {
        let color = rgba(1.0, 0.5, 0.0, 0.8);
        let c: Srgba = color.into();
        assert!((c.red - 1.0).abs() < 0.001);
        assert!((c.green - 0.5).abs() < 0.001);
        assert!((c.blue - 0.0).abs() < 0.001);
        assert!((c.alpha - 0.8).abs() < 0.001);
    }

    #[test]
    fn white_is_ffffff() {
        let c: Srgba = white().into();
        assert!((c.red - 1.0).abs() < 0.001);
        assert!((c.green - 1.0).abs() < 0.001);
        assert!((c.blue - 1.0).abs() < 0.001);
    }

    #[test]
    fn black_is_000000() {
        let c: Srgba = black().into();
        assert!((c.red).abs() < 0.001);
        assert!((c.green).abs() < 0.001);
        assert!((c.blue).abs() < 0.001);
    }

    #[test]
    fn transparent_has_zero_alpha() {
        let c: Srgba = transparent().into();
        assert!((c.alpha).abs() < 0.001);
    }

    #[test]
    fn palette_functions_return_colors() {
        // Spot-check a few palette colors
        let s500: Srgba = slate_500().into();
        assert!((s500.red - 0x64 as f32 / 255.0).abs() < 0.001);

        let r500: Srgba = red_500().into();
        assert!((r500.red - 0xef as f32 / 255.0).abs() < 0.001);

        let b500: Srgba = blue_500().into();
        assert!((b500.blue - 0xf6 as f32 / 255.0).abs() < 0.001);
    }
}
