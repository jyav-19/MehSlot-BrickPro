use crate::{Btn, Millis, RawEvent};

pub const SELECT_CHORD_MS: Millis = 600;

pub const SELECT_TAP_MS: Millis = 50;
pub const MENU_TAP_MS: Millis = 250;
pub const MENU_DOUBLE_TAP_MS: Millis = 350;
pub const MENU_HOLD_MS: Millis = 1000;
pub const FF_DOUBLE_TAP_MS: Millis = 250;
pub const MUTE_CHORD_MS: Millis = 200;
pub const POWER_HOLD_MS: Millis = 1000;

pub const VOLUME_REPEAT_DELAY_MS: Millis = 400;
pub const VOLUME_REPEAT_MS: Millis = 120;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Action {
    GbaDown(Btn),
    GbaUp(Btn),
    ShelfLeft,
    ShelfRight,
    Insert,
    Eject,
    Polaroids,
    SaveState,
    LoadState,
    RewindStart,
    RewindStop,
    FfStart,
    FfStop,
    BrightnessUp,
    BrightnessDown,
    BlueLightUp,
    BlueLightDown,
    VolumeUp,
    VolumeDown,
    QuickMenu,
    GameMenu,
    MuteToggle,
    ColourCorrectionToggle,
    PaletteNext,
    PalettePrev,
    ShaderNext,
    ShaderPrev,
    LedToggle,
    /// Brick Home+X: switch the GBA picture between 3:2 and full-screen 4:3.
    PictureToggle,
    /// Brick Home+L1 / Home+R1: the previous / next overlay for the system being played.
    OverlayPrev,
    OverlayNext,
    PowerPress,
    PowerTap,
    PowerHold,
    PowerOff,
    LidClose,
    LidOpen,
    /// While capturing (the Controls page asking for a new shortcut): the next shortcut pressed,
    /// reported in place of its action.
    Capture(Modifier, Btn),
}

#[derive(Copy, Clone, Default)]
enum Select {
    #[default]
    Idle,
    Held {
        since: Millis,
        chorded: bool,
        sent: Option<Millis>,
    },
    ReleaseDue(Millis),
}

#[derive(Default)]
pub struct Gestures {
    select: Select,
    home_chords: bool,
    chord_held: u32,
    menu_down_at: Option<Millis>,
    menu_last_tap: Option<Millis>,
    menu_eject_fired: bool,
    power_down_at: Option<Millis>,
    power_hold_fired: bool,
    vol_up_at: Option<Millis>,
    vol_down_at: Option<Millis>,
    vol_up_ramp: Option<Millis>,
    vol_down_ramp: Option<Millis>,
    mute_fired: bool,
    ff_on: bool,
    ff_latched: bool,
    ff_latching_press: bool,
    r2_refused: bool,
    r2_last_release: Option<Millis>,
    rewinding: bool,
    /// A PlayStation game is running: its pad has L2 and R2 of its own, so the triggers go to
    /// the game, and fast forward moves to HOME+R2 (HOME+L2 asks for a rewind, which a
    /// PlayStation game doesn't record).
    triggers_to_game: bool,
    /// The shortcuts: which button held with SELECT or HOME does what.
    shortcuts: Shortcuts,
    /// The next shortcut pressed is reported as `Action::Capture`, not done.
    capture: bool,
}

fn ramp_due(down: Option<Millis>, last: Option<Millis>, now: Millis) -> bool {
    let Some(down) = down else {
        return false;
    };
    if now.saturating_sub(down) < VOLUME_REPEAT_DELAY_MS {
        return false;
    }
    last.is_none_or(|l| now.saturating_sub(l) >= VOLUME_REPEAT_MS)
}

impl Gestures {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn brick() -> Self {
        Self { home_chords: true, shortcuts: Shortcuts::standard(true), ..Self::default() }
    }

    pub fn set_triggers_to_game(&mut self, on: bool) {
        self.triggers_to_game = on;
    }

    pub fn shortcuts(&self) -> &Shortcuts {
        &self.shortcuts
    }

    /// New shortcuts: any combo held down is let go of first.
    pub fn set_shortcuts(&mut self, shortcuts: Shortcuts) {
        self.shortcuts = shortcuts;
        self.chord_held &= TRIGGER_BITS;
    }

    pub fn set_capture(&mut self, on: bool) {
        self.capture = on;
    }

    pub fn ff_latched(&self) -> bool {
        self.ff_latched
    }

    pub fn drop_ff_latch(&mut self) -> Vec<Action> {
        if !self.ff_latched {
            return Vec::new();
        }
        self.ff_clear()
    }

