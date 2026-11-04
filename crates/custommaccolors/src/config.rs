//! The settings file, `~/.config/custommaccolors/config.toml`, and publishing it to every app: the
//! packed settings go out as the state of a Darwin notification the payload watches, which works for
//! sandboxed apps and needs no root. The daemon holds the notification open so the state outlives
//! whatever published it, and publishes the file when someone logs in.

use std::ffi::CStr;
use std::path::{Path, PathBuf};

use colors_core::settings::{Settings, parse_hex};
use eris_inject::notify::Channel;

pub const NOTIFICATION: &CStr = c"io.github.eriskii.custommaccolors.settings";

pub fn channel() -> Result<Channel, String> {
    Channel::open(NOTIFICATION)
}

/// The file in `home`.
pub fn path_in(home: &Path) -> PathBuf {
    home.join(".config/custommaccolors/config.toml")
}

/// The file of whoever runs this.
pub fn path() -> Result<PathBuf, String> {
    std::env::var_os("HOME").map(|home| path_in(Path::new(&home))).ok_or_else(|| "no HOME".to_string())
}

/// The settings in `path`. With no file yet, the colors ErisMenus had, which these came out of;
/// with neither, the defaults.
pub fn load(path: &Path) -> Result<Settings, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse(&text).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(from_erismenus(path).unwrap_or_default()),
        Err(e) => Err(format!("can't read {}: {e}", path.display())),
    }
}

/// The colors in ErisMenus' file, next to the folder of `path`, if it can be read.
fn from_erismenus(path: &Path) -> Option<Settings> {
    let text = std::fs::read_to_string(path.parent()?.parent()?.join("erismenus/config.toml")).ok()?;
    parse_keeping(&text, |key| matches!(key, "accent" | "icon_tint")).ok()
}

/// Writes `settings` to `path`, sends them to every app, and gives macOS's own icon tint the
/// color (see `icons`).
pub fn save(path: &Path, settings: &Settings) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("can't make {}: {e}", dir.display()))?;
    }
    std::fs::write(path, render(settings)).map_err(|e| format!("can't write {}: {e}", path.display()))?;
    publish(settings)?;
    crate::icons::apply(settings.effective_icon_tint())
}

pub fn publish(settings: &Settings) -> Result<(), String> {
    channel()?.publish(settings.pack())
}

/// What every app has now, if anything was published.
pub fn live() -> Option<Settings> {
    channel().ok()?.current().and_then(Settings::unpack)
}

/// Changes one setting by name, as `custommaccolors set` takes it.
pub fn set(settings: &mut Settings, key: &str, value: &str) -> Result<(), String> {
    match key {
        "accent" => settings.accent = accent(value)?,
        "icon-tint" | "icon_tint" => settings.icon_tint = icon_tint(value)?,
        _ => return Err(format!("no setting {key:?}; accent or icon-tint")),
    }
    Ok(())
}

fn accent(value: &str) -> Result<Option<u32>, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "system" | "off" | "none" => Ok(None),
        _ => parse_hex(value).map(Some).ok_or_else(|| format!("{value:?} isn't a hex color like #FF6B00 (or \"system\")")),
    }
}

fn icon_tint(value: &str) -> Result<Option<u32>, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "accent" | "off" | "none" => Ok(None),
        _ => parse_hex(value).map(Some).ok_or_else(|| format!("{value:?} isn't a hex color like #FF6B00 (or \"accent\")")),
    }
}

pub fn hex(rgb: u32) -> String {
    format!("#{rgb:06X}")
}

fn render(s: &Settings) -> String {
    format!(
        "# CustomMacColors settings. The settings window (CustomMacColors in Applications) and\n\
         # `custommaccolors set` change this file; after editing it by hand, run `custommaccolors reload`.\n\
         \n\
         # The system accent color in every app: a hex color like \"#FF6B00\", or \"system\" for the one\n\
         # picked in System Settings.\n\
         accent = \"{}\"\n\
         \n\
         # The color tinted app icons and folders take (Tinted icons, in System Settings > Appearance):\n\
         # a hex color, or \"accent\" to follow the accent above.\n\
         icon_tint = \"{}\"\n",
        s.accent.map_or_else(|| "system".to_string(), hex),
        s.icon_tint.map_or_else(|| "accent".to_string(), hex),
    )
}

/// `key = value` lines; `#` starts a comment, values may be quoted.
fn parse(text: &str) -> Result<Settings, String> {
    parse_keeping(text, |_| true)
}

/// The same, reading only the keys `wanted` lets through.
fn parse_keeping(text: &str, wanted: fn(&str) -> bool) -> Result<Settings, String> {
    let mut settings = Settings::default();
    for (n, line) in text.lines().enumerate() {
        let line = strip_comment(line).trim();
        if line.is_empty() {
            continue;
        }
        let (key, value) = line.split_once('=').ok_or_else(|| format!("line {}: expected key = value", n + 1))?;
        let value = value.trim().trim_matches('"');
        if !wanted(key.trim()) {
            continue;
        }
        set(&mut settings, key.trim(), value).map_err(|e| format!("line {}: {e}", n + 1))?;
    }
    Ok(settings)
}

/// A line without its comment. A `#` in quotes, or starting a value (`accent = #FF6B00`), is a
/// hex color, not a comment.
fn strip_comment(line: &str) -> &str {
    let mut quoted = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '#' if !quoted && !line[..i].trim_end().ends_with('=') => return &line[..i],
            _ => {}
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_the_file() {
        for settings in [
            Settings::default(),
            Settings { accent: Some(0xFF6B00), icon_tint: None },
            Settings { accent: Some(0x00A0A0), icon_tint: Some(0x3060FF) },
        ] {
            assert_eq!(parse(&render(&settings)), Ok(settings));
        }
    }

    #[test]
    fn reads_hand_edits() {
        let s = parse("accent = #f60   # orange\n\n").unwrap();
        assert_eq!(s.accent, Some(0xFF6600));
        assert!(parse("accent = \"#FF6B00\"").is_ok());
        assert_eq!(parse("icon_tint = #00A0A0").unwrap().icon_tint, Some(0x00A0A0));
        assert_eq!(parse("icon_tint = accent").unwrap().icon_tint, None);
        assert!(parse("accent = purple").is_err());
        assert!(parse("colour = #FF6B00").is_err());
    }

    #[test]
    fn takes_the_colors_from_erismenus_when_there_is_no_file_yet() {
        let home = std::env::temp_dir().join(format!("custommaccolors-test-{}", std::process::id()));
        std::fs::create_dir_all(home.join(".config/erismenus")).unwrap();
        std::fs::write(
            home.join(".config/erismenus/config.toml"),
            "# ErisMenus\naccent = \"#B473F7\"\nicon_tint = \"#DDB3FF\"\nmenus = true\nglass = false\nanimation = \"squishy\"\n",
        )
        .unwrap();
        let path = path_in(&home);
        assert_eq!(load(&path), Ok(Settings { accent: Some(0xB473F7), icon_tint: Some(0xDDB3FF) }));
        // Its own file, once there is one, is what counts.
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, render(&Settings::default())).unwrap();
        assert_eq!(load(&path), Ok(Settings::default()));
        std::fs::remove_dir_all(&home).unwrap();
    }
}
