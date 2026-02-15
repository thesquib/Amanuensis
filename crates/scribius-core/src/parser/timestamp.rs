use chrono::NaiveDateTime;

/// Parse a Clan Lord log timestamp from the beginning of a line.
/// Format: `M/D/YY H:MM:SSa/p` (12-hour, no leading zeros on month/day/hour)
/// Returns (NaiveDateTime, rest_of_line) or None if no timestamp found.
pub fn parse_timestamp(line: &str) -> Option<(NaiveDateTime, &str)> {
    // Find the position after the timestamp: look for 'a ' or 'p ' after time
    // Pattern: M/D/YY H:MM:SSa or M/D/YY H:MM:SSp
    // Min length: "1/1/17 1:00:00a " = 16 chars

    let bytes = line.as_bytes();
    if bytes.len() < 16 {
        return None;
    }

    // Find first space (between date and time)
    let date_end = line.find(' ')?;
    let date_part = &line[..date_end];

    // Parse date: M/D/YY
    let mut date_parts = date_part.split('/');
    let month: u32 = date_parts.next()?.parse().ok()?;
    let day: u32 = date_parts.next()?.parse().ok()?;
    let year_short: i32 = date_parts.next()?.parse().ok()?;
    if date_parts.next().is_some() {
        return None; // Extra slash
    }

    // Convert 2-digit year: 00-99 → 2000-2099
    let year = 2000 + year_short;

    // Parse time: H:MM:SSa/p <message>
    let rest = &line[date_end + 1..];

    // Find the space between time and message (e.g., "7:39:54p You slaughtered...")
    let time_end = rest.find(' ')?;
    let time_with_ampm = &rest[..time_end];
    let message = &rest[time_end + 1..];

    // Last char of time string is a/p
    if time_with_ampm.is_empty() {
        return None;
    }
    let am_pm = time_with_ampm.as_bytes()[time_with_ampm.len() - 1];
    let time_part = &time_with_ampm[..time_with_ampm.len() - 1];

    // Parse time: H:MM:SS
    let mut time_parts = time_part.split(':');
    let mut hour: u32 = time_parts.next()?.parse().ok()?;
    let minute: u32 = time_parts.next()?.parse().ok()?;
    let second: u32 = time_parts.next()?.parse().ok()?;
    if time_parts.next().is_some() {
        return None; // Extra colon
    }

    // Convert 12-hour to 24-hour
    match &am_pm {
        b'a' => {
            if hour == 12 {
                hour = 0; // 12:xx AM = 0:xx
            }
        }
        b'p' => {
            if hour != 12 {
                hour += 12; // 1-11 PM = 13-23
            }
        }
        _ => return None,
    }

    // Validate ranges
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 || second > 59 {
        return None;
    }

    let dt = chrono::NaiveDate::from_ymd_opt(year, month, day)?
        .and_hms_opt(hour, minute, second)?;

    Some((dt, message))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, Timelike};

    #[test]
    fn test_basic_am() {
        let (dt, msg) = parse_timestamp("11/20/17 7:31:48a You have 101 coins.").unwrap();
        assert_eq!(dt.year(), 2017);
        assert_eq!(dt.month(), 11);
        assert_eq!(dt.day(), 20);
        assert_eq!(dt.hour(), 7);
        assert_eq!(dt.minute(), 31);
        assert_eq!(dt.second(), 48);
        assert_eq!(msg, "You have 101 coins.");
    }

    #[test]
    fn test_basic_pm() {
        let (dt, msg) = parse_timestamp("4/9/18 7:39:54p You slaughtered a Rat.").unwrap();
        assert_eq!(dt.hour(), 19); // 7p = 19
        assert_eq!(dt.month(), 4);
        assert_eq!(dt.day(), 9);
        assert_eq!(msg, "You slaughtered a Rat.");
    }

    #[test]
    fn test_noon() {
        let (dt, _) = parse_timestamp("1/1/20 12:00:00p Hello").unwrap();
        assert_eq!(dt.hour(), 12);
    }

    #[test]
    fn test_midnight() {
        let (dt, _) = parse_timestamp("1/1/20 12:00:00a Hello").unwrap();
        assert_eq!(dt.hour(), 0);
    }

    #[test]
    fn test_double_digit_month_day() {
        let (dt, msg) = parse_timestamp("12/26/21 10:33:22p Welcome back, Ruuk!").unwrap();
        assert_eq!(dt.month(), 12);
        assert_eq!(dt.day(), 26);
        assert_eq!(dt.year(), 2021);
        assert_eq!(dt.hour(), 22); // 10p = 22
        assert_eq!(msg, "Welcome back, Ruuk!");
    }

    #[test]
    fn test_single_digit_everything() {
        let (dt, _) = parse_timestamp("1/2/03 1:05:09a test").unwrap();
        assert_eq!(dt.month(), 1);
        assert_eq!(dt.day(), 2);
        assert_eq!(dt.year(), 2003);
        assert_eq!(dt.hour(), 1);
    }

    #[test]
    fn test_no_timestamp() {
        assert!(parse_timestamp("This has no timestamp").is_none());
        assert!(parse_timestamp("").is_none());
    }

    #[test]
    fn test_yen_prefix_message() {
        let (dt, msg) = parse_timestamp("11/20/17 7:31:48a \u{00a5}Your combat ability improves.").unwrap();
        assert_eq!(dt.hour(), 7);
        assert!(msg.starts_with('¥'));
    }

    #[test]
    fn test_10am() {
        let (dt, _) = parse_timestamp("9/5/14 10:30:22a Hello").unwrap();
        assert_eq!(dt.hour(), 10);
    }

    #[test]
    fn test_11pm() {
        let (dt, _) = parse_timestamp("9/5/14 11:30:22p Hello").unwrap();
        assert_eq!(dt.hour(), 23);
    }
}
