//! Night Mode: what the Fn switch changes while it is on. `Config/night.ini` keeps three
//! numbers, a line each: `brightness=1` (the backlight step, as the brightness shortcut sets),
//! `exposure=4` (how far the picture is dimmed below the backlight's lowest step) and
//! `colour=3` (the warm, less blue filter's step). The day's own settings are never touched.

use std::path::Path;

use slot_store::{BLUE_LIGHT_MAX, BRIGHTNESS_MAX};

/// The most the picture can be dimmed below the backlight's lowest step.
pub const EXPOSURE_MAX: u8 = 10;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Night {
    pub brightness: u8,
    pub exposure: u8,
    pub colour: u8,
}

impl Default for Night {
    fn default() -> Self {
        Night {
            brightness: 1,
            exposure: 4,
            colour: BLUE_LIGHT_MAX / 2,
        }
    }
}

impl Night {
    pub fn read(root: &Path) -> Night {
        std::fs::read_to_string(root.join("Config").join("night.ini"))
            .map(|text| Night::parse(&text))
            .unwrap_or_default()
    }

    pub fn parse(text: &str) -> Night {
        let mut night = Night::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let Ok(value) = value.trim().parse::<u8>() else {
                continue;
            };
            match key.trim() {
                "brightness" => night.brightness = value.min(BRIGHTNESS_MAX),
                "exposure" => night.exposure = value.min(EXPOSURE_MAX),
                "colour" => night.colour = value.min(BLUE_LIGHT_MAX),
                _ => {}
            }
        }
        night
    }

    /// Writes `Config/night.ini` through a temporary file.
    pub fn write(&self, root: &Path) -> std::io::Result<()> {
        let dir = root.join("Config");
        std::fs::create_dir_all(&dir)?;
        let temp = dir.join("night.ini.tmp");
        std::fs::write(
            &temp,
            format!("brightness={}\nexposure={}\ncolour={}\n", self.brightness, self.exposure, self.colour),
        )?;
        std::fs::rename(temp, dir.join("night.ini"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_changes_only_what_it_names_and_keeps_to_the_limits() {
        let night = Night::parse("exposure=7\nbrightness=99\nbogus=1\ncolour=x\n");
        assert_eq!(night.exposure, 7);
        assert_eq!(night.brightness, BRIGHTNESS_MAX);
        assert_eq!(night.colour, Night::default().colour);
        assert_eq!(Night::parse(""), Night::default());
    }
}
