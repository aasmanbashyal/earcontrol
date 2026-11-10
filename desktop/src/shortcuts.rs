/// Parses one key chord into Windows virtual-key codes in press order.
/// The caller releases keys in reverse order. `None` is an explicit no-op.
pub fn parse_shortcut(text: &str) -> Result<Vec<u16>, String> {
    parse_chord(text).map(|(keys, length)| keys[..length].to_vec())
}

/// Validate without allocating a key vector that the caller would discard.
pub fn validate_shortcut(text: &str) -> Result<(), String> {
    parse_chord(text).map(|_| ())
}

fn parse_chord(text: &str) -> Result<([u16; 5], usize), String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("Choose a key, enter a shortcut such as Ctrl+F5, or use None.".into());
    }
    if text.eq_ignore_ascii_case("none") {
        return Ok(([0; 5], 0));
    }
    if text.len() > 100 {
        return Err("A keyboard shortcut must be 100 characters or fewer.".into());
    }

    // Shortcuts are bounded above. ASCII case folding preserves UTF-8 and
    // avoids allocating a new String for every part of each validated chord.
    let mut uppercase = [0; 100];
    for (out, byte) in uppercase.iter_mut().zip(text.bytes()) {
        *out = byte.to_ascii_uppercase();
    }
    let uppercase = std::str::from_utf8(&uppercase[..text.len()]).unwrap();
    if !uppercase.as_bytes().contains(&b'+') {
        if let Some(key) = main_virtual_key(uppercase) {
            return Ok(([key, 0, 0, 0, 0], 1));
        }
    }
    let mut modifiers = 0_u8;
    let mut main_key = None;
    for (index, upper) in uppercase.split('+').map(str::trim).enumerate() {
        let modifier = match upper {
            "CTRL" | "CONTROL" => Some((1, "Ctrl")),
            "ALT" => Some((2, "Alt")),
            "SHIFT" => Some((4, "Shift")),
            "WIN" | "WINDOWS" | "META" => Some((8, "Win")),
            _ => None,
        };
        if let Some((bit, name)) = modifier {
            if modifiers & bit != 0 {
                return Err(format!("{name} appears more than once."));
            }
            modifiers |= bit;
            continue;
        }

        let key = main_virtual_key(upper).ok_or_else(|| {
            let part = text.split('+').nth(index).unwrap().trim();
            format!("Unknown key '{part}'. Use letters, digits, F1 to F24, or navigation keys.")
        })?;
        if main_key.replace(key).is_some() {
            return Err(
                "Use one main key with optional Ctrl, Alt, Shift, or Win modifiers.".into(),
            );
        }
    }

    let main_key = main_key.ok_or("Add a main key after the modifier, such as Ctrl+F5.")?;
    let mut keys = [0; 5];
    let mut length = 0;
    for (bit, key) in [(1, 0x11), (2, 0x12), (4, 0x10), (8, 0x5B)] {
        if modifiers & bit != 0 {
            keys[length] = key;
            length += 1;
        }
    }
    keys[length] = main_key;
    Ok((keys, length + 1))
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
    let number = match bytes {
        [b'F', digit @ b'1'..=b'9'] => u16::from(digit - b'0'),
        [b'F', tens @ b'1'..=b'2', ones @ b'0'..=b'9'] => {
            u16::from(tens - b'0') * 10 + u16::from(ones - b'0')
        }
        _ => return None,
    };
    (number <= 24).then_some(0x70 + number - 1)
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
    fn all_function_keys_and_unicode_whitespace_preserve_chord_order() {
        for number in 1..=24 {
            assert_eq!(
                parse_shortcut(&format!("win+shift+alt+ctrl+f{number}")).unwrap(),
                vec![0x11, 0x12, 0x10, 0x5B, 0x70 + number - 1]
            );
        }
        assert_eq!(
            parse_shortcut("\u{2003}Ctrl\u{2003}+\u{2003}a").unwrap(),
            vec![0x11, 0x41]
        );
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
