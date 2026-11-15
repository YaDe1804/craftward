//! End-of-life checks against the endoflife.date API v1.
//!
//! The product list is `GET https://endoflife.date/api/v1/products/`.
//! One product, including its release cycles, is
//! `GET https://endoflife.date/api/v1/products/{name}/`.
//! The path contains only the product name. The API version in the
//! specification is 1.2.1. The data is under the MIT License.

use std::path::Path;

use serde::de::Deserializer;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::cache::{self, CacheChoice, CachePolicy, ShapeFault, TimedCache};
use crate::date::{days_between, is_calendar_date};
use crate::http::{HttpClient, HttpRequest, MAX_RESPONSE_BYTES, Method, accepted_body_or};
use crate::json_loose::{lenient_array, optional_bool, optional_lenient_vec};
use crate::model::Component;
use crate::percent::{encode, encode_file_name};
use crate::store::{FindingKind, Severity};

const LIST_URL: &str = "https://endoflife.date/api/v1/products/";
const PRODUCT_URL: &str = "https://endoflife.date/api/v1/products/";
const TTL_SECONDS: i64 = 24 * 60 * 60;

/// One product name from the product list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogProduct {
    /// Product id used in the API path.
    pub name: String,
    /// Alternate names from the product summary.
    #[serde(default, deserialize_with = "crate::json_loose::lenient_vec")]
    pub aliases: Vec<String>,
}

/// One release cycle and the end-of-life fields from API v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Cycle {
    /// Cycle name, such as `9.3`.
    pub name: String,
    /// `isEol`. Absent when the record did not contain a boolean.
    pub is_eol: Option<bool>,
    /// `eolFrom`, as `YYYY-MM-DD`, when it is a real calendar day.
    pub eol_from: Option<String>,
    /// `eolFrom` was present but not null and not a calendar day.
    pub eol_from_invalid: bool,
}

/// A support-end finding for one component version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EolHit {
    /// `EOL:<product>:<cycle>`.
    pub id: String,
    /// Product id.
    pub product: String,
    /// Component version that selected the cycle.
    pub version: String,
    /// `eol` or `eol_soon`.
    pub kind: FindingKind,
    /// `high` for `eol`, `medium` for `eol_soon`.
    pub severity: Severity,
    /// Short description.
    pub summary: String,
    /// Product page on endoflife.date.
    pub url: String,
}

/// Warnings and findings for one project's components.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EolReport {
    /// Support-end findings.
    pub hits: Vec<EolHit>,
    /// Warnings that do not fail the project.
    pub warnings: Vec<String>,
    /// Why the check could not be completed.
    pub error: Option<String>,
    /// Whether a component with a version was considered.
    pub checked: bool,
}

/// Cache and date inputs for one end-of-life check.
pub struct EolOptions<'a> {
    /// Directory that holds `eol/`.
    pub cache_dir: &'a Path,
    /// Ignore fresh cache entries.
    pub refresh: bool,
    /// Never contact endoflife.date.
    pub offline: bool,
    /// POSIX seconds used for cache expiry.
    pub now: i64,
    /// UTC day of the check, `YYYY-MM-DD`.
    pub today: &'a str,
    /// Days ahead of today that still count as `eol_soon`.
    pub warning_days: u32,
}

