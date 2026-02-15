use encoding_rs::WINDOWS_1252;

/// Detect whether bytes are valid UTF-8 or need legacy encoding decoding.
/// Returns the decoded string.
///
/// Strategy: try UTF-8 first. If it fails, decode as Windows-1252 (superset of ISO-8859-1).
/// The key byte is 0xA5 which is ¥ (U+00A5) in ISO-8859-1/Windows-1252.
/// Note: despite the CLAUDE.md saying "Mac Roman", the actual encoding for 0xA5→¥
/// is ISO-8859-1, not Mac Roman (which maps 0xA5 to bullet •).
pub fn decode_log_bytes(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        Err(_) => {
            let (cow, _encoding, _had_errors) = WINDOWS_1252.decode(bytes);
            cow.into_owned()
        }
    }
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
}
