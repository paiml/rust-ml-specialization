//! Pure, testable pieces of the planted-defect sweep: sizing the subset `n`
//! from a measured per-review wall time, and scoring one lane's outcomes
//! against MANIFEST ground truth. The live `std::process::Command` calls to
//! agy and claude stay in `main.rs`; everything here is deterministic.

use crate::corpus::ManifestFile;

/// `n = floor(budget_s / slowest_per_review_s / 2 lanes)`, clamped to
/// `[4, 20]`, then rounded down to even so clean:planted stays balanced.
pub fn choose_n(slowest_per_review_s: f64, budget_s: f64) -> u32 {
    let raw = (budget_s / slowest_per_review_s / 2.0).floor();
    let clamped = raw.clamp(4.0, 20.0) as u32;
    if clamped.is_multiple_of(2) {
        clamped
    } else {
        clamped - 1
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scored {
    pub recall: f64,
    pub precision: f64,
    pub true_positives: u32,
    pub false_positives: u32,
    pub planted_total: u32,
    pub clean_total: u32,
}

/// One lane's outcome on one diff in the subset: did it FAIL, and if so at
/// which lines. `diffs`, `failed` and `finding_lines` are parallel slices,
/// one entry per diff reviewed (clean and planted, any order).
pub fn score_lane(diffs: &[&ManifestFile], failed: &[bool], finding_lines: &[Vec<i64>]) -> Scored {
    assert_eq!(diffs.len(), failed.len());
    assert_eq!(diffs.len(), finding_lines.len());
    let mut tp = 0u32;
    let mut fp = 0u32;
    let mut planted_total = 0u32;
    let mut clean_total = 0u32;
    for ((file, &did_fail), lines) in diffs.iter().zip(failed).zip(finding_lines) {
        if file.planted {
            planted_total += 1;
            let marker = file.marker_line.expect("planted file has a marker_line") as i64;
            let found = did_fail && lines.iter().any(|l| (*l - marker).abs() <= 3);
            if found {
                tp += 1;
            }
        } else {
            clean_total += 1;
            if did_fail {
                fp += 1;
            }
        }
    }
    let recall = if planted_total == 0 {
        0.0
    } else {
        f64::from(tp) / f64::from(planted_total)
    };
    let precision = if tp + fp == 0 {
        1.0
    } else {
        f64::from(tp) / f64::from(tp + fp)
    };
    Scored {
        recall,
        precision,
        true_positives: tp,
        false_positives: fp,
        planted_total,
        clean_total,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::ManifestFile;

    fn planted(path: &str, marker_line: u32) -> ManifestFile {
        ManifestFile {
            path: path.to_string(),
            planted: true,
            class: Some("test".into()),
            marker_line: Some(marker_line),
            marker_text: Some("x".into()),
        }
    }

    fn clean(path: &str) -> ManifestFile {
        ManifestFile {
            path: path.to_string(),
            planted: false,
            class: None,
            marker_line: None,
            marker_text: None,
        }
    }

    #[test]
    fn choose_n_clamps_to_the_floor() {
        // A slow reviewer (e.g. 60s/review) drives the raw estimate below 4.
        assert_eq!(choose_n(60.0, 250.0), 4);
    }

    #[test]
    fn choose_n_clamps_to_the_ceiling() {
        // A fast reviewer (e.g. 1s/review) drives the raw estimate above 20.
        assert_eq!(choose_n(1.0, 250.0), 20);
    }

    #[test]
    fn choose_n_rounds_down_to_even() {
        // floor(250 / 12.5 / 2) = floor(10.0) = 10 (already even).
        assert_eq!(choose_n(12.5, 250.0), 10);
        // floor(250 / 11.5 / 2) = floor(10.86) = 10 (already even).
        assert_eq!(choose_n(11.5, 250.0), 10);
        // Pick a value whose raw n is odd: floor(250 / (250/11/2)) = 11 -> 10.
        let per_review = 250.0 / 11.0 / 2.0;
        assert_eq!(choose_n(per_review, 250.0), 10);
    }

    #[test]
    fn choose_n_never_goes_below_four() {
        for slow in [30.0, 60.0, 120.0, 1000.0] {
            assert!(choose_n(slow, 250.0) >= 4);
        }
    }

    #[test]
    fn score_lane_perfect_recall_and_precision() {
        let files = [
            planted("p1.diff", 10),
            planted("p2.diff", 20),
            clean("c1.diff"),
        ];
        let refs: Vec<&ManifestFile> = files.iter().collect();
        let failed = vec![true, true, false];
        let lines = vec![vec![10i64], vec![21i64], vec![]];
        let s = score_lane(&refs, &failed, &lines);
        assert_eq!(s.true_positives, 2);
        assert_eq!(s.false_positives, 0);
        assert_eq!(s.planted_total, 2);
        assert_eq!(s.clean_total, 1);
        assert!((s.recall - 1.0).abs() < 1e-9);
        assert!((s.precision - 1.0).abs() < 1e-9);
    }

    #[test]
    fn score_lane_line_outside_window_is_a_miss() {
        let files = [planted("p1.diff", 10)];
        let refs: Vec<&ManifestFile> = files.iter().collect();
        let failed = vec![true];
        let lines = vec![vec![20i64]]; // 10 away, outside +-3
        let s = score_lane(&refs, &failed, &lines);
        assert_eq!(s.true_positives, 0);
        assert_eq!(s.recall, 0.0);
    }

    #[test]
    fn score_lane_fail_on_clean_is_a_false_positive() {
        let files = [planted("p1.diff", 10), clean("c1.diff")];
        let refs: Vec<&ManifestFile> = files.iter().collect();
        let failed = vec![true, true];
        let lines = vec![vec![10i64], vec![3i64]];
        let s = score_lane(&refs, &failed, &lines);
        assert_eq!(s.true_positives, 1);
        assert_eq!(s.false_positives, 1);
        assert!((s.precision - 0.5).abs() < 1e-9);
    }

    #[test]
    fn score_lane_no_positives_at_all_is_precision_one_vacuously() {
        let files = [planted("p1.diff", 10), clean("c1.diff")];
        let refs: Vec<&ManifestFile> = files.iter().collect();
        let failed = vec![false, false];
        let lines = vec![vec![], vec![]];
        let s = score_lane(&refs, &failed, &lines);
        assert_eq!(s.recall, 0.0);
        assert_eq!(s.precision, 1.0);
    }
}