/// Checks every component that has a version.
///
/// # Errors
///
/// A network or cache failure is returned on [`EolReport::error`]. A missing
/// product or cycle is a warning.
pub fn check_components(
    components: &[Component],
    client: &impl HttpClient,
    options: &EolOptions<'_>,
) -> EolReport {
    let needed: Vec<&Component> = components
        .iter()
        .filter(|component| has_version(component) && !eol_disabled(component))
        .collect();
    if needed.is_empty() {
        return EolReport {
            hits: Vec::new(),
            warnings: Vec::new(),
            error: None,
            checked: false,
        };
    }
    let catalog = match load_catalog(client, options) {
        Ok(catalog) => catalog,
        Err(error) => {
            return EolReport {
                hits: Vec::new(),
                warnings: Vec::new(),
                error: Some(error),
                checked: true,
            };
        }
    };
    let mut hits = Vec::new();
    let mut warnings = Vec::new();
    for component in needed {
        let version = component.version.as_deref().unwrap_or("");
        let product = match select_product(&catalog.products, component) {
            ProductChoice::One(name) => name,
            ProductChoice::None => {
                warnings.push(format!(
                    "no endoflife.date product for {}; set eol_product",
                    component.name
                ));
                continue;
            }
        };
        let cycles = match load_cycles(client, options, &product) {
            Ok(cycles) => cycles,
            Err(error) => {
                return EolReport {
                    hits,
                    warnings,
                    error: Some(error),
                    checked: true,
                };
            }
        };
        match select_cycle(&cycles.cycles, version) {
            CycleChoice::One(cycle) => match effect(
                &product,
                version,
                cycle,
                options.today,
                options.warning_days,
            ) {
                CycleEffect::Hit(hit) => hits.push(hit),
                CycleEffect::Quiet => {}
                CycleEffect::Warning(warning) => warnings.push(warning),
            },
            CycleChoice::None if version_is_newer(version, &cycles.cycles) => {
                warnings.push(format!(
                    "{} {version} is newer than any cycle known to endoflife.date",
                    component.name
                ));
            }
            CycleChoice::None => {
                warnings.push(format!("no endoflife.date cycle for {product} {version}"));
            }
            CycleChoice::Tie => {
                warnings.push(format!(
                    "more than one endoflife.date cycle matches {product} {version}"
                ));
            }
        }
        warnings.extend(cycles.warnings);
    }
    warnings.extend(catalog.warnings);
    EolReport {
        hits,
        warnings,
        error: None,
        checked: true,
    }
}

/// Letters and digits, folded to lower case.
#[must_use]
pub fn normalize_name(value: &str) -> String {
    value
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect()
}

enum ProductChoice {
    One(String),
    None,
}

fn select_product(catalog: &[CatalogProduct], component: &Component) -> ProductChoice {
    if let Some(explicit) = component
        .eol_product
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        let matches: Vec<&CatalogProduct> = catalog
            .iter()
            .filter(|product| product.name.eq_ignore_ascii_case(explicit))
            .collect();
        return if matches.len() == 1 {
            ProductChoice::One(matches[0].name.clone())
        } else {
            ProductChoice::None
        };
    }
    let wanted = normalize_name(&component.name);
    if wanted.is_empty() {
        return ProductChoice::None;
    }
    let matches: Vec<&CatalogProduct> = catalog
        .iter()
        .filter(|product| {
            normalize_name(&product.name) == wanted
                || product
                    .aliases
                    .iter()
                    .any(|alias| normalize_name(alias) == wanted)
        })
        .collect();
    if matches.len() == 1 {
        ProductChoice::One(matches[0].name.clone())
    } else {
        ProductChoice::None
    }
}

enum CycleChoice<'a> {
    One(&'a Cycle),
    None,
    Tie,
}

fn select_cycle<'a>(cycles: &'a [Cycle], version: &str) -> CycleChoice<'a> {
    let version_parts = parts(version);
    let mut best: Vec<&Cycle> = Vec::new();
    let mut best_len = 0_usize;
    for cycle in cycles {
        let cycle_parts = parts(&cycle.name);
        if cycle_parts.is_empty() || cycle_parts.len() > version_parts.len() {
            continue;
        }
        if !cycle_parts
            .iter()
            .zip(&version_parts)
            .all(|(left, right)| left == right)
        {
            continue;
        }
        if cycle_parts.len() > best_len {
            best.clear();
            best_len = cycle_parts.len();
        }
        if cycle_parts.len() == best_len {
            best.push(cycle);
        }
    }
    match best.len() {
        1 => CycleChoice::One(best[0]),
        0 => CycleChoice::None,
        _ => CycleChoice::Tie,
    }
}

fn parts(value: &str) -> Vec<&str> {
    value.split('.').filter(|part| !part.is_empty()).collect()
}

fn has_version(component: &Component) -> bool {
    component
        .version
        .as_ref()
        .is_some_and(|version| !version.trim().is_empty())
}

fn eol_disabled(component: &Component) -> bool {
    component
        .eol_product
        .as_ref()
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("none"))
}

