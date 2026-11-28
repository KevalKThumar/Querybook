//! Small, stable encodings used on both sides: a hash for file names and
//! percent-encoding for the links Querybook opens in the browser.

/// FNV-1a: a hash that stays the same across Rust versions, for file names.
pub fn fnv1a64(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3))
}

/// Percent-encode everything but RFC 3986's unreserved characters, for a URL
/// query value (spaces become `%20`, newlines `%0A`).
pub fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnv_is_stable() {
        // The change-history file names depend on these values never changing.
        assert_eq!(fnv1a64(""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64("a"), 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn percent_encoding() {
        assert_eq!(percent_encode("a b&c=d/é\n"), "a%20b%26c%3Dd%2F%C3%A9%0A");
        assert_eq!(percent_encode("Az09-_.~"), "Az09-_.~");
    }
}
