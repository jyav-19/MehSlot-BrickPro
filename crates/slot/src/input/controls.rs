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