    pub fn feed(&mut self, ev: RawEvent, now: Millis) -> Vec<Action> {
        match ev {
            RawEvent::Down(b) => self.down(b, now),
            RawEvent::Up(b) => self.up(b, now),
        }
    }

    pub fn tick(&mut self, now: Millis) -> Vec<Action> {
        let mut out = Vec::new();
        if let Select::ReleaseDue(due) = self.select {
            if now >= due {
                self.select = Select::Idle;
                out.push(Action::GbaUp(Btn::Select));
            }
        }
        if let Select::Held {
            since,
            chorded: false,
            sent: None,
        } = self.select
        {
            if now.saturating_sub(since) >= SELECT_CHORD_MS {
                out.extend(self.hand_over_select(now));
            }
        }
        if let Some(d) = self.menu_down_at {
            if !self.menu_eject_fired && now.saturating_sub(d) >= MENU_HOLD_MS {
                self.menu_eject_fired = true;
                out.push(Action::Eject);
            }
        }
        if let Some(d) = self.power_down_at {
            if !self.power_hold_fired && now.saturating_sub(d) >= POWER_HOLD_MS {
                self.power_hold_fired = true;
                out.push(Action::PowerHold);
            }
        }
        if !self.mute_fired {
            if ramp_due(self.vol_up_at, self.vol_up_ramp, now) {
                self.vol_up_ramp = Some(now);
                out.push(Action::VolumeUp);
            }
            if ramp_due(self.vol_down_at, self.vol_down_ramp, now) {
                self.vol_down_ramp = Some(now);
                out.push(Action::VolumeDown);
            }
        }
        out
    }

    fn down(&mut self, b: Btn, now: Millis) -> Vec<Action> {
        if let (true, Some(bit)) = (self.triggers_to_game, trigger_bit(b)) {
            if self.menu_down_at.is_none() {
                return vec![Action::GbaDown(b)];
            }
            self.chord_held |= bit;
            self.menu_eject_fired = true; // consume Home: no tap menu or hold eject
            return match b {
                Btn::L2 => self.rewind_start(),
                _ => self.ff_down(now),
            };
        }
        if let Some(captured) = self.capture_chord(b, now) {
            return captured;
        }
        if let (true, Some(_), Some((at, action))) =
            (self.home_chords, self.menu_down_at, self.shortcuts.find(Modifier::Home, b))
        {
            let bit = 1u32 << at;
            if self.chord_held & bit != 0 {
                return Vec::new();
            }
            self.chord_held |= bit;
            self.menu_eject_fired = true; // consume Home: no tap menu or hold eject
            return vec![action];
        }
        if let (true, Some((at, action))) = (self.chording(now), self.shortcuts.find(Modifier::Select, b)) {
            self.mark_chorded();
            self.chord_held |= 1u32 << at;
            return vec![action];
        }
        match b {
            Btn::Select => self.select_down(now),
            Btn::Menu => self.menu_down(now),
            Btn::Power => {
                self.power_down_at = Some(now);
                self.power_hold_fired = false;
                vec![Action::PowerPress]
            }
            Btn::Lid => vec![Action::LidClose],
            Btn::VolUp | Btn::VolDown => self.volume_press(b, now),
            Btn::L2 => self.rewind_start(),
            Btn::R2 => self.ff_down(now),
            _ => {
                let mut out = self.hand_over_select(now);
                out.push(Action::GbaDown(b));
                out
            }
        }
    }

    fn up(&mut self, b: Btn, now: Millis) -> Vec<Action> {
        if let Some(bit) = trigger_bit(b) {
            if self.chord_held & bit != 0 {
                self.chord_held &= !bit;
                return match b {
                    Btn::L2 => self.rewind_stop(),
                    _ => self.ff_up(now),
                };
            }
            if self.triggers_to_game {
                return vec![Action::GbaUp(b)];
            }
        }
        let held = (0..self.shortcuts.list.len())
            .find(|at| self.chord_held & (1u32 << at) != 0 && self.shortcuts.list[*at].combo.is_some_and(|(_, button)| button == b));
        if let Some(at) = held {
            self.chord_held &= !(1u32 << at);
            return Vec::new();
        }
        match b {
            Btn::Select => self.select_up(now),
            Btn::Menu => self.menu_up(now),
            Btn::Power => self.power_up(),
            Btn::Lid => vec![Action::LidOpen],
            Btn::VolUp | Btn::VolDown => self.volume_release(b),
            Btn::L2 => self.rewind_stop(),
            Btn::R2 => self.ff_up(now),
            _ => vec![Action::GbaUp(b)],
        }
    }

