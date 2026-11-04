//! Which game button each physical button sends, console by console: `Config/controls.ini`,
//! one line a console, like `gba=X:Start,Y:Select`. A button that is not listed sends itself,
//! and `None` sends nothing. SELECT is never remapped: it is the modifier of the shortcuts.

use std::path::Path;

use slot_input::Btn;
use slot_store::Platform;

/// The buttons a game can be handed, and so the ones that can be sent and remapped.
pub const GAME_BUTTONS: [Btn; 14] = [
    Btn::Up,
    Btn::Down,
    Btn::Left,
    Btn::Right,
    Btn::A,
    Btn::B,
    Btn::X,
    Btn::Y,
    Btn::L1,
    Btn::R1,
    Btn::L2,
    Btn::R2,
    Btn::Start,
    Btn::Select,
];

/// For each physical button listed, the game button it sends (or none).
pub type Remap = Vec<(Btn, Option<Btn>)>;

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct ButtonMaps {
    maps: [Remap; Platform::ALL.len()],
}

fn platform_key(platform: Platform) -> String {
    format!("{platform:?}").to_ascii_lowercase()
}

fn button(name: &str) -> Option<Btn> {
    let name = name.trim();
    GAME_BUTTONS
        .into_iter()
        .find(|b| format!("{b:?}").eq_ignore_ascii_case(name))
}

/// A button's name, or `none` for no button at all.
fn target(name: &str) -> Option<Option<Btn>> {
    match name.trim().to_ascii_lowercase().as_str() {
        "none" | "" => Some(None),
        _ => button(name).map(Some),
    }
}

impl ButtonMaps {
    pub fn read(root: &Path) -> ButtonMaps {
        std::fs::read_to_string(root.join("Config").join("controls.ini"))
            .map(|text| ButtonMaps::parse(&text))
            .unwrap_or_default()
    }

    pub fn parse(text: &str) -> ButtonMaps {
        let mut maps = ButtonMaps::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            let Some(platform) = Platform::ALL.into_iter().find(|p| platform_key(*p) == key) else {
                continue;
            };
            let mut remap = Remap::new();
            for pair in value.split(',') {
                let Some((from, to)) = pair.split_once(':') else {
                    continue;
                };
                let (Some(from), Some(to)) = (button(from), target(to)) else {
                    continue;
                };
                if from == Btn::Select {
                    continue;
                }
                remap.retain(|(f, _)| *f != from);
                remap.push((from, to));
            }
            maps.maps[platform.index()] = remap;
        }
        maps
    }

    /// What each physical button listed sends to a game of `platform`.
    pub fn for_platform(&self, platform: Platform) -> &[(Btn, Option<Btn>)] {
        &self.maps[platform.index()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_names_what_each_button_sends() {
        let first = Platform::ALL[0];
        let text = format!("{}=X:Start, y:none ,Select:A,Bogus:B\n", platform_key(first));
        let maps = ButtonMaps::parse(&text);
        assert_eq!(
            maps.for_platform(first),
            &[(Btn::X, Some(Btn::Start)), (Btn::Y, None)]
        );
    }

    #[test]
    fn no_file_leaves_every_button_sending_itself() {
        let maps = ButtonMaps::parse("");
        assert!(maps.for_platform(Platform::ALL[0]).is_empty());
    }
}

/// The shortcuts: the standard ones with those `Config/shortcuts.ini` changes.
pub fn read_shortcuts(root: &Path, home_chords: bool) -> slot_input::Shortcuts {
    let text = std::fs::read_to_string(root.join("Config").join("shortcuts.ini")).unwrap_or_default();
    slot_input::Shortcuts::parse(&text, home_chords)
}

/// Writes `Config/shortcuts.ini` through a temporary file.
pub fn write_shortcuts(root: &Path, shortcuts: &slot_input::Shortcuts) -> std::io::Result<()> {
    let dir = root.join("Config");
    std::fs::create_dir_all(&dir)?;
    let temp = dir.join("shortcuts.ini.tmp");
    std::fs::write(&temp, shortcuts.to_text())?;
    std::fs::rename(temp, dir.join("shortcuts.ini"))
}

impl ButtonMaps {
    /// The game buttons a game of `platform` has, in the order Button Remapping steps through.
    pub fn choices(platform: Platform) -> &'static [Btn] {
        match platform {
            Platform::Gba => &[
                Btn::A,
                Btn::B,
                Btn::L1,
                Btn::R1,
                Btn::Start,
                Btn::Select,
                Btn::Up,
                Btn::Down,
                Btn::Left,
                Btn::Right,
            ],
            Platform::Gb | Platform::Gbc | Platform::Nes => &[
                Btn::A,
                Btn::B,
                Btn::Start,
                Btn::Select,
                Btn::Up,
                Btn::Down,
                Btn::Left,
                Btn::Right,
            ],
            Platform::Snes => &[
                Btn::A,
                Btn::B,
                Btn::X,
                Btn::Y,
                Btn::L1,
                Btn::R1,
                Btn::Start,
                Btn::Select,
                Btn::Up,
                Btn::Down,
                Btn::Left,
                Btn::Right,
            ],
            Platform::Ps1 => &[
                Btn::A,
                Btn::B,
                Btn::X,
                Btn::Y,
                Btn::L1,
                Btn::R1,
                Btn::L2,
                Btn::R2,
                Btn::Start,
                Btn::Select,
                Btn::Up,
                Btn::Down,
                Btn::Left,
                Btn::Right,
            ],
        }
    }

    /// What the physical button `from` sends in a game of `platform`: itself unless listed,
    /// and none for a button set to send nothing.
    pub fn target_of(&self, platform: Platform, from: Btn) -> Option<Btn> {
        match self.maps[platform.index()].iter().find(|(f, _)| *f == from) {
            Some((_, to)) => *to,
            None => Some(from),
        }
    }

    /// Sets what `from` sends in a game of `platform`; sending itself is the same as no line.
    pub fn set_target(&mut self, platform: Platform, from: Btn, to: Option<Btn>) {
        let remap = &mut self.maps[platform.index()];
        remap.retain(|(f, _)| *f != from);
        if to != Some(from) {
            remap.push((from, to));
        }
    }

    /// Every button of `platform` sending itself again.
    pub fn reset(&mut self, platform: Platform) {
        self.maps[platform.index()].clear();
    }

    /// Writes `Config/controls.ini`, a console to a line, through a temporary file.
    pub fn write(&self, root: &Path) -> std::io::Result<()> {
        let mut text = String::new();
        for platform in Platform::ALL {
            let remap = &self.maps[platform.index()];
            if remap.is_empty() {
                continue;
            }
            let pairs: Vec<String> = remap
                .iter()
                .map(|(from, to)| match to {
                    Some(to) => format!("{from:?}:{to:?}"),
                    None => format!("{from:?}:None"),
                })
                .collect();
            text.push_str(&format!("{}={}\n", platform_key(platform), pairs.join(",")));
        }
        let dir = root.join("Config");
        std::fs::create_dir_all(&dir)?;
        let temp = dir.join("controls.ini.tmp");
        std::fs::write(&temp, text)?;
        std::fs::rename(temp, dir.join("controls.ini"))
    }
}
