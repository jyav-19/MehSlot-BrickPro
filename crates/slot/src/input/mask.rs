use slot_input::{Action, Btn};
use slot_retro::ButtonMask;

#[derive(Default)]
pub struct Pad {
    mask: u16,
}

impl Pad {
    pub fn apply(&mut self, action: Action) {
        match action {
            Action::GbaDown(b) => {
                if let Some(bit) = bit(b) {
                    self.mask |= bit;
                }
            }
            Action::GbaUp(b) => {
                if let Some(bit) = bit(b) {
                    self.mask &= !bit;
                }
            }
            _ => {}
        }
    }

    pub fn mask(&self) -> ButtonMask {
        ButtonMask(self.mask)
    }

    pub fn clear(&mut self) {
        self.mask = 0;
    }

    /// The mask the game is handed: each button held, sent where `map` says, or as itself if
    /// it is not listed.
    pub fn mask_with(&self, map: &[(Btn, Option<Btn>)]) -> ButtonMask {
        if map.is_empty() {
            return self.mask();
        }
        let mut out = 0u16;
        for btn in super::controls::GAME_BUTTONS {
            let Some(held) = bit(btn) else {
                continue;
            };
            if self.mask & held == 0 {
                continue;
            }
            let sends = map
                .iter()
                .find(|(from, _)| *from == btn)
                .map_or(Some(btn), |(_, to)| *to);
            if let Some(sent) = sends.and_then(bit) {
                out |= sent;
            }
        }
        ButtonMask(out)
    }
}

fn bit(btn: Btn) -> Option<u16> {
    Some(match btn {
        Btn::Up => ButtonMask::UP,
        Btn::Down => ButtonMask::DOWN,
        Btn::Left => ButtonMask::LEFT,
        Btn::Right => ButtonMask::RIGHT,
        Btn::A => ButtonMask::A,
        Btn::B => ButtonMask::B,
        Btn::X => ButtonMask::X,
        Btn::Y => ButtonMask::Y,
        Btn::L1 => ButtonMask::L,
        Btn::R1 => ButtonMask::R,
        // Only a PlayStation game is handed its triggers (Gestures::set_triggers_to_game).
        Btn::L2 => ButtonMask::L2,
        Btn::R2 => ButtonMask::R2,
        Btn::Start => ButtonMask::START,
        Btn::Select => ButtonMask::SELECT,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_remapped_button_sends_what_it_is_told_and_two_can_share() {
        let mut pad = Pad::default();
        pad.apply(Action::GbaDown(Btn::X));
        pad.apply(Action::GbaDown(Btn::Y));
        let map = [(Btn::X, Some(Btn::Start)), (Btn::Y, Some(Btn::Start))];
        assert_eq!(pad.mask_with(&map).0, ButtonMask::START);
        pad.apply(Action::GbaUp(Btn::X));
        assert_eq!(pad.mask_with(&map).0, ButtonMask::START, "Y still holds Start");
        pad.apply(Action::GbaUp(Btn::Y));
        assert_eq!(pad.mask_with(&map).0, 0);
    }

    #[test]
    fn a_button_not_listed_sends_itself_and_none_sends_nothing() {
        let mut pad = Pad::default();
        pad.apply(Action::GbaDown(Btn::A));
        pad.apply(Action::GbaDown(Btn::B));
        assert_eq!(pad.mask_with(&[(Btn::B, None)]).0, ButtonMask::A);
    }
}