    /// With capture on, a shortcut pressed (SELECT or HOME held, then a button) is reported as
    /// `Action::Capture` and capture is over. None when this is not such a press.
    fn capture_chord(&mut self, b: Btn, now: Millis) -> Option<Vec<Action>> {
        if !self.capture || !BINDABLE.contains(&b) {
            return None;
        }
        let modifier = if self.home_chords && self.menu_down_at.is_some() {
            self.menu_eject_fired = true; // consume Home: no tap menu or hold eject
            Modifier::Home
        } else if self.chording(now) {
            self.mark_chorded();
            Modifier::Select
        } else {
            return None;
        };
        self.capture = false;
        Some(vec![Action::Capture(modifier, b)])
    }

    fn chording(&self, now: Millis) -> bool {
        match self.select {
            Select::Held { since, chorded, .. } => {
                chorded || now.saturating_sub(since) < SELECT_CHORD_MS
            }
            _ => false,
        }
    }

    fn mark_chorded(&mut self) {
        if let Select::Held { chorded, .. } = &mut self.select {
            *chorded = true;
        }
    }

    fn select_down(&mut self, now: Millis) -> Vec<Action> {
        let mut out = Vec::new();
        if matches!(self.select, Select::ReleaseDue(_)) {
            out.push(Action::GbaUp(Btn::Select));
        }
        if !matches!(self.select, Select::Held { .. }) {
            self.select = Select::Held {
                since: now,
                chorded: false,
                sent: None,
            };
        }
        out
    }

    fn hand_over_select(&mut self, now: Millis) -> Vec<Action> {
        match &mut self.select {
            Select::Held {
                chorded: false,
                sent: sent @ None,
                ..
            } => {
                *sent = Some(now);
                vec![Action::GbaDown(Btn::Select)]
            }
            _ => Vec::new(),
        }
    }

    fn select_up(&mut self, now: Millis) -> Vec<Action> {
        let Select::Held { chorded, sent, .. } = std::mem::take(&mut self.select) else {
            return Vec::new();
        };
        match (sent, chorded) {
            (Some(at), _) if now.saturating_sub(at) >= SELECT_TAP_MS => {
                vec![Action::GbaUp(Btn::Select)]
            }
            (Some(at), _) => {
                self.select = Select::ReleaseDue(at + SELECT_TAP_MS);
                Vec::new()
            }
            (None, true) => Vec::new(),
            (None, false) => {
                self.select = Select::ReleaseDue(now + SELECT_TAP_MS);
                vec![Action::GbaDown(Btn::Select)]
            }
        }
    }

    fn menu_down(&mut self, now: Millis) -> Vec<Action> {
        if self.chording(now) {
            self.mark_chorded();
            self.menu_down_at = None;
            self.menu_last_tap = None;
            return vec![Action::GameMenu];
        }
        if let Some(tap) = self.menu_last_tap {
            if now.saturating_sub(tap) <= MENU_DOUBLE_TAP_MS {
                self.menu_last_tap = None;
                self.menu_down_at = None;
                return vec![Action::Polaroids];
            }
        }
        self.menu_down_at = Some(now);
        self.menu_eject_fired = false;
        Vec::new()
    }

    fn menu_up(&mut self, now: Millis) -> Vec<Action> {
        let Some(d) = self.menu_down_at.take() else {
            return Vec::new();
        };
        let ejected = self.menu_eject_fired;
        self.menu_eject_fired = false;
        let tapped = !ejected && now.saturating_sub(d) < MENU_TAP_MS;
        self.menu_last_tap = tapped.then_some(now);
        match tapped {
            true => vec![Action::QuickMenu],
            false => Vec::new(),
        }
    }

    fn power_up(&mut self) -> Vec<Action> {
        // A wake-key release can arrive after the input state was reset.
        if self.power_down_at.is_none() { return Vec::new(); }
        let held = self.power_hold_fired;
        self.power_down_at = None;
        self.power_hold_fired = false;
        vec![if held {
            Action::PowerOff
        } else {
            Action::PowerTap
        }]
    }

    fn volume_press(&mut self, b: Btn, now: Millis) -> Vec<Action> {
        let (mine, other, action) = match b {
            Btn::VolUp => (&mut self.vol_up_at, self.vol_down_at, Action::VolumeUp),
            _ => (&mut self.vol_down_at, self.vol_up_at, Action::VolumeDown),
        };
        *mine = Some(now);
        match b {
            Btn::VolUp => self.vol_up_ramp = None,
            _ => self.vol_down_ramp = None,
        }
        let mut out = vec![action];
        let paired = other.is_some_and(|t| now.abs_diff(t) <= MUTE_CHORD_MS);
        if paired && !self.mute_fired {
            self.mute_fired = true;
            out.push(Action::MuteToggle);
        }
        out
    }

