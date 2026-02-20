use encoding_rs::WINDOWS_1252;

/// Detect whether bytes are valid UTF-8 or need legacy encoding decoding.
/// Returns the decoded string.
///
/// Strategy: try UTF-8 first. If it fails, decode as Windows-1252 (superset of ISO-8859-1).
/// The key byte is 0xA5 which is ¥ (U+00A5) in ISO-8859-1/Windows-1252.
/// Note: despite the CLAUDE.md saying "Mac Roman", the actual encoding for 0xA5→¥
/// is ISO-8859-1, not Mac Roman (which maps 0xA5 to bullet •).
pub fn decode_log_bytes(bytes: &[u8]) -> String {
    // Check if the file contains a Windows-1252 ¥ marker (0xA5 that isn't part of
    // a valid UTF-8 sequence). If found, decode the entire file as Windows-1252.
    // Otherwise, use lossy UTF-8 decoding to handle files that are mostly UTF-8
    // but may have truncated bytes at the end (e.g., file cut mid-character).
    if has_windows1252_yen(bytes) {
        let (cow, _encoding, _had_errors) = WINDOWS_1252.decode(bytes);
        cow.into_owned()
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

/// Check if bytes contain a 0xA5 byte that is NOT part of a valid UTF-8 sequence.
/// In Windows-1252, 0xA5 = ¥. In UTF-8, 0xA5 is only valid as part of the
/// 2-byte sequence C2 A5 (which also encodes ¥).
fn has_windows1252_yen(bytes: &[u8]) -> bool {
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b < 0x80 {
            i += 1;
        } else if b == 0xA5 {
            // 0xA5 as a standalone byte (not preceded by 0xC2) indicates Windows-1252
            if i == 0 || bytes[i - 1] != 0xC2 {
                return true;
            }
            i += 1;
        } else if b & 0xE0 == 0xC0 {
            i += 2; // 2-byte UTF-8
        } else if b & 0xF0 == 0xE0 {
            i += 3; // 3-byte UTF-8
        } else if b & 0xF8 == 0xF0 {
            i += 4; // 4-byte UTF-8
        } else {
            i += 1; // Invalid byte, skip
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_utf8_passthrough() {
        let input = "Hello, world! ¥You feel tougher.";
        let result = decode_log_bytes(input.as_bytes());
        assert_eq!(result, input);
    }

    #[test]
    fn test_0xa5_becomes_yen_sign() {
        // 0xA5 in ISO-8859-1/Windows-1252 = ¥ (U+00A5)
        let bytes: Vec<u8> = vec![0xA5, b'H', b'e', b'l', b'l', b'o'];
        let result = decode_log_bytes(&bytes);
        assert!(result.starts_with('¥'), "Expected yen prefix, got: {}", result);
        assert_eq!(result, "¥Hello");
    }

    #[test]
    fn test_pure_ascii() {
        let input = b"You slaughtered a Rat.";
        let result = decode_log_bytes(input);
        assert_eq!(result, "You slaughtered a Rat.");
    }

    #[test]
    fn test_trainer_message_with_0xa5() {
        let mut bytes = vec![0xA5];
        bytes.extend_from_slice(b"Your combat ability improves.");
        let result = decode_log_bytes(&bytes);
        assert_eq!(result, "¥Your combat ability improves.");
    }

    #[test]
    fn test_truncated_utf8_preserves_bullet_prefix() {
        // Simulate a file that is mostly valid UTF-8 with • (e2 80 a2) prefixes,
        // but truncated mid-character at the end (e.g., file cut off at e2 80).
        let mut bytes = Vec::new();
        // A valid line with bullet prefix
        bytes.extend_from_slice("1/1/25 1:00:00p \u{2022}You learn more.\r\n".as_bytes());
        // Truncated line ending with incomplete UTF-8 (e2 80 without final a2)
        bytes.extend_from_slice(b"1/1/25 1:01:00p ");
        bytes.extend_from_slice(&[0xe2, 0x80]); // incomplete •

        let result = decode_log_bytes(&bytes);
        // The first line's • should be preserved, not mangled to â€¢
        assert!(result.contains('\u{2022}'), "Bullet should be preserved, got: {}", result);
    }
}
