//! Shared portfolio ordering for the desktop app and `craftward list`.

use std::cmp::Ordering;

use crate::model::ProjectStatus;

/// Inputs for [`compare_project_order`].
#[derive(Debug, Clone, Copy)]
pub struct ProjectOrderKey<'a> {
    /// Lifecycle status.
    pub status: ProjectStatus,
    /// Open finding counts by severity (critical…unknown).
    pub findings: &'a [usize; 5],
    /// Earliest outstanding due date, when any.
    pub due: Option<&'a str>,
    /// Display name.
    pub name: &'a str,
    /// Stable project id (tie-break).
    pub id: &'a str,
}

/// Compares two projects for portfolio / `list` order.
///
/// Status rank first, then open-finding severity counts (critical…unknown,
/// higher counts sort first), then next due date, then name, then id.
#[must_use]
pub fn compare_project_order(left: &ProjectOrderKey<'_>, right: &ProjectOrderKey<'_>) -> Ordering {
    left.status
        .sort_rank()
        .cmp(&right.status.sort_rank())
        .then_with(|| right.findings.cmp(left.findings))
        .then_with(|| match (left.due, right.due) {
            (Some(a), Some(b)) => a.cmp(b),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        })
        .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
        .then_with(|| left.id.cmp(right.id))
}

#[cfg(test)]
mod tests {
    use super::{ProjectOrderKey, compare_project_order};
    use crate::model::ProjectStatus;

    fn key<'a>(
        status: ProjectStatus,
        findings: &'a [usize; 5],
        due: Option<&'a str>,
        name: &'a str,
        id: &'a str,
    ) -> ProjectOrderKey<'a> {
        ProjectOrderKey {
            status,
            findings,
            due,
            name,
            id,
        }
    }

    #[test]
    fn status_outranks_findings() {
        let left = key(ProjectStatus::Released, &[1, 0, 0, 0, 0], None, "Z", "z");
        let right = key(
            ProjectStatus::Idea,
            &[0, 0, 0, 0, 0],
            Some("2020-01-01"),
            "A",
            "a",
        );
        assert_eq!(
            compare_project_order(&left, &right),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn status_rank_order_and_open_findings_within_status() {
        let statuses = [
            ProjectStatus::Released,
            ProjectStatus::Maintenance,
            ProjectStatus::InDevelopment,
            ProjectStatus::Paused,
            ProjectStatus::Idea,
            ProjectStatus::Discontinued,
        ];
        let empty = [0, 0, 0, 0, 0];
        let critical = [9, 0, 0, 0, 0];
        for window in statuses.windows(2) {
            let left = key(window[0], &empty, None, "b", "b");
            let right = key(window[1], &critical, Some("2000-01-01"), "a", "a");
            assert_eq!(
                compare_project_order(&left, &right),
                std::cmp::Ordering::Less,
                "{:?} before {:?}",
                window[0],
                window[1]
            );
        }
        let high = [0, 1, 0, 0, 0];
        let medium = [0, 0, 1, 0, 0];
        let left = key(
            ProjectStatus::Maintenance,
            &high,
            Some("2027-01-01"),
            "z",
            "z",
        );
        let right = key(
            ProjectStatus::Maintenance,
            &medium,
            Some("2020-01-01"),
            "a",
            "a",
        );
        assert_eq!(
            compare_project_order(&left, &right),
            std::cmp::Ordering::Less
        );
    }
}