    fn volume_release(&mut self, b: Btn) -> Vec<Action> {
        match b {
            Btn::VolUp => (self.vol_up_at, self.vol_up_ramp) = (None, None),
            _ => (self.vol_down_at, self.vol_down_ramp) = (None, None),
        }
        if self.vol_up_at.is_none() && self.vol_down_at.is_none() {
            self.mute_fired = false;
        }
        Vec::new()
    }

    fn rewind_start(&mut self) -> Vec<Action> {
        if self.rewinding {
            return Vec::new();
        }
        let mut out = Vec::new();
        out.extend(self.ff_clear());
        self.rewinding = true;
        out.push(Action::RewindStart);
        out
    }

    fn rewind_stop(&mut self) -> Vec<Action> {
        if !self.rewinding {
            return Vec::new();
        }
        self.rewinding = false;
        vec![Action::RewindStop]
    }

    fn ff_down(&mut self, now: Millis) -> Vec<Action> {
        if self.rewinding {
            self.r2_refused = true;
            return Vec::new();
        }
        if self.ff_latched {
            self.ff_latching_press = false;
        } else {
            let double = self
                .r2_last_release
                .is_some_and(|rel| now.saturating_sub(rel) <= FF_DOUBLE_TAP_MS);
            self.ff_latched = double;
            self.ff_latching_press = double;
        }
        if self.ff_on {
            return Vec::new();
        }
        self.ff_on = true;
        vec![Action::FfStart]
    }

    fn ff_up(&mut self, now: Millis) -> Vec<Action> {
        if std::mem::take(&mut self.r2_refused) {
            return Vec::new();
        }
        self.r2_last_release = Some(now);
        if self.ff_latching_press {
            self.ff_latching_press = false;
            return Vec::new();
        }
        self.ff_clear()
    }

    fn ff_clear(&mut self) -> Vec<Action> {
        self.ff_latched = false;
        self.ff_latching_press = false;
        if !self.ff_on {
            return Vec::new();
        }
        self.ff_on = false;
        vec![Action::FfStop]
    }
}

/// The held-bits HOME+L2 and HOME+R2 use while the triggers belong to the game, past the bits the
/// shortcuts use (one each, by their place in the list): held, as the triggers alone are.
const TRIGGER_BITS: u32 = (1 << 20) | (1 << 21);

fn trigger_bit(b: Btn) -> Option<u32> {
    match b {
        Btn::L2 => Some(1 << 20),
        Btn::R2 => Some(1 << 21),
        _ => None,
    }
}

/// The buttons a shortcut can be on.
pub const BINDABLE: [Btn; 12] = [
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
];

/// The button held with a shortcut's own: SELECT, or the Brick's HOME.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Modifier {
    Select,
    Home,
}

impl Modifier {
    pub fn name(self) -> &'static str {
        match self {
            Modifier::Select => "SELECT",
            Modifier::Home => "HOME",
        }
    }
}

/// One shortcut: the action it does, and the combo that does it (none when it is turned off).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Shortcut {
    pub action: Action,
    pub combo: Option<(Modifier, Btn)>,
}

/// How each shortcut is named in `Config/shortcuts.ini`, in the order of `Shortcuts::standard`.
pub const SHORTCUT_KEYS: [&str; 16] = [
    "brightness_up",
    "brightness_down",
    "blue_light_down",
    "blue_light_up",
    "load_state",
    "save_state",
    "colour_correction",
    "palette_prev",
    "palette_next",
    "shader_next",
    "shader_prev",
    "shader_next_alt",
    "led_toggle",
    "overlay_prev",
    "overlay_next",
    "picture_toggle",
];

/// The shortcuts, in a fixed order, each with the combo that does it now.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Shortcuts {
    pub list: Vec<Shortcut>,
}

impl Default for Shortcuts {
    fn default() -> Self {
        Shortcuts::standard(false)
    }
}

