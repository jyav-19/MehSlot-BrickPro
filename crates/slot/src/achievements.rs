//! A game's achievements kept on the card (`Config/Achievements/<console>-<game>.txt`), so the
//! list can be looked at with no connection: a `when=` line (seconds since 1970, 0 if the
//! clock was not set), then a line a achievement: id, points, unlocked (0 or 1), title and
//! description, tab-separated.

use std::path::{Path, PathBuf};

use slot_cheevos::Achievement;

pub struct Saved {
    pub items: Vec<Achievement>,
    pub saved_at: u64,
}

fn path(root: &Path, key: &str) -> PathBuf {
    let name: String = key
        .chars()
        .map(|c| if c.is_alphanumeric() || " -_.()".contains(c) { c } else { '_' })
        .collect();
    root.join("Config").join("Achievements").join(format!("{name}.txt"))
}

fn now() -> u64 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    // Before 2020 the clock was not set.
    if secs < 1_577_836_800 {
        0
    } else {
        secs
    }
}

fn clean(text: &str) -> String {
    text.replace(['\t', '\n', '\r'], " ")
}

pub fn encode(items: &[Achievement], saved_at: u64) -> String {
    let mut text = format!("when={saved_at}\n");
    for a in items {
        text.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            a.id,
            a.points,
            u8::from(a.unlocked),
            clean(&a.title),
            clean(&a.description)
        ));
    }
    text
}

pub fn decode(text: &str) -> Option<Saved> {
    let mut lines = text.lines();
    let saved_at = lines.next()?.strip_prefix("when=")?.trim().parse().ok()?;
    let mut items = Vec::new();
    for line in lines {
        let mut part = line.splitn(5, '\t');
        let (id, points, unlocked, title, description) =
            (part.next()?, part.next()?, part.next()?, part.next()?, part.next().unwrap_or(""));
        items.push(Achievement {
            id: id.parse().ok()?,
            title: title.to_string(),
            description: description.to_string(),
            points: points.parse().ok()?,
            unlocked: unlocked == "1",
        });
    }
    Some(Saved { items, saved_at })
}

/// Keeps `items` for the game called `key`, through a temporary file.
pub fn save(root: &Path, key: &str, items: &[Achievement]) {
    let path = path(root, key);
    let Some(dir) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let temp = path.with_extension("tmp");
    if std::fs::write(&temp, encode(items, now())).is_ok() {
        let _ = std::fs::rename(temp, path);
    }
}

pub fn load(root: &Path, key: &str) -> Option<Saved> {
    decode(&std::fs::read_to_string(path(root, key)).ok()?).filter(|s| !s.items.is_empty())
}

/// How long ago `saved_at` was, roughly: `5 min ago`, `3 h ago`, `2 days ago`.
pub fn ago(saved_at: u64) -> String {
    let now = now();
    if saved_at == 0 || now < saved_at {
        return "earlier".to_string();
    }
    let secs = now - saved_at;
    match secs {
        s if s < 90 => "a moment ago".to_string(),
        s if s < 3600 => format!("{} min ago", s / 60),
        s if s < 86_400 => format!("{} h ago", s / 3600),
        s => format!("{} days ago", s / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kept_list_reads_back_as_it_was_written() {
        let items = vec![
            Achievement { id: 7, title: "First Steps".into(), description: "Leave the\thouse".into(), points: 5, unlocked: true },
            Achievement { id: 9, title: "Hidden Chest".into(), description: "".into(), points: 10, unlocked: false },
        ];
        let back = decode(&encode(&items, 1_700_000_000)).expect("a list");
        assert_eq!(back.saved_at, 1_700_000_000);
        assert_eq!(back.items.len(), 2);
        assert_eq!(back.items[0].description, "Leave the house");
        assert!(back.items[0].unlocked && !back.items[1].unlocked);
        assert_eq!(back.items[1].points, 10);
    }

    #[test]
    fn a_damaged_file_is_not_a_list() {
        assert!(decode("nonsense").is_none());
        assert!(decode("when=1\nx\ty\n").is_none());
    }
}
