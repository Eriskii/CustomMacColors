//! The accent color every app uses and the color icons are tinted. Small enough to travel as one
//! `u64`, the state of the Darwin notification the payload in every app watches.
//!
//! Other programs may read that state (ErisMacShell's menus highlight in the accent), so these
//! bits stay as they are: 63 valid, 24 accent on, 0 to 23 the accent as 0xRRGGBB. See the README.

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    /// The system accent color, as 0xRRGGBB, in place of the one picked in System Settings.
    pub accent: Option<u32>,
    /// The color tinted app icons and folders take, as 0xRRGGBB; `None` follows the accent.
    pub icon_tint: Option<u32>,
}

/// Set in every packed value, so a notification nobody has published yet (state 0) isn't read
/// as settings.
const VALID: u64 = 1 << 63;
const ACCENT_ON: u64 = 1 << 24;
const ICON_TINT_ON: u64 = 1 << 29;
const ICON_TINT_SHIFT: u32 = 30;

impl Settings {
    pub fn pack(&self) -> u64 {
        let mut packed = VALID;
        if let Some(rgb) = self.accent {
            packed |= ACCENT_ON | u64::from(rgb & 0xFF_FFFF);
        }
        if let Some(rgb) = self.icon_tint {
            packed |= ICON_TINT_ON | (u64::from(rgb & 0xFF_FFFF) << ICON_TINT_SHIFT);
        }
        packed
    }

    /// The icon tint in effect: its own color, else a custom accent; `None` leaves icons to macOS.
    pub fn effective_icon_tint(&self) -> Option<u32> {
        self.icon_tint.or(self.accent)
    }

    pub fn unpack(packed: u64) -> Option<Settings> {
        if packed & VALID == 0 {
            return None;
        }
        Some(Settings {
            accent: (packed & ACCENT_ON != 0).then_some((packed & 0xFF_FFFF) as u32),
            icon_tint: (packed & ICON_TINT_ON != 0).then_some(((packed >> ICON_TINT_SHIFT) & 0xFF_FFFF) as u32),
        })
    }
}

/// `#FF6B00`, `FF6B00`, `#f60` or `f60` as 0xRRGGBB.
pub fn parse_hex(text: &str) -> Option<u32> {
    let hex = text.trim();
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    match hex.len() {
        6 => u32::from_str_radix(hex, 16).ok(),
        3 => {
            let short = u32::from_str_radix(hex, 16).ok()?;
            let (r, g, b) = ((short >> 8) & 0xF, (short >> 4) & 0xF, short & 0xF);
            Some((r * 17) << 16 | (g * 17) << 8 | b * 17)
        }
        _ => None,
    }
}

/// The components of 0xRRGGBB, each 0 to 1.
pub fn components(rgb: u32) -> (f64, f64, f64) {
    let c = |shift: u32| f64::from((rgb >> shift) & 0xFF) / 255.0;
    (c(16), c(8), c(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let all = [
            Settings::default(),
            Settings { accent: Some(0xFF6B00), icon_tint: None },
            Settings { accent: Some(0), icon_tint: Some(0xFFFFFF) },
            Settings { accent: Some(0xFFFFFF), icon_tint: Some(0) },
            Settings { accent: None, icon_tint: Some(0x00A0A0) },
        ];
        for settings in all {
            assert_eq!(Settings::unpack(settings.pack()), Some(settings));
        }
    }

    /// What other programs read (see the top of the file).
    #[test]
    fn the_accent_stays_where_others_read_it() {
        let packed = Settings { accent: Some(0xB473F7), icon_tint: Some(0xDDB3FF) }.pack();
        assert_ne!(packed & 1 << 63, 0);
        assert_ne!(packed & 1 << 24, 0);
        assert_eq!(packed & 0xFF_FFFF, 0xB473F7);
        assert_eq!(Settings { accent: None, icon_tint: Some(0xDDB3FF) }.pack() & 1 << 24, 0);
    }

    #[test]
    fn icons_follow_the_accent_unless_tinted_their_own_way() {
        let accent = Settings { accent: Some(0xFF6B00), ..Settings::default() };
        assert_eq!(accent.effective_icon_tint(), Some(0xFF6B00));
        assert_eq!(Settings { icon_tint: Some(0x00A0A0), ..accent }.effective_icon_tint(), Some(0x00A0A0));
        assert_eq!(Settings::default().effective_icon_tint(), None);
    }

    #[test]
    fn nothing_published_is_no_settings() {
        assert_eq!(Settings::unpack(0), None);
    }

    #[test]
    fn reads_hex() {
        assert_eq!(parse_hex("#FF6B00"), Some(0xFF6B00));
        assert_eq!(parse_hex("ff6b00"), Some(0xFF6B00));
        assert_eq!(parse_hex("#f60"), Some(0xFF6600));
        assert_eq!(parse_hex(" #00a0a0 "), Some(0x00A0A0));
        assert_eq!(parse_hex("orange"), None);
        assert_eq!(parse_hex("#FF6B0"), None);
        assert_eq!(parse_hex("+FF6B00"), None);
    }
}
