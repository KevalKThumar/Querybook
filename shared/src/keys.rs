//! Keyboard-shortcut hints. The UI writes them in Mac notation ("⇧⌘C");
//! elsewhere they read the Windows / Linux way ("Ctrl+Shift+C").

/// Rewrite every Mac shortcut in `text` (a run of ⌘ ⌃ ⇧ ⌥ plus the key that
/// follows) for the current platform. On a Mac the text is returned as is.
pub fn shortcut_text(text: &str, mac: bool) -> String {
    if mac {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 8);
    let mut i = 0;
    while i < chars.len() {
        if !is_modifier(chars[i]) {
            match chars[i] {
                // A bare key: the grid takes Delete or Backspace; Windows calls ↵ Enter.
                '⌫' => out.push_str("Delete"),
                '↵' => out.push_str("Enter"),
                c => out.push(c),
            }
            i += 1;
            continue;
        }
        let (mut ctrl, mut shift, mut alt) = (false, false, false);
        while let Some(&c) = chars.get(i).filter(|c| is_modifier(**c)) {
            match c {
                '⌘' | '⌃' => ctrl = true,
                '⇧' => shift = true,
                _ => alt = true,
            }
            i += 1;
        }
        let key = match chars.get(i) {
            Some('↵') => {
                i += 1;
                "Enter".to_string()
            }
            Some('↑') => {
                i += 1;
                "Up".to_string()
            }
            Some('↓') => {
                i += 1;
                "Down".to_string()
            }
            Some('⌫') => {
                i += 1;
                "Backspace".to_string()
            }
            _ => {
                let start = i;
                while chars.get(i).is_some_and(char::is_ascii_alphanumeric) {
                    i += 1;
                }
                chars[start..i].iter().collect()
            }
        };
        let parts = [(ctrl, "Ctrl"), (shift, "Shift"), (alt, "Alt")];
        let mut combo: Vec<&str> = parts.iter().filter(|(on, _)| *on).map(|(_, name)| *name).collect();
        if !key.is_empty() {
            combo.push(&key);
        }
        out.push_str(&combo.join("+"));
    }
    out
}

fn is_modifier(c: char) -> bool {
    matches!(c, '⌘' | '⌃' | '⇧' | '⌥')
}

#[cfg(test)]
mod tests {
    use super::shortcut_text;

    fn win(s: &str) -> String {
        shortcut_text(s, false)
    }

    #[test]
    fn single_and_combined_modifiers() {
        assert_eq!(win("⌘O"), "Ctrl+O");
        assert_eq!(win("⇧⌘C"), "Ctrl+Shift+C");
        assert_eq!(win("⇧⌥F"), "Shift+Alt+F");
        assert_eq!(win("⌃Space"), "Ctrl+Space");
    }

    #[test]
    fn symbol_keys_inside_sentences() {
        assert_eq!(win("Run the statement under the cursor (⇧↵ or ⌘↵)"), "Run the statement under the cursor (Shift+Enter or Ctrl+Enter)");
        assert_eq!(win("⇧↵ run current · ⌃Space suggest"), "Shift+Enter run current · Ctrl+Space suggest");
        assert_eq!(win("Select all rows (⌘A)"), "Select all rows (Ctrl+A)");
        assert_eq!(win("⌘↑ / ⌘↓"), "Ctrl+Up / Ctrl+Down");
    }

    #[test]
    fn bare_keys() {
        assert_eq!(win("Delete the selected rows (⌫)"), "Delete the selected rows (Delete)");
        assert_eq!(win("↵"), "Enter");
    }

    #[test]
    fn mac_and_plain_text_unchanged() {
        assert_eq!(shortcut_text("Copy (⇧⌘C)", true), "Copy (⇧⌘C)");
        assert_eq!(win("Run a .sql script"), "Run a .sql script");
        assert_eq!(win(""), "");
    }
}
