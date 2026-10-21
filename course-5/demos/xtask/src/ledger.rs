//! Nightly ledger (RD-7 lane (b), #792-S6): one TSV row per night --
//! `date  demos  green  red  not_run  apr_sha` -- and `nights_in_last_7`,
//! the KPI "7/7 nights per week", computed from it. An absent ledger is
//! NOT MEASURED, never 0/7 and never 7/7.

use std::path::Path;

/// One night's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub date: String,
    pub demos: u32,
    pub green: u32,
    pub red: u32,
    pub not_run: u32,
    pub apr_sha: String,
}

impl Row {
    pub fn line(&self) -> String {
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            self.date, self.demos, self.green, self.red, self.not_run, self.apr_sha
        )
    }

    /// `None` for a malformed line (comment, blank, bad date or counts).
    pub fn parse(line: &str) -> Option<Row> {
        let f: Vec<&str> = line.trim().split('\t').collect();
        if f.len() != 6 || day_number(f[0]).is_none() {
            return None;
        }
        let n = |s: &str| s.parse::<u32>().ok();
        Some(Row {
            date: f[0].to_string(),
            demos: n(f[1])?,
            green: n(f[2])?,
            red: n(f[3])?,
            not_run: n(f[4])?,
            apr_sha: f[5].to_string(),
        })
    }
}

/// Days since 1970-01-01 of `YYYY-MM-DD`; `None` if it is not a real date.
pub fn day_number(date: &str) -> Option<i64> {
    let p: Vec<&str> = date.split('-').collect();
    if p.len() != 3 || p[0].len() != 4 || p[1].len() != 2 || p[2].len() != 2 {
        return None;
    }
    let (y, m, d) = (
        p[0].parse::<i64>().ok()?,
        p[1].parse::<i64>().ok()?,
        p[2].parse::<i64>().ok()?,
    );
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let dim = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if !(1..=12).contains(&m) || d < 1 || d > dim[(m - 1) as usize] {
        return None;
    }
    // Howard Hinnant's days_from_civil.
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// Distinct ledger nights in the 7 days ending `today` (inclusive).
/// `None` (NOT MEASURED) when there is no ledger text or `today` is not a date.
pub fn nights_in_last_7(ledger: Option<&str>, today: &str) -> Option<usize> {
    let t = day_number(today)?;
    let text = ledger?;
    let mut days: Vec<i64> = text
        .lines()
        .filter_map(Row::parse)
        .filter_map(|r| day_number(&r.date))
        .filter(|d| *d <= t && *d > t - 7)
        .collect();
    days.sort_unstable();
    days.dedup();
    Some(days.len())
}

/// `7/7`-style figure, or `NOT MEASURED`.
pub fn figure(ledger: Option<&str>, today: &str) -> String {
    match nights_in_last_7(ledger, today) {
        Some(n) => format!("{n}/7"),
        None => "NOT MEASURED".to_string(),
    }
}

/// Append `row`; a second run on the same date replaces that night's row, so
/// the ledger holds one row per night. Creates the file if absent.
pub fn append(path: &Path, row: &Row) -> Result<(), String> {
    let old = std::fs::read_to_string(path).unwrap_or_default();
    let mut lines: Vec<String> = old
        .lines()
        .filter(|l| Row::parse(l).map_or(true, |r| r.date != row.date))
        .map(str::to_string)
        .collect();
    lines.push(row.line());
    std::fs::write(path, lines.join("\n") + "\n").map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(date: &str) -> Row {
        Row {
            date: date.into(),
            demos: 12,
            green: 9,
            red: 1,
            not_run: 2,
            apr_sha: "d13f86934".into(),
        }
    }

    fn ledger(dates: &[&str]) -> String {
        dates.iter().map(|d| row(d).line() + "\n").collect()
    }

    #[test]
    fn seven_consecutive_nights_read_7_of_7() {
        let l = ledger(&[
            "2026-10-15",
            "2026-10-16",
            "2026-10-17",
            "2026-10-18",
            "2026-10-19",
            "2026-10-20",
            "2026-10-21",
        ]);
        assert_eq!(figure(Some(&l), "2026-10-21"), "7/7");
    }

    #[test]
    fn a_gap_reads_6_of_7() {
        let l = ledger(&[
            "2026-10-15",
            "2026-10-16",
            "2026-10-18",
            "2026-10-19",
            "2026-10-20",
            "2026-10-21",
        ]);
        assert_eq!(figure(Some(&l), "2026-10-21"), "6/7");
    }

    #[test]
    fn a_missing_ledger_is_not_measured_never_0_or_7() {
        assert_eq!(figure(None, "2026-10-21"), "NOT MEASURED");
        assert_eq!(figure(Some(""), "2026-10-21"), "0/7");
        assert_eq!(
            figure(Some(&ledger(&["2026-10-21"])), "not-a-date"),
            "NOT MEASURED"
        );
    }

    #[test]
    fn duplicates_future_and_old_rows_do_not_count() {
        let l = ledger(&["2026-10-21", "2026-10-21", "2026-10-22", "2026-10-14"]);
        assert_eq!(figure(Some(&l), "2026-10-21"), "1/7");
    }

    #[test]
    fn day_numbers_cross_month_year_and_leap_boundaries() {
        assert_eq!(day_number("1970-01-01"), Some(0));
        assert_eq!(
            day_number("2026-11-01"),
            day_number("2026-10-31").map(|d| d + 1)
        );
        assert_eq!(
            day_number("2028-03-01"),
            day_number("2028-02-29").map(|d| d + 1)
        );
        assert_eq!(day_number("2027-02-29"), None);
        assert_eq!(day_number("2026-13-01"), None);
    }

    #[test]
    fn append_makes_one_row_per_night() {
        let p = std::env::temp_dir().join(format!("ledger-{}.tsv", std::process::id()));
        std::fs::remove_file(&p).ok();
        append(&p, &row("2026-10-20")).unwrap();
        let mut again = row("2026-10-21");
        append(&p, &again).unwrap();
        again.red = 3;
        append(&p, &again).unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        std::fs::remove_file(&p).ok();
        assert_eq!(text.lines().count(), 2);
        assert!(text.contains("2026-10-21\t12\t9\t3\t2"));
        assert_eq!(nights_in_last_7(Some(&text), "2026-10-21"), Some(2));
    }
}
