//! CVSS v3.x base score calculation (FIRST specification).
//!
//! Only base metrics are used. Vectors must start with `CVSS:3.0/` or
//! `CVSS:3.1/`. Qualitative ratings follow the official score bands.

use crate::store::Severity;

/// Parses a CVSS v3.0/v3.1 vector and returns the base score and severity band.
#[must_use]
pub fn score_v3(vector: &str) -> Option<(f64, Severity)> {
    let metrics = parse_v3_metrics(vector)?;
    let score = base_score(&metrics);
    Some((score, severity_from_score(score)))
}

/// Maps a CVSS base score to Craftward severity (None → Low).
#[must_use]
pub fn severity_from_score(score: f64) -> Severity {
    // FIRST: None is 0.0; Low is 0.1–3.9. Craftward stores both as `low`.
    if score < 4.0 {
        Severity::Low
    } else if score < 7.0 {
        Severity::Medium
    } else if score < 9.0 {
        Severity::High
    } else {
        Severity::Critical
    }
}

/// Maps a qualitative CVSS label (including v4 qualitative text) to severity.
#[must_use]
pub fn severity_from_qualitative(label: &str) -> Option<Severity> {
    match label.trim().to_ascii_uppercase().as_str() {
        "NONE" | "LOW" => Some(Severity::Low),
        "MEDIUM" | "MODERATE" => Some(Severity::Medium),
        "HIGH" => Some(Severity::High),
        "CRITICAL" => Some(Severity::Critical),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy)]
struct Metrics {
    av: f64,
    ac: f64,
    pr: f64,
    ui: f64,
    scope_changed: bool,
    c: f64,
    i: f64,
    a: f64,
}

fn parse_v3_metrics(vector: &str) -> Option<Metrics> {
    let rest = vector
        .strip_prefix("CVSS:3.1/")
        .or_else(|| vector.strip_prefix("CVSS:3.0/"))?;
    let mut av = None;
    let mut ac = None;
    let mut pr_letter = None;
    let mut ui = None;
    let mut scope_changed = None;
    let mut c = None;
    let mut i = None;
    let mut a = None;
    for part in rest.split('/') {
        let (key, value) = part.split_once(':')?;
        match key {
            "AV" => {
                av = Some(match value {
                    "N" => 0.85,
                    "A" => 0.62,
                    "L" => 0.55,
                    "P" => 0.2,
                    _ => return None,
                });
            }
            "AC" => {
                ac = Some(match value {
                    "L" => 0.77,
                    "H" => 0.44,
                    _ => return None,
                });
            }
            "PR" => pr_letter = Some(value),
            "UI" => {
                ui = Some(match value {
                    "N" => 0.85,
                    "R" => 0.62,
                    _ => return None,
                });
            }
            "S" => {
                scope_changed = Some(match value {
                    "U" => false,
                    "C" => true,
                    _ => return None,
                });
            }
            "C" => c = Some(cia(value)?),
            "I" => i = Some(cia(value)?),
            "A" => a = Some(cia(value)?),
            _ => {}
        }
    }
    let scope_changed = scope_changed?;
    let pr = match (pr_letter?, scope_changed) {
        ("N", _) => 0.85,
        ("L", false) => 0.62,
        ("L", true) => 0.68,
        ("H", false) => 0.27,
        ("H", true) => 0.5,
        _ => return None,
    };
    Some(Metrics {
        av: av?,
        ac: ac?,
        pr,
        ui: ui?,
        scope_changed,
        c: c?,
        i: i?,
        a: a?,
    })
}

fn cia(value: &str) -> Option<f64> {
    match value {
        "H" => Some(0.56),
        "L" => Some(0.22),
        "N" => Some(0.0),
        _ => None,
    }
}

fn base_score(metrics: &Metrics) -> f64 {
    let iss = 1.0 - ((1.0 - metrics.c) * (1.0 - metrics.i) * (1.0 - metrics.a));
    let impact = if metrics.scope_changed {
        7.52 * (iss - 0.029) - 3.25 * (iss - 0.02).powi(15)
    } else {
        6.42 * iss
    };
    if impact <= 0.0 {
        return 0.0;
    }
    let exploitability = 8.22 * metrics.av * metrics.ac * metrics.pr * metrics.ui;
    let score = if metrics.scope_changed {
        (1.08 * (impact + exploitability)).min(10.0)
    } else {
        (impact + exploitability).min(10.0)
    };
    roundup(score)
}

fn roundup(value: f64) -> f64 {
    let scaled = (value * 100_000.0).round() / 100_000.0;
    (scaled * 10.0).ceil() / 10.0
}

#[cfg(test)]
mod tests {
    use super::{score_v3, severity_from_qualitative, severity_from_score};
    use crate::store::Severity;

    #[test]
    fn first_specification_example_vectors() {
        // FIRST CVSS v3.1 examples (base score).
        assert_eq!(
            score_v3("CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H"),
            Some((9.8, Severity::Critical))
        );
        assert_eq!(
            score_v3("CVSS:3.1/AV:N/AC:L/PR:N/UI:R/S:C/C:L/I:L/A:N"),
            Some((6.1, Severity::Medium))
        );
        assert_eq!(
            score_v3("CVSS:3.1/AV:N/AC:L/PR:L/UI:N/S:C/C:L/I:L/A:N"),
            Some((6.4, Severity::Medium))
        );
        assert_eq!(
            score_v3("CVSS:3.1/AV:L/AC:L/PR:H/UI:N/S:U/C:H/I:H/A:H"),
            Some((6.7, Severity::Medium))
        );
        assert_eq!(
            score_v3("CVSS:3.1/AV:L/AC:L/PR:N/UI:R/S:U/C:H/I:H/A:H"),
            Some((7.8, Severity::High))
        );
        assert_eq!(
            score_v3("CVSS:3.0/AV:N/AC:L/PR:N/UI:N/S:U/C:N/I:N/A:N"),
            Some((0.0, Severity::Low))
        );
    }

    #[test]
    fn score_bands_match_first() {
        assert_eq!(severity_from_score(0.0), Severity::Low);
        assert_eq!(severity_from_score(3.9), Severity::Low);
        assert_eq!(severity_from_score(4.0), Severity::Medium);
        assert_eq!(severity_from_score(6.9), Severity::Medium);
        assert_eq!(severity_from_score(7.0), Severity::High);
        assert_eq!(severity_from_score(8.9), Severity::High);
        assert_eq!(severity_from_score(9.0), Severity::Critical);
    }

    #[test]
    fn qualitative_labels() {
        assert_eq!(
            severity_from_qualitative("Critical"),
            Some(Severity::Critical)
        );
        assert_eq!(
            severity_from_qualitative("MODERATE"),
            Some(Severity::Medium)
        );
        assert_eq!(severity_from_qualitative("none"), Some(Severity::Low));
        assert_eq!(severity_from_qualitative("weird"), None);
    }
}