fn version_is_newer(version: &str, cycles: &[Cycle]) -> bool {
    let Some(version_parts) = numeric_parts(version) else {
        return false;
    };
    let mut newest: Option<Vec<u64>> = None;
    for cycle in cycles {
        let Some(cycle_parts) = numeric_parts(&cycle.name) else {
            continue;
        };
        let replace = newest
            .as_ref()
            .is_none_or(|current| cmp_versions(&cycle_parts, current).is_gt());
        if replace {
            newest = Some(cycle_parts);
        }
    }
    newest.is_some_and(|current| cmp_versions(&version_parts, &current).is_gt())
}

fn numeric_parts(value: &str) -> Option<Vec<u64>> {
    let raw = parts(value);
    if raw.is_empty() {
        return None;
    }
    raw.iter().map(|part| part.parse().ok()).collect()
}

fn cmp_versions(left: &[u64], right: &[u64]) -> std::cmp::Ordering {
    let length = left.len().max(right.len());
    for index in 0..length {
        let left_part = left.get(index).copied().unwrap_or(0);
        let right_part = right.get(index).copied().unwrap_or(0);
        match left_part.cmp(&right_part) {
            std::cmp::Ordering::Equal => {}
            other => return other,
        }
    }
    std::cmp::Ordering::Equal
}

enum CycleEffect {
    Hit(EolHit),
    Quiet,
    Warning(String),
}

fn effect(
    product: &str,
    version: &str,
    cycle: &Cycle,
    today: &str,
    warning_days: u32,
) -> CycleEffect {
    let Some(is_eol) = cycle.is_eol else {
        return unusable(product, &cycle.name);
    };
    if cycle.eol_from_invalid {
        return unusable(product, &cycle.name);
    }
    match (is_eol, cycle.eol_from.as_deref()) {
        (true, Some(date)) => match days_between(today, date) {
            Some(distance) if distance < 0 => CycleEffect::Hit(hit(
                product,
                version,
                cycle,
                FindingKind::Eol,
                Severity::High,
                date,
            )),
            Some(_) => contradictory(product, &cycle.name),
            None => unusable(product, &cycle.name),
        },
        (true, None) => CycleEffect::Hit(hit(
            product,
            version,
            cycle,
            FindingKind::Eol,
            Severity::High,
            "unknown",
        )),
        (false, Some(date)) => match days_between(today, date) {
            Some(distance) if distance < 0 => contradictory(product, &cycle.name),
            Some(distance) if distance <= i64::from(warning_days) => CycleEffect::Hit(hit(
                product,
                version,
                cycle,
                FindingKind::EolSoon,
                Severity::Medium,
                date,
            )),
            Some(_) => CycleEffect::Quiet,
            None => unusable(product, &cycle.name),
        },
        (false, None) => CycleEffect::Quiet,
    }
}

fn hit(
    product: &str,
    version: &str,
    cycle: &Cycle,
    kind: FindingKind,
    severity: Severity,
    date: &str,
) -> EolHit {
    let verb = if kind == FindingKind::Eol {
        "ended"
    } else {
        "ends"
    };
    EolHit {
        id: format!("EOL:{product}:{}", cycle.name),
        product: product.to_owned(),
        version: version.to_owned(),
        kind,
        severity,
        summary: format!("support {verb} on {date}"),
        url: format!("https://endoflife.date/{product}"),
    }
}

fn unusable(product: &str, cycle: &str) -> CycleEffect {
    CycleEffect::Warning(format!(
        "unusable endoflife.date data for {product} {cycle}"
    ))
}

fn contradictory(product: &str, cycle: &str) -> CycleEffect {
    CycleEffect::Warning(format!(
        "contradictory endoflife.date data for {product} {cycle}"
    ))
}

struct LoadedCatalog {
    products: Vec<CatalogProduct>,
    warnings: Vec<String>,
}

struct LoadedCycles {
    cycles: Vec<Cycle>,
    warnings: Vec<String>,
}

