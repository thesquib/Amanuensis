use std::collections::HashMap;

use crate::db::queries::CreatureFrequency;
use crate::models::Kill;

/// Output format for the unified kills export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    Csv,
    Text,
}

/// Column headers, in the Kills-view order.
const HEADERS: [&str; 13] = [
    "Creature", "Vanquished", "Killed", "Dispatched", "Slaughtered",
    "Killed By", "Value", "First Kill", "Last Kill",
    "Best Day", "Best Day Date", "Best 2h", "Best 2h Window",
];

/// One creature's cells, in HEADERS order. Frequency cells are "" when absent.
fn row_cells(k: &Kill, freq: Option<&CreatureFrequency>) -> Vec<String> {
    let (best_day, best_day_date, best_2h, best_2h_window) = match freq {
        Some(f) if f.best_day_count > 0 || f.best_2h_count > 0 => (
            num_or_blank(f.best_day_count),
            f.best_day_date.clone().unwrap_or_default(),
            num_or_blank(f.best_2h_count),
            f.best_2h_start.as_deref().map(two_hour_window).unwrap_or_default(),
        ),
        _ => (String::new(), String::new(), String::new(), String::new()),
    };
    vec![
        k.creature_name.clone(),
        (k.vanquished_count + k.assisted_vanquish_count).to_string(),
        (k.killed_count + k.assisted_kill_count).to_string(),
        (k.dispatched_count + k.assisted_dispatch_count).to_string(),
        (k.slaughtered_count + k.assisted_slaughter_count).to_string(),
        k.killed_by_count.to_string(),
        k.creature_value.to_string(),
        date_only(k.date_first.as_deref()),
        date_only(k.date_last.as_deref()),
        best_day,
        best_day_date,
        best_2h,
        best_2h_window,
    ]
}

fn num_or_blank(n: i64) -> String {
    if n > 0 { n.to_string() } else { String::new() }
}

/// Date portion of a "YYYY-MM-DD HH:MM:SS" timestamp (matches the GUI's date display).
fn date_only(s: Option<&str>) -> String {
    s.unwrap_or("").split(' ').next().unwrap_or("").to_string()
}

/// "YYYY-MM-DD HH:00" hour-bucket start -> "YYYY-MM-DD HH:00–HH:00" (start + 2h,
/// wrapping past midnight). Mirrors the GUI's formatTwoHourWindow tooltip.
fn two_hour_window(start: &str) -> String {
    let (date, time) = match start.split_once(' ') {
        Some(p) => p,
        None => return start.to_string(),
    };
    let start_hour: i64 = match time.split(':').next().and_then(|h| h.parse().ok()) {
        Some(h) => h,
        None => return start.to_string(),
    };
    let end_hour = (start_hour + 2) % 24;
    format!("{date} {start_hour:02}:00\u{2013}{end_hour:02}:00")
}

/// Quote a CSV cell when it contains a comma, quote, or space; double inner quotes.
/// (Same rule the CLI's frequency export uses.)
fn csv_cell(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains(' ') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Render the unified kills table to a string. Pure: callers decide ordering and
/// what to do with the result (write file / print). `freq` is joined onto `kills`
/// by creature name; kills are emitted in the order given.
pub fn format_kills_export(
    kills: &[Kill],
    freq: &[CreatureFrequency],
    format: ExportFormat,
) -> String {
    let freq_by_name: HashMap<&str, &CreatureFrequency> =
        freq.iter().map(|f| (f.creature_name.as_str(), f)).collect();

    match format {
        ExportFormat::Csv => {
            let mut out = String::new();
            out.push_str(&HEADERS.join(","));
            out.push('\n');
            for k in kills {
                let cells = row_cells(k, freq_by_name.get(k.creature_name.as_str()).copied());
                let line: Vec<String> = cells.iter().map(|c| csv_cell(c)).collect();
                out.push_str(&line.join(","));
                out.push('\n');
            }
            out
        }
        ExportFormat::Text => format_text(kills, &freq_by_name),
    }
}

fn format_text(_kills: &[Kill], _freq: &HashMap<&str, &CreatureFrequency>) -> String {
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::queries::CreatureFrequency;
    use crate::models::Kill;
    use std::collections::BTreeMap;

    fn lg_vermine() -> Kill {
        let mut k = Kill::new(0, "Large Vermine".into(), 70);
        k.killed_count = 5;
        k.assisted_kill_count = 2;
        k.slaughtered_count = 3;
        k.killed_by_count = 1;
        k.date_first = Some("2024-01-01 09:00:00".into());
        k.date_last = Some("2024-01-05 12:00:00".into());
        k
    }

    fn rat() -> Kill {
        let mut k = Kill::new(0, "Rat".into(), 2);
        k.killed_count = 8;
        k.date_first = Some("2024-02-01 09:00:00".into());
        k.date_last = Some("2024-02-02 09:00:00".into());
        k
    }

    fn lg_vermine_freq() -> CreatureFrequency {
        CreatureFrequency {
            creature_name: "Large Vermine".into(),
            best_day_count: 4,
            best_day_date: Some("2024-01-03".into()),
            best_day_verbs: BTreeMap::new(),
            best_2h_count: 3,
            best_2h_start: Some("2024-01-03 08:00".into()),
            best_2h_verbs: BTreeMap::new(),
        }
    }

    #[test]
    fn csv_has_header_and_rows_with_combined_totals_and_quoting() {
        let kills = vec![lg_vermine(), rat()];
        let freq = vec![lg_vermine_freq()]; // Rat has no frequency entry

        let out = format_kills_export(&kills, &freq, ExportFormat::Csv);
        let lines: Vec<&str> = out.lines().collect();

        assert_eq!(
            lines[0],
            "Creature,Vanquished,Killed,Dispatched,Slaughtered,Killed By,Value,First Kill,Last Kill,Best Day,Best Day Date,Best 2h,Best 2h Window"
        );
        assert_eq!(
            lines[1],
            r#""Large Vermine",0,7,0,3,1,70,2024-01-01,2024-01-05,4,2024-01-03,3,"2024-01-03 08:00–10:00""#
        );
        assert_eq!(lines[2], "Rat,0,8,0,0,0,2,2024-02-01,2024-02-02,,,,");
    }
}
