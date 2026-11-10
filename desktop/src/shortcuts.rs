use std::collections::HashSet;

/// Parses one key chord into Windows virtual-key codes in press order.
/// The caller releases keys in reverse order. `None` is an explicit no-op.
pub fn parse_shortcut(text: &str) -> Result<Vec<u16>, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Choose a key, enter a shortcut such as Ctrl+F5, or use None.".into());
    }
    if text.eq_ignore_ascii_case("none") {
        return Ok(Vec::new());
    }
    if text.len() > 100 {
        return Err("A keyboard shortcut must be 100 characters or fewer.".into());
    }

    let mut modifiers = HashSet::new();
    let mut main_key = None;
    for part in text.split('+').map(str::trim) {
        let upper = part.to_ascii_uppercase();
        let modifier = match upper.as_str() {
            "CTRL" | "CONTROL" => Some((0x11, "Ctrl")),
            "ALT" => Some((0x12, "Alt")),
            "SHIFT" => Some((0x10, "Shift")),
            "WIN" | "WINDOWS" | "META" => Some((0x5B, "Win")),
            _ => None,
        };
        if let Some((key, name)) = modifier {
            if !modifiers.insert(key) {
                return Err(format!("{name} appears more than once."));
            }
            continue;
        }

        let key = main_virtual_key(&upper).ok_or_else(|| {
            format!("Unknown key '{part}'. Use letters, digits, F1 to F24, or navigation keys.")
        })?;
        if main_key.replace(key).is_some() {
            return Err(
                "Use one main key with optional Ctrl, Alt, Shift, or Win modifiers.".into(),
            );
        }
    }

    let main_key = main_key.ok_or("Add a main key after the modifier, such as Ctrl+F5.")?;
    let mut keys: Vec<u16> = [0x11, 0x12, 0x10, 0x5B]
        .into_iter()
        .filter(|key| modifiers.contains(key))
        .collect();
    keys.push(main_key);
    Ok(keys)
}

fn main_virtual_key(key: &str) -> Option<u16> {
    let named = match key {
        "LEFT" | "ARROWLEFT" => 0x25,
        "UP" | "ARROWUP" => 0x26,
        "RIGHT" | "ARROWRIGHT" => 0x27,
        "DOWN" | "ARROWDOWN" => 0x28,
        "ESCAPE" | "ESC" => 0x1B,
        "SPACE" | "SPACEBAR" => 0x20,
        "ENTER" | "RETURN" => 0x0D,
        "TAB" => 0x09,
        "BACKSPACE" => 0x08,
        "HOME" => 0x24,
        "END" => 0x23,
        "PAGEUP" | "PGUP" => 0x21,
        "PAGEDOWN" | "PGDOWN" | "PGDN" => 0x22,
        "INSERT" | "INS" => 0x2D,
        "DELETE" | "DEL" => 0x2E,
        _ => 0,
    };
    if named != 0 {
        return Some(named);
    }
    let bytes = key.as_bytes();
    if bytes.len() == 1 && bytes[0].is_ascii_alphanumeric() {
        return Some(bytes[0] as u16);
    }
    if let Some(number) = key
        .strip_prefix('F')
        .and_then(|number| number.parse::<u16>().ok())
    {
        if (1..=24).contains(&number) && key == format!("F{number}") {
            return Some(0x70 + number - 1);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::parse_shortcut;

    #[test]
    fn parses_chords_case_insensitively_in_consistent_press_order() {
        assert_eq!(
            parse_shortcut(" shift + control + l ").unwrap(),
            vec![0x11, 0x10, 0x4C]
        );
        assert_eq!(
            parse_shortcut("Win+Alt+F24").unwrap(),
            vec![0x12, 0x5B, 0x87]
        );
        assert_eq!(parse_shortcut("Ctrl+0").unwrap(), vec![0x11, 0x30]);
    }

    #[test]
    fn accepts_navigation_aliases_and_explicit_no_op() {
        assert_eq!(parse_shortcut("ArrowRight").unwrap(), vec![0x27]);
        assert_eq!(parse_shortcut("Esc").unwrap(), vec![0x1B]);
        assert_eq!(parse_shortcut("PageDown").unwrap(), vec![0x22]);
        assert_eq!(parse_shortcut("Delete").unwrap(), vec![0x2E]);
        assert!(parse_shortcut(" none ").unwrap().is_empty());
    }

    #[test]
    fn rejects_ambiguous_or_incomplete_shortcuts() {
        for invalid in [
            "",
            " ",
            "Ctrl",
            "Ctrl+Shift",
            "Ctrl+Control+A",
            "A+B",
            "A+A",
            "Ctrl++A",
            "Alt+",
            "F0",
            "F25",
            "F01",
            "None+A",
            "MediaNext",
            "é",
        ] {
            assert!(parse_shortcut(invalid).is_err(), "accepted {invalid:?}");
        }
    }
}
