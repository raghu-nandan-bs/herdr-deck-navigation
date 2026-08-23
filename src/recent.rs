//! A tiny most-recent-first list of pane ids, persisted between runs so `r` can
//! alt-tab you back to the pane you were in before this one.

use std::path::{Path, PathBuf};

const CAP: usize = 16;

/// `$XDG_STATE_HOME/herdr-deck/recent.json`, else `~/.local/state/herdr-deck/recent.json`.
pub fn path() -> Option<PathBuf> {
    let base = match std::env::var("XDG_STATE_HOME") {
        Ok(x) if !x.is_empty() => PathBuf::from(x),
        _ => PathBuf::from(std::env::var("HOME").ok()?).join(".local/state"),
    };
    Some(base.join("herdr-deck/recent.json"))
}

/// Most-recent-first pane ids. Never fails: a missing or corrupt store is empty.
pub fn load() -> Vec<String> {
    path().map(|p| read_from(&p)).unwrap_or_default()
}

/// Record `id` as the newest entry. Best-effort — navigation must not fail because
/// the state file is unwritable.
pub fn record(id: &str) {
    let Some(p) = path() else { return };
    let mut list = read_from(&p);
    promote(&mut list, id, CAP);
    write_to(&p, &list);
}

fn promote(list: &mut Vec<String>, id: &str, cap: usize) {
    list.retain(|x| x != id);
    list.insert(0, id.to_string());
    list.truncate(cap);
}

fn read_from(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_to(path: &Path, list: &[String]) {
    if let Some(dir) = path.parent() {
        if std::fs::create_dir_all(dir).is_err() {
            return;
        }
    }
    if let Ok(json) = serde_json::to_string(list) {
        let _ = std::fs::write(path, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promote_moves_an_id_to_the_front_without_duplicating_it() {
        let mut v = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        promote(&mut v, "c", 8);
        assert_eq!(v, ["c", "a", "b"]);
        promote(&mut v, "c", 8);
        assert_eq!(v, ["c", "a", "b"]);
    }

    #[test]
    fn promote_caps_the_list_at_the_limit() {
        let mut v: Vec<String> = (0..5).map(|i| i.to_string()).collect();
        promote(&mut v, "new", 3);
        assert_eq!(v, ["new", "0", "1"]);
    }

    #[test]
    fn round_trips_through_a_file_and_treats_a_missing_one_as_empty() {
        let dir = std::env::temp_dir().join("herdr-deck-recent-test");
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join("recent.json");
        assert!(read_from(&file).is_empty(), "missing file reads as empty");
        write_to(&file, &["w1:p1".to_string(), "w2:p3".to_string()]);
        assert_eq!(read_from(&file), ["w1:p1", "w2:p3"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupt_contents_read_as_empty_rather_than_failing() {
        let dir = std::env::temp_dir().join("herdr-deck-recent-corrupt");
        let _ = std::fs::remove_dir_all(&dir);
        let file = dir.join("recent.json");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&file, "not json").unwrap();
        assert!(read_from(&file).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