fn load_catalog(
    client: &impl HttpClient,
    options: &EolOptions<'_>,
) -> Result<LoadedCatalog, String> {
    let path = options.cache_dir.join("eol").join("products.json");
    let policy = policy(options);
    match cache::choose::<ProductsFile>(&path, policy, ShapeFault::Error(PRODUCTS_SHAPE))? {
        CacheChoice::Fresh(file) => Ok(loaded_products(file.products, Vec::new())),
        CacheChoice::Stale(file) => Ok(loaded_products(
            file.products,
            vec![cache::offline_cache_warning(file.stored_at)],
        )),
        CacheChoice::Unavailable => {
            Err("endoflife.date product list is not in the cache".to_owned())
        }
        CacheChoice::Fetch => match get_json(client, LIST_URL) {
            Ok(body) => {
                let products = parse_product_list(&body)?;
                let _ = cache::write_stored(
                    &path,
                    &ProductsFile {
                        stored_at: Some(options.now),
                        products: products.clone(),
                    },
                    false,
                );
                Ok(loaded_products(products, Vec::new()))
            }
            Err(error) => {
                match cache::read_shaped::<ProductsFile>(&path, ShapeFault::Error(PRODUCTS_SHAPE))?
                {
                    Some(file) => Ok(loaded_products(
                        file.products,
                        vec![cache::network_cache_warning(file.stored_at)],
                    )),
                    None => Err(format!("endoflife.date product list: {error}")),
                }
            }
        },
    }
}

fn load_cycles(
    client: &impl HttpClient,
    options: &EolOptions<'_>,
    product: &str,
) -> Result<LoadedCycles, String> {
    let path = options
        .cache_dir
        .join("eol")
        .join("products")
        .join(format!("{}.json", encode_file_name(product)));
    let policy = policy(options);
    match cache::choose::<CyclesFile>(&path, policy, ShapeFault::Error(CYCLES_SHAPE))? {
        CacheChoice::Fresh(file) => {
            let (_stored_at, cycles) = cycles_from(file);
            Ok(loaded_cycles(cycles, Vec::new()))
        }
        CacheChoice::Stale(file) => {
            let (stored_at, cycles) = cycles_from(file);
            Ok(loaded_cycles(
                cycles,
                vec![cache::offline_cache_warning(stored_at)],
            ))
        }
        CacheChoice::Unavailable => Err(format!(
            "endoflife.date product {product} is not in the cache"
        )),
        CacheChoice::Fetch => {
            let url = format!("{PRODUCT_URL}{}/", encode(product));
            match get_json(client, &url) {
                Ok(body) => {
                    let cycles = parse_product(&body)?;
                    let _ = cache::write_stored(
                        &path,
                        &StoredCycles {
                            stored_at: options.now,
                            cycles: &cycles,
                        },
                        false,
                    );
                    Ok(loaded_cycles(cycles, Vec::new()))
                }
                Err(error) => {
                    match cache::read_shaped::<CyclesFile>(&path, ShapeFault::Error(CYCLES_SHAPE))?
                    {
                        Some(file) => {
                            let (stored_at, cycles) = cycles_from(file);
                            Ok(loaded_cycles(
                                cycles,
                                vec![cache::network_cache_warning(stored_at)],
                            ))
                        }
                        None => Err(format!("endoflife.date product {product}: {error}")),
                    }
                }
            }
        }
    }
}

const PRODUCTS_SHAPE: &str = "cached product list is not an object";
const CYCLES_SHAPE: &str = "cached product is not an object";

fn policy(options: &EolOptions<'_>) -> CachePolicy {
    CachePolicy {
        now: options.now,
        ttl_seconds: TTL_SECONDS,
        refresh: options.refresh,
        offline: options.offline,
    }
}

fn loaded_products(products: Vec<CatalogProduct>, warnings: Vec<String>) -> LoadedCatalog {
    LoadedCatalog { products, warnings }
}

fn loaded_cycles(cycles: Vec<Cycle>, warnings: Vec<String>) -> LoadedCycles {
    LoadedCycles { cycles, warnings }
}

fn cycles_from(file: CyclesFile) -> (Option<i64>, Vec<Cycle>) {
    (
        file.stored_at,
        file.cycles.into_iter().map(Cycle::from).collect(),
    )
}

