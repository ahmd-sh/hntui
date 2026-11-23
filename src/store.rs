// What the app keeps in ~/.config/hntui: saved posts and view history (the
// same files and format as the TypeScript releases, so an upgrade keeps
// them) and state.json, things it remembers between runs. User settings
// live next to them in config.toml (see config.rs).
// HNTUI_CONFIG_DIR overrides the location (used for side-by-side testing).

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::path::PathBuf;

pub const HISTORY_CAP: usize = 1000;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedEntry {
    pub id: u64,
    #[serde(rename = "savedAt")]
    pub saved_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: u64,
    #[serde(rename = "viewedAt")]
    pub viewed_at: i64,
}

pub fn config_dir() -> PathBuf {
    if let Ok(d) = std::env::var("HNTUI_CONFIG_DIR") {
        return PathBuf::from(d);
    }
    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let new = home.join(".config/hntui");
    let old = home.join(".config/hackernuis");
    // the app was renamed: move old config over, once, best-effort
    if !new.exists() && old.exists() {
        let _ = std::fs::rename(&old, &new);
    }
    new
}

// entries that don't match the shape are dropped, not fatal
fn load<T: DeserializeOwned>(name: &str) -> Vec<T> {
    let Ok(raw) = std::fs::read(config_dir().join(name)) else { return vec![] };
    let Ok(serde_json::Value::Array(items)) = serde_json::from_slice(&raw) else { return vec![] };
    items.into_iter().filter_map(|v| serde_json::from_value(v).ok()).collect()
}

// write-then-rename, so a crash mid-write can't leave a truncated file
fn write_json<T: Serialize + ?Sized>(name: &str, value: &T) {
    let dir = config_dir();
    let _ = std::fs::create_dir_all(&dir);
    let Ok(json) = serde_json::to_string_pretty(value) else { return };
    let tmp = dir.join(format!(".{name}.tmp"));
    if std::fs::write(&tmp, json).is_ok() {
        let _ = std::fs::rename(&tmp, dir.join(name));
    }
}

fn persist<T: Serialize>(name: &str, entries: &[T]) {
    write_json(name, entries)
}

pub fn load_saved() -> Vec<SavedEntry> {
    load("saved.json")
}

pub fn persist_saved(e: &[SavedEntry]) {
    persist("saved.json", e)
}

pub fn load_history() -> Vec<HistoryEntry> {
    let mut h: Vec<HistoryEntry> = load("history.json");
    h.truncate(HISTORY_CAP);
    h
}

pub fn persist_history(e: &[HistoryEntry]) {
    persist("history.json", e)
}

/// Remembered between runs; written by the app, never by hand.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    /// the theme `t` last left the app on
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<crate::theme::ThemeName>,
    /// the newest version whose release notes have been shown
    #[serde(default, rename = "notesSeen", skip_serializing_if = "Option::is_none")]
    pub notes_seen: Option<String>,
}

pub fn load_state() -> State {
    std::fs::read(config_dir().join("state.json"))
        .ok()
        .and_then(|raw| serde_json::from_slice(&raw).ok())
        .unwrap_or_default()
}

pub fn persist_state(s: &State) {
    write_json("state.json", s)
}
