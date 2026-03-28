use bevy::ui::Val;

pub fn px(value: f32) -> Val {
    Val::Px(value)
}

pub fn pct(value: f32) -> Val {
    Val::Percent(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn px_creates_val_px() {
        assert_eq!(px(8.0), Val::Px(8.0));
    }

    #[test]
    fn pct_creates_val_percent() {
        assert_eq!(pct(50.0), Val::Percent(50.0));
    }
}
