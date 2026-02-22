use encoding_rs::WINDOWS_1252;

/// Patch bytes that are undefined in Windows-1252 but valid in Mac Roman.
///
/// Clan Lord is a classic Mac game, so log files may contain Mac Roman byte values
/// for accented characters (e.g., 0x8F = è in "Violène"). These 5 bytes are undefined
/// in W1252, so we remap them to the W1252 byte that produces the same Unicode character.
fn patch_mac_roman_bytes(line: &[u8]) -> Vec<u8> {
    line.iter()
        .map(|&b| match b {
            0x81 => 0xC5, // Å
            0x8D => 0xE7, // ç
            0x8F => 0xE8, // è
            0x90 => 0xEA, // ê
            0x9D => 0xF9, // ù
            _ => b,
        })
        .collect()
}

/// Decode log file bytes, handling mixed encoding (some lines UTF-8, some Windows-1252).
///
/// Strategy:
/// 1. Fast path: if the entire file is valid UTF-8, use it directly.
/// 2. Mixed encoding: decode line-by-line — try UTF-8 first for each line, fall back to W1252
///    with Mac Roman patching for the 5 bytes that W1252 leaves undefined.
pub fn decode_log_bytes(bytes: &[u8]) -> String {
    // Fast path: if entire file is valid UTF-8, use it directly
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }

    // Mixed encoding: decode line-by-line
    let mut result = String::new();
    for line in bytes.split(|&b| b == b'\n') {
        if !result.is_empty() {
            result.push('\n');
        }
        match std::str::from_utf8(line) {
            Ok(s) => result.push_str(s),
            Err(_) => {
                let patched = patch_mac_roman_bytes(line);
                let (cow, _, _) = WINDOWS_1252.decode(&patched);
                result.push_str(&cow);
            }
        }
    }
    result
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

    #[test]
    fn test_mixed_encoding_utf8_and_w1252() {
        // Simulate a file with W1252 ¥ trainer lines AND UTF-8 accented creature names
        let mut bytes = Vec::new();
        // Line 1: W1252 trainer message (0xA5 = ¥)
        bytes.extend_from_slice(b"1/1/24 1:00:00p ");
        bytes.push(0xA5);
        bytes.extend_from_slice(b"Your combat ability improves.");
        bytes.push(b'\n');
        // Line 2: UTF-8 kill message with è (C3 A8)
        bytes.extend_from_slice("1/1/24 1:01:00p You slaughtered a Violène Arachne.\n".as_bytes());

        let result = decode_log_bytes(&bytes);
        // The trainer line should decode 0xA5 as ¥
        assert!(result.contains("¥Your combat ability improves."), "Trainer line wrong: {}", result);
        // The kill line should preserve è (not produce mojibake Ã¨)
        assert!(result.contains("Violène"), "Accented name should be preserved, got: {}", result);
        assert!(!result.contains("ViolÃ"), "Should not have mojibake, got: {}", result);
    }

    #[test]
    fn test_mac_roman_0x8f_becomes_e_grave() {
        // Clan Lord logs may contain Mac Roman byte 0x8F for è (e.g., "Violène Arachne").
        // 0x8F is undefined in W1252, so without patching it renders as a control char (□).
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"You slaughtered a Viol");
        bytes.push(0x8F); // Mac Roman è
        bytes.extend_from_slice(b"ne Arachne.");
        let result = decode_log_bytes(&bytes);
        assert!(result.contains("Violène Arachne"), "Expected è, got: {}", result);
    }

    #[test]
    fn test_mac_roman_bytes_with_yen_prefix() {
        // A file with both 0xA5 (¥ in W1252) and 0x8F (è in Mac Roman)
        let mut bytes = Vec::new();
        // Line 1: trainer message with ¥
        bytes.push(0xA5);
        bytes.extend_from_slice(b"Your combat ability improves.\n");
        // Line 2: kill with Mac Roman è
        bytes.extend_from_slice(b"You slaughtered a Viol");
        bytes.push(0x8F);
        bytes.extend_from_slice(b"ne Arachne.");
        let result = decode_log_bytes(&bytes);
        assert!(result.contains("¥Your combat ability"), "¥ prefix broken: {}", result);
        assert!(result.contains("Violène Arachne"), "Mac Roman è broken: {}", result);
    }
}