#[derive(Serialize, Deserialize)]
struct ProductsFile {
    #[serde(default)]
    stored_at: Option<i64>,
    #[serde(deserialize_with = "lenient_array")]
    products: Vec<CatalogProduct>,
}

#[derive(Serialize)]
struct StoredCycles<'a> {
    stored_at: i64,
    cycles: &'a [Cycle],
}

#[derive(Deserialize)]
struct CyclesFile {
    #[serde(default)]
    stored_at: Option<i64>,
    #[serde(deserialize_with = "lenient_array")]
    cycles: Vec<RawCycle>,
}

/// One cycle from the API (`isEol` / `eolFrom`) or from our cache (`is_eol` / `eol_from`).
#[derive(Deserialize)]
struct RawCycle {
    name: String,
    #[serde(alias = "isEol", default, deserialize_with = "optional_bool")]
    is_eol: Option<bool>,
    #[serde(alias = "eolFrom", default)]
    eol_from: EolFrom,
}

impl TimedCache for ProductsFile {
    fn stored_at(&self) -> Option<i64> {
        self.stored_at
    }
}

impl TimedCache for CyclesFile {
    fn stored_at(&self) -> Option<i64> {
        self.stored_at
    }
}

impl From<RawCycle> for Cycle {
    fn from(cycle: RawCycle) -> Self {
        let (eol_from, eol_from_invalid) = cycle.eol_from.into_parts();
        Self {
            name: cycle.name,
            is_eol: cycle.is_eol,
            eol_from,
            eol_from_invalid,
        }
    }
}

/// Matches the previous Value walk: null is fine, a calendar day is kept, anything else is unusable.
#[derive(Debug, Clone, Default)]
enum EolFrom {
    #[default]
    Absent,
    Null,
    Valid(String),
    Invalid,
}

impl EolFrom {
    fn into_parts(self) -> (Option<String>, bool) {
        match self {
            Self::Valid(text) => (Some(text), false),
            Self::Null => (None, false),
            Self::Absent | Self::Invalid => (None, true),
        }
    }
}

impl<'de> Deserialize<'de> for EolFrom {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(match Value::deserialize(deserializer)? {
            Value::Null => Self::Null,
            Value::String(text) if is_calendar_date(&text) => Self::Valid(text),
            _ => Self::Invalid,
        })
    }
}

fn parse_product_list(body: &[u8]) -> Result<Vec<CatalogProduct>, String> {
    let list: ProductList = serde_json::from_slice(body).map_err(|error| {
        if error.is_syntax() || error.is_eof() {
            error.to_string()
        } else {
            "endoflife.date product list has no result".to_owned()
        }
    })?;
    list.result
        .ok_or_else(|| "endoflife.date product list has no result".to_owned())
}

fn parse_product(body: &[u8]) -> Result<Vec<Cycle>, String> {
    let document: ProductDocument = serde_json::from_slice(body).map_err(|error| {
        if error.is_syntax() || error.is_eof() {
            error.to_string()
        } else {
            "endoflife.date product has no result".to_owned()
        }
    })?;
    let Some(result) = document.result else {
        return Err("endoflife.date product has no result".to_owned());
    };
    let Some(releases) = result.releases else {
        return Err("endoflife.date product has no releases".to_owned());
    };
    Ok(releases.into_iter().map(Cycle::from).collect())
}

fn get_json(client: &impl HttpClient, url: &str) -> Result<Vec<u8>, String> {
    let response = client
        .execute(&HttpRequest {
            method: Method::Get,
            url,
            body: None,
        })
        .map_err(|error| error.message)?;
    accepted_body_or(
        response,
        || format!("response is larger than {MAX_RESPONSE_BYTES} bytes"),
        |status| format!("HTTP status {status}"),
    )
}

#[derive(Deserialize)]
struct ProductList {
    #[serde(default, deserialize_with = "optional_lenient_vec")]
    result: Option<Vec<CatalogProduct>>,
}

#[derive(Deserialize)]
struct ProductDocument {
    #[serde(default)]
    result: Option<ProductResult>,
}

#[derive(Deserialize)]
struct ProductResult {
    #[serde(default, deserialize_with = "optional_lenient_vec")]
    releases: Option<Vec<RawCycle>>,
}
