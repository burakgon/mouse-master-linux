//! Key names used in configuration files: Linux input event code names such
//! as `KEY_LEFTCTRL` or `KEY_F5`. The `KEY_` prefix may be omitted.

use std::str::FromStr;

use evdev::KeyCode;

pub fn parse_key(name: &str) -> Option<KeyCode> {
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    let upper = name.to_ascii_uppercase();
    if let Ok(k) = KeyCode::from_str(&upper) {
        return Some(k);
    }
    let alias = match upper.as_str() {
        "CTRL" | "CONTROL" => "KEY_LEFTCTRL",
        "SHIFT" => "KEY_LEFTSHIFT",
        "ALT" => "KEY_LEFTALT",
        "SUPER" | "META" | "WIN" | "LOGO" => "KEY_LEFTMETA",
        "ALTGR" => "KEY_RIGHTALT",
        "RETURN" => "KEY_ENTER",
        "ESCAPE" => "KEY_ESC",
        "DEL" => "KEY_DELETE",
        "INS" => "KEY_INSERT",
        "PGUP" => "KEY_PAGEUP",
        "PGDN" => "KEY_PAGEDOWN",
        "PRINT" | "PRINTSCREEN" => "KEY_SYSRQ",
        "PLUS" => "KEY_EQUAL",
        _ => "",
    };
    if !alias.is_empty() {
        return KeyCode::from_str(alias).ok();
    }
    KeyCode::from_str(&format!("KEY_{upper}")).ok()
}

/// Parses a combination, rejecting unknown names.
pub fn parse_combo(keys: &[String]) -> Result<Vec<KeyCode>, String> {
    keys.iter().map(|k| parse_key(k).ok_or_else(|| format!("unknown key name {k:?}"))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(parse_key("KEY_LEFTCTRL"), Some(KeyCode::KEY_LEFTCTRL));
        assert_eq!(parse_key("a"), Some(KeyCode::KEY_A));
        assert_eq!(parse_key("F5"), Some(KeyCode::KEY_F5));
        assert_eq!(parse_key("ctrl"), Some(KeyCode::KEY_LEFTCTRL));
        assert_eq!(parse_key("volumeup"), Some(KeyCode::KEY_VOLUMEUP));
        assert_eq!(parse_key("nonsense"), None);
        assert!(parse_combo(&["KEY_LEFTCTRL".into(), "bogus".into()]).is_err());
    }
}
