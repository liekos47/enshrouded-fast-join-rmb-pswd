//! `shroudkit/quickjoin.ini`: the remembered passwords, one line per server.
//!
//! ```ini
//! [passwords]
//! 0164a3...e1 = secret
//! ```
//!
//! The key is the hex of the Join screen's 0x22-byte block for the selected
//! server (README: Remember password); its layout is not decoded, so it is used as is.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

const HEADER: &str = "\
; Quick join: remembered server passwords.
;
; Type a server's password once in the Join box. From then on, press Join with
; the box empty and the saved password is used. Typing a password always wins
; and replaces the saved one. Delete a line to forget that server.
;
; Passwords are stored as plain text. Do not share this file.

[passwords]
";

static PASSWORDS: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());

fn path() -> Option<PathBuf> {
    Some(std::env::current_exe().ok()?.with_file_name("shroudkit").join("quickjoin.ini"))
}

/// Reads the file; returns how many passwords it holds.
pub fn load() -> usize {
    let Some(path) = path() else { return 0 };
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut map = PASSWORDS.lock().unwrap();
    map.clear();
    let mut in_section = false;
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_section = trimmed.eq_ignore_ascii_case("[passwords]");
            continue;
        }
        if !in_section || trimmed.is_empty() || trimmed.starts_with(';') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else { continue };
        // "key = value": one space after '=' is layout, the rest belongs to the password.
        let value = value.strip_prefix(' ').unwrap_or(value);
        let key = key.trim().to_ascii_lowercase();
        if !key.is_empty() && !value.is_empty() {
            map.insert(key, value.to_string());
        }
    }
    if !path.exists() {
        let _ = std::fs::write(&path, HEADER);
    }
    map.len()
}

pub fn get(key: &str) -> Option<String> {
    PASSWORDS.lock().unwrap().get(key).cloned()
}

/// Saves `password` for `key`; returns false if it was already saved.
pub fn remember(key: &str, password: &str) -> Result<bool, String> {
    let mut map = PASSWORDS.lock().unwrap();
    if map.get(key).is_some_and(|saved| saved == password) {
        return Ok(false);
    }
    map.insert(key.to_string(), password.to_string());
    let mut text = HEADER.to_string();
    for (key, password) in map.iter() {
        text.push_str(&format!("{key} = {password}\n"));
    }
    let path = path().ok_or("no exe path")?;
    std::fs::write(&path, text.replace('\n', "\r\n")).map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    Ok(true)
}