impl Shortcuts {
    /// The shortcuts as they come. With `home_chords` (the Brick) HOME shortcuts and the
    /// picture one are there too.
    pub fn standard(home_chords: bool) -> Shortcuts {
        let (select, home) = (Modifier::Select, Modifier::Home);
        let on = |modifier, button, action| Shortcut {
            action,
            combo: Some((modifier, button)),
        };
        let mut list = vec![
            on(select, Btn::Up, Action::BrightnessUp),
            on(select, Btn::Down, Action::BrightnessDown),
            on(select, Btn::Left, Action::BlueLightDown),
            on(select, Btn::Right, Action::BlueLightUp),
            on(select, Btn::L1, Action::LoadState),
            on(select, Btn::R1, Action::SaveState),
            on(select, Btn::Y, Action::ColourCorrectionToggle),
            on(select, Btn::L2, Action::PalettePrev),
            on(select, Btn::R2, Action::PaletteNext),
            on(select, Btn::A, Action::ShaderNext),
            on(select, Btn::B, Action::ShaderPrev),
        ];
        if home_chords {
            list.extend([
                on(home, Btn::A, Action::ShaderNext),
                on(home, Btn::B, Action::LedToggle),
                on(home, Btn::L1, Action::OverlayPrev),
                on(home, Btn::R1, Action::OverlayNext),
                on(select, Btn::X, Action::PictureToggle),
            ]);
        }
        Shortcuts { list }
    }

    /// What `modifier` held with `button` does: the shortcut's place in the list, and its action.
    pub fn find(&self, modifier: Modifier, button: Btn) -> Option<(usize, Action)> {
        self.list
            .iter()
            .enumerate()
            .find_map(|(at, s)| (s.combo == Some((modifier, button))).then_some((at, s.action)))
    }

    /// The standard shortcuts with those a `shortcuts.ini` names changed: `save_state=HOME+X`,
    /// or `save_state=Off`.
    pub fn parse(text: &str, home_chords: bool) -> Shortcuts {
        let mut shortcuts = Shortcuts::standard(home_chords);
        for line in text.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let Some(at) = SHORTCUT_KEYS.iter().position(|k| k.eq_ignore_ascii_case(key.trim())) else {
                continue;
            };
            let (Some(shortcut), Some(combo)) = (shortcuts.list.get_mut(at), parse_combo(value)) else {
                continue;
            };
            shortcut.combo = combo;
        }
        shortcuts
    }

    /// The text of a `shortcuts.ini`, a line to a shortcut.
    pub fn to_text(&self) -> String {
        let mut text = String::new();
        for (key, shortcut) in SHORTCUT_KEYS.iter().zip(&self.list) {
            let value = match shortcut.combo {
                Some((modifier, button)) => format!("{}+{:?}", modifier.name(), button),
                None => "Off".to_string(),
            };
            text.push_str(&format!("{key}={value}\n"));
        }
        text
    }
}

/// A combo as written: `SELECT+R1`, `HOME+A`, or `Off` for none.
fn parse_combo(value: &str) -> Option<Option<(Modifier, Btn)>> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("off") || value.eq_ignore_ascii_case("none") {
        return Some(None);
    }
    let (modifier, button) = value.split_once('+')?;
    let modifier = match modifier.trim().to_ascii_lowercase().as_str() {
        "select" => Modifier::Select,
        "home" => Modifier::Home,
        _ => return None,
    };
    let button = BINDABLE
        .into_iter()
        .find(|b| format!("{b:?}").eq_ignore_ascii_case(button.trim()))?;
    Some(Some((modifier, button)))
}

#[cfg(test)]
mod shortcut_tests {
    use super::*;

    #[test]
    fn the_standard_shortcuts_are_the_ones_the_brick_always_had() {
        let s = Shortcuts::standard(true);
        assert_eq!(s.find(Modifier::Select, Btn::R1).map(|f| f.1), Some(Action::SaveState));
        assert_eq!(s.find(Modifier::Home, Btn::B).map(|f| f.1), Some(Action::LedToggle));
        assert_eq!(s.find(Modifier::Select, Btn::X).map(|f| f.1), Some(Action::PictureToggle));
        assert_eq!(Shortcuts::standard(false).list.len(), 11);
    }

    #[test]
    fn a_shortcut_can_be_moved_and_turned_off() {
        let s = Shortcuts::parse("save_state=HOME+X\nload_state=off\n", true);
        assert_eq!(s.find(Modifier::Home, Btn::X).map(|f| f.1), Some(Action::SaveState));
        assert_eq!(s.find(Modifier::Select, Btn::L1), None);
        assert_eq!(Shortcuts::parse(&s.to_text(), true), s);
    }

    #[test]
    fn capture_reports_the_next_combo_instead_of_doing_it() {
        let mut g = Gestures::brick();
        g.set_capture(true);
        assert!(g.feed(RawEvent::Down(Btn::Select), 0).is_empty());
        assert_eq!(
            g.feed(RawEvent::Down(Btn::R1), 100),
            vec![Action::Capture(Modifier::Select, Btn::R1)]
        );
        g.feed(RawEvent::Up(Btn::R1), 150);
        assert_eq!(g.feed(RawEvent::Down(Btn::R1), 200), vec![Action::SaveState]);
    }
}
