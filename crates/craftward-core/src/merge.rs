//! Merges OSV findings that share package versions and advisory aliases.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::radar::Finding;
use crate::store::{FindingKind, Severity, SeveritySource};

/// Merges findings that share ecosystem/package/version and are linked by id
/// or aliases (transitive). Different packages stay separate.
#[must_use]
pub fn merge_aliased_findings(findings: Vec<Finding>) -> Vec<Finding> {
    let mut by_package: BTreeMap<(String, String, String), Vec<Finding>> = BTreeMap::new();
    for finding in findings {
        by_package
            .entry((
                finding.ecosystem.clone(),
                finding.package.clone(),
                finding.version.clone(),
            ))
            .or_default()
            .push(finding);
    }
    let mut merged = Vec::new();
    for (_, group) in by_package {
        merged.extend(merge_package_group(group));
    }
    merged.sort_by(|left, right| {
        left.severity
            .cmp(&right.severity)
            .then(left.package.cmp(&right.package))
            .then(left.version.cmp(&right.version))
            .then(left.id.cmp(&right.id))
    });
    merged
}

fn merge_package_group(group: Vec<Finding>) -> Vec<Finding> {
    if group.len() <= 1 {
        return group;
    }
    let n = group.len();
    let mut parent: Vec<usize> = (0..n).collect();
    let mut id_owner: HashMap<String, usize> = HashMap::new();
    for (index, finding) in group.iter().enumerate() {
        for id in finding_ids(finding) {
            if let Some(existing) = id_owner.get(&id).copied() {
                union(&mut parent, index, existing);
            } else {
                id_owner.insert(id, index);
            }
        }
    }
    let mut clusters: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for index in 0..n {
        clusters
            .entry(find(&mut parent, index))
            .or_default()
            .push(index);
    }
    clusters
        .into_values()
        .map(|members| {
            let findings: Vec<&Finding> = members.iter().map(|index| &group[*index]).collect();
            collapse_cluster(&findings)
        })
        .collect()
}

fn finding_ids(finding: &Finding) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    ids.insert(finding.id.clone());
    for alias in &finding.aliases {
        ids.insert(alias.clone());
    }
    for id in &finding.related_ids {
        ids.insert(id.clone());
    }
    ids
}

fn collapse_cluster(findings: &[&Finding]) -> Finding {
    let mut all_ids = BTreeSet::new();
    let mut fixed = BTreeSet::new();
    let mut best_severity = Severity::Unknown;
    let mut best_source = SeveritySource::Unknown;
    let mut best_kind = findings[0].kind;
    let mut summary = String::new();
    let first = findings[0];
    for finding in findings {
        all_ids.insert(finding.id.clone());
        for alias in &finding.aliases {
            all_ids.insert(alias.clone());
        }
        for id in &finding.related_ids {
            all_ids.insert(id.clone());
        }
        for version in &finding.fixed_versions {
            fixed.insert(version.clone());
        }
        if severity_rank(finding.severity) < severity_rank(best_severity) {
            best_severity = finding.severity;
            best_source = finding.severity_source;
        } else if finding.severity == best_severity
            && source_rank(finding.severity_source) < source_rank(best_source)
        {
            best_source = finding.severity_source;
        }
        if kind_rank(finding.kind) < kind_rank(best_kind) {
            best_kind = finding.kind;
        }
        if summary.is_empty() && !finding.summary.is_empty() {
            summary.clone_from(&finding.summary);
        }
    }
    let primary = prefer_primary_id(&all_ids);
    let mut related_ids: Vec<String> = all_ids.into_iter().collect();
    related_ids.sort();
    if let Some(position) = related_ids.iter().position(|id| id == &primary) {
        related_ids.swap(0, position);
    }
    let aliases: Vec<String> = related_ids
        .iter()
        .filter(|id| *id != &primary)
        .cloned()
        .collect();
    Finding {
        ecosystem: first.ecosystem.clone(),
        package: first.package.clone(),
        version: first.version.clone(),
        id: primary.clone(),
        aliases,
        related_ids,
        severity: best_severity,
        severity_source: best_source,
        summary,
        fixed_versions: fixed.into_iter().collect(),
        url: format!("https://osv.dev/vulnerability/{primary}"),
        kind: best_kind,
    }
}

/// Chooses the display id: GHSA, else RUSTSEC, else the first sorted id.
#[must_use]
pub fn prefer_primary_id(ids: &BTreeSet<String>) -> String {
    if let Some(id) = ids.iter().find(|id| id.starts_with("GHSA-")) {
        return id.clone();
    }
    if let Some(id) = ids.iter().find(|id| id.starts_with("RUSTSEC-")) {
        return id.clone();
    }
    ids.iter().next().cloned().unwrap_or_default()
}

fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Critical => 0,
        Severity::High => 1,
        Severity::Medium => 2,
        Severity::Low => 3,
        Severity::Unknown => 4,
    }
}

fn source_rank(source: SeveritySource) -> u8 {
    match source {
        SeveritySource::Advisory => 0,
        SeveritySource::Cvss => 1,
        SeveritySource::Unknown => 2,
    }
}

fn kind_rank(kind: FindingKind) -> u8 {
    match kind {
        FindingKind::Malicious => 0,
        FindingKind::Vulnerability => 1,
        FindingKind::Unsound => 2,
        FindingKind::Unmaintained => 3,
        FindingKind::Notice => 4,
        FindingKind::PlatformDeadline => 5,
        FindingKind::Eol => 6,
        FindingKind::PlatformDeadlineSoon => 7,
        FindingKind::EolSoon => 8,
    }
}

fn find(parent: &mut [usize], mut index: usize) -> usize {
    while parent[index] != index {
        parent[index] = parent[parent[index]];
        index = parent[index];
    }
    index
}

fn union(parent: &mut [usize], left: usize, right: usize) {
    let left_root = find(parent, left);
    let right_root = find(parent, right);
    if left_root != right_root {
        parent[right_root] = left_root;
    }
}

#[cfg(test)]
mod tests {
    use super::{merge_aliased_findings, prefer_primary_id};
    use crate::radar::Finding;
    use crate::store::{FindingKind, Severity, SeveritySource};
    use std::collections::BTreeSet;

    fn finding(id: &str, aliases: &[&str], package: &str) -> Finding {
        Finding {
            ecosystem: "crates.io".to_owned(),
            package: package.to_owned(),
            version: "0.10.76".to_owned(),
            id: id.to_owned(),
            aliases: aliases.iter().map(|text| (*text).to_owned()).collect(),
            related_ids: {
                let mut ids = vec![id.to_owned()];
                ids.extend(aliases.iter().map(|text| (*text).to_owned()));
                ids
            },
            severity: Severity::High,
            severity_source: SeveritySource::Advisory,
            summary: id.to_owned(),
            fixed_versions: vec!["0.10.77".to_owned()],
            url: format!("https://osv.dev/vulnerability/{id}"),
            kind: FindingKind::Vulnerability,
        }
    }

    #[test]
    fn merges_two_aliased_ids() {
        let merged = merge_aliased_findings(vec![
            finding("RUSTSEC-2026-0463", &["CVE-2026-1"], "openssl"),
            finding("GHSA-8c75-8mhr-p7r9", &["CVE-2026-1"], "openssl"),
        ]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].id, "GHSA-8c75-8mhr-p7r9");
        assert!(merged[0].aliases.contains(&"RUSTSEC-2026-0463".to_owned()));
        assert!(merged[0].aliases.contains(&"CVE-2026-1".to_owned()));
        assert_eq!(merged[0].related_ids.len(), 3);
    }

    #[test]
    fn merges_three_transitively() {
        let merged = merge_aliased_findings(vec![
            finding("A", &["X"], "pkg"),
            finding("B", &["X"], "pkg"),
            finding("C", &["B"], "pkg"),
        ]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].related_ids.len(), 4);
    }

    #[test]
    fn keeps_different_packages_apart() {
        let merged = merge_aliased_findings(vec![
            finding("GHSA-1", &["CVE-1"], "alpha"),
            finding("GHSA-1", &["CVE-1"], "beta"),
        ]);
        assert_eq!(merged.len(), 2);
    }

    #[test]
    fn prefers_ghsa_then_rustsec() {
        let ids = BTreeSet::from([
            "CVE-1".to_owned(),
            "RUSTSEC-1".to_owned(),
            "GHSA-1".to_owned(),
        ]);
        assert_eq!(prefer_primary_id(&ids), "GHSA-1");
        let ids = BTreeSet::from(["CVE-1".to_owned(), "RUSTSEC-1".to_owned()]);
        assert_eq!(prefer_primary_id(&ids), "RUSTSEC-1");
    }

    #[test]
    fn takes_worst_kind_and_unions_fixed_versions() {
        let mut left = finding("GHSA-1", &["CVE-1"], "pkg");
        left.kind = FindingKind::Unmaintained;
        left.fixed_versions = vec!["1.0.1".to_owned()];
        let mut right = finding("RUSTSEC-1", &["CVE-1"], "pkg");
        right.kind = FindingKind::Malicious;
        right.fixed_versions = vec!["1.0.2".to_owned()];
        let merged = merge_aliased_findings(vec![left, right]);
        assert_eq!(merged[0].kind, FindingKind::Malicious);
        assert_eq!(
            merged[0].fixed_versions,
            vec!["1.0.1".to_owned(), "1.0.2".to_owned()]
        );
    }
}
