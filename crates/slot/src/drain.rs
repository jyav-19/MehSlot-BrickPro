//! How long the battery will last, from how fast it has been falling.
//!
//! The battery reports whole percents, so the rate is taken from the moments the percent
//! dropped: from the first drop in the last half hour to the latest one, at least two percents
//! apart. Until then there is no estimate. Charging, a jump up, a drop of two percents between
//! two readings (the Brick was asleep) or a long gap all start the count again.

/// How far back the drops that set the rate are looked at, and how long a gap between two
/// readings means the Brick was asleep.
const WINDOW_MS: u64 = 30 * 60 * 1000;
const GAP_MS: u64 = 60 * 1000;
/// The least the percent must have fallen, and the least time that took, for a rate.
const MIN_DROP: u8 = 2;
const MIN_SPAN_MS: u64 = 3 * 60 * 1000;
/// Longer than this is shown as "24h+".
const MAX_MINUTES: u32 = 24 * 60;

#[derive(Default, Debug)]
pub struct Drain {
    /// When each percent was reached, since the count began: (time, percent).
    steps: Vec<(u64, u8)>,
    /// The last reading: its time, percent and whether it was charging.
    seen: Option<(u64, u8, bool)>,
    /// Minutes left as of the time given.
    left: Option<(u32, u64)>,
}

impl Drain {
    /// A reading of the battery.
    pub fn sample(&mut self, now: u64, percent: u8, charging: bool) {
        if charging {
            self.restart();
            self.seen = Some((now, percent, true));
            return;
        }
        if let Some((at, was, was_charging)) = self.seen {
            if was_charging || now.saturating_sub(at) > GAP_MS || percent > was || was.saturating_sub(percent) >= MIN_DROP {
                self.restart();
            } else if percent < was {
                self.steps.push((now, percent));
                while self.steps.first().is_some_and(|(t, _)| now.saturating_sub(*t) > WINDOW_MS) {
                    self.steps.remove(0);
                }
                self.left = self.estimate().map(|minutes| (minutes, now));
            }
        }
        self.seen = Some((now, percent, false));
    }

    fn restart(&mut self) {
        self.steps.clear();
        self.left = None;
    }

    /// Minutes left at the latest drop's rate, if there is a rate yet.
    fn estimate(&self) -> Option<u32> {
        let (&(t0, p0), &(t1, p1)) = (self.steps.first()?, self.steps.last()?);
        let drop = p0.checked_sub(p1)?;
        let span = t1.checked_sub(t0)?;
        if drop < MIN_DROP || span < MIN_SPAN_MS {
            return None;
        }
        let per_percent_ms = span as f64 / f64::from(drop);
        Some((f64::from(p1) * per_percent_ms / 60_000.0).round() as u32)
    }

    /// What the footer says beside the percent: the time left, or `--` until there is an estimate.
    pub fn text(&self, now: u64) -> String {
        let Some((left, at)) = self.left else {
            return "--".to_string();
        };
        let gone = (now.saturating_sub(at) / 60_000) as u32;
        minutes_text(left.saturating_sub(gone))
    }
}

/// Minutes as the footer writes them, to the nearest five: `3h20m`, `45m`, `<5m`, `24h+`.
pub fn minutes_text(minutes: u32) -> String {
    if minutes > MAX_MINUTES {
        return "24h+".to_string();
    }
    let rounded = (minutes + 2) / 5 * 5;
    match rounded {
        0 => "<5m".to_string(),
        m if m >= 60 => format!("{}h{:02}m", m / 60, m % 60),
        m => format!("{m}m"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: u64 = 60_000;

    /// A reading every ten seconds, the percent falling one point every five minutes.
    fn run(from: u8, minutes: u64) -> Drain {
        let mut d = Drain::default();
        for tick in 0..=(minutes * 6) {
            let now = tick * 10_000;
            let percent = from - (now / (5 * MIN)) as u8;
            d.sample(now, percent, false);
        }
        d
    }

    #[test]
    fn five_minutes_a_percent_leaves_five_minutes_for_every_percent() {
        let d = run(80, 15); // 77% at fifteen minutes
        assert_eq!(d.text(15 * MIN), "6h25m");
    }

    #[test]
    fn there_is_no_estimate_before_two_percents_have_gone() {
        assert_eq!(run(80, 10).text(10 * MIN), "--");
    }

    #[test]
    fn the_time_ticks_down_between_drops() {
        let d = run(80, 15);
        assert_eq!(d.text(15 * MIN + 2 * MIN), "6h25m");
        assert_eq!(d.text(15 * MIN + 10 * MIN), "6h15m");
    }

    #[test]
    fn charging_and_sleep_start_the_count_again() {
        let mut d = run(80, 15);
        d.sample(16 * MIN, 77, true);
        assert_eq!(d.text(16 * MIN), "--");
        let mut d = run(80, 15);
        d.sample(15 * MIN + 10_000 * 7, 74, false); // three percents in seventy seconds: asleep
        assert_eq!(d.text(16 * MIN), "--");
    }

    #[test]
    fn minutes_are_written_to_the_nearest_five() {
        assert_eq!(minutes_text(200), "3h20m");
        assert_eq!(minutes_text(47), "45m");
        assert_eq!(minutes_text(2), "<5m");
        assert_eq!(minutes_text(24 * 60 + 1), "24h+");
    }
}
