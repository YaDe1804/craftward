//! End-of-life checks against the endoflife.date API v1.
//!
//! The product list is `GET https://endoflife.date/api/v1/products/`.
//! One product, including its release cycles, is
//! `GET https://endoflife.date/api/v1/products/{name}/`.
//! The path contains only the product name. The API version in the
//! specification is 1.2.1. The data is under the MIT License.

use std::fmt::Write as _;
use std::path::Path;

use serde::Serialize;
use serde_json::Value;

use crate::date::{days_between, format_unix_timestamp, is_calendar_date};
use crate::http::{HttpClient, HttpError, HttpRequest, HttpResponse, MAX_RESPONSE_BYTES};
use crate::model::Component;
use crate::workspace::write_atomic;

const LIST_URL: &str = "https://endoflife.date/api/v1/products/";
const PRODUCT_URL: &str = "https://endoflife.date/api/v1/products/";
const TTL_SECONDS: i64 = 24 * 60 * 60;

/// One product name from the product list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogProduct {
    /// Product id used in the API path.
    pub name: String,
    /// Alternate names from the product summary.
    pub aliases: Vec<String>,
}

/// One release cycle and the end-of-life fields from API v1.
#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub kind: String,
    /// `high` for `eol`, `medium` for `eol_soon`.
    pub severity: String,
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
            Some(distance) if distance < 0 => {
                CycleEffect::Hit(hit(product, version, cycle, "eol", "high", date))
            }
            Some(_) => contradictory(product, &cycle.name),
            None => unusable(product, &cycle.name),
        },
        (true, None) => CycleEffect::Hit(hit(product, version, cycle, "eol", "high", "unknown")),
        (false, Some(date)) => match days_between(today, date) {
            Some(distance) if distance < 0 => contradictory(product, &cycle.name),
            Some(distance) if distance <= i64::from(warning_days) => {
                CycleEffect::Hit(hit(product, version, cycle, "eol_soon", "medium", date))
            }
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
    kind: &str,
    severity: &str,
    date: &str,
) -> EolHit {
    let verb = if kind == "eol" { "ended" } else { "ends" };
    EolHit {
        id: format!("EOL:{product}:{}", cycle.name),
        product: product.to_owned(),
        version: version.to_owned(),
        kind: kind.to_owned(),
        severity: severity.to_owned(),
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
    match use_cache(&path, options, "endoflife.date product list")? {
        CacheRead::Fresh(value) | CacheRead::Stale(value) => Ok(LoadedCatalog {
            products: parse_cached_products(&value)?,
            warnings: stale_warning(&value, options.now),
        }),
        CacheRead::Fetch => match get_json(client, LIST_URL) {
            Ok(body) => {
                let products = parse_product_list(&body)?;
                write_cached(
                    &path,
                    &CachedProducts {
                        stored_at: options.now,
                        products: &products,
                    },
                );
                Ok(LoadedCatalog {
                    products,
                    warnings: Vec::new(),
                })
            }
            Err(error) => match read_cached(&path) {
                Some(value) => Ok(LoadedCatalog {
                    products: parse_cached_products(&value)?,
                    warnings: vec![stale_network(&value)],
                }),
                None => Err(format!("endoflife.date product list: {error}")),
            },
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
        .join(format!("{}.json", encode_file(product)));
    match use_cache(&path, options, &format!("endoflife.date product {product}"))? {
        CacheRead::Fresh(value) | CacheRead::Stale(value) => Ok(LoadedCycles {
            cycles: parse_cached_cycles(&value)?,
            warnings: stale_warning(&value, options.now),
        }),
        CacheRead::Fetch => {
            let url = format!("{PRODUCT_URL}{}/", encode_path(product));
            match get_json(client, &url) {
                Ok(body) => {
                    let cycles = parse_product(&body)?;
                    write_cached(
                        &path,
                        &CachedCycles {
                            stored_at: options.now,
                            cycles: &cycles,
                        },
                    );
                    Ok(LoadedCycles {
                        cycles,
                        warnings: Vec::new(),
                    })
                }
                Err(error) => match read_cached(&path) {
                    Some(value) => Ok(LoadedCycles {
                        cycles: parse_cached_cycles(&value)?,
                        warnings: vec![stale_network(&value)],
                    }),
                    None => Err(format!("endoflife.date product {product}: {error}")),
                },
            }
        }
    }
}

enum CacheRead {
    Fresh(Value),
    Stale(Value),
    Fetch,
}

fn use_cache(path: &Path, options: &EolOptions<'_>, label: &str) -> Result<CacheRead, String> {
    if options.refresh && !options.offline {
        return Ok(CacheRead::Fetch);
    }
    match read_cached(path) {
        Some(value) if fresh(&value, options.now) => Ok(CacheRead::Fresh(value)),
        Some(value) if options.offline => Ok(CacheRead::Stale(value)),
        Some(_) if !options.offline => Ok(CacheRead::Fetch),
        None if options.offline => Err(format!("{label} is not in the cache")),
        _ => Ok(CacheRead::Fetch),
    }
}

fn fresh(value: &Value, now: i64) -> bool {
    value
        .get("stored_at")
        .and_then(Value::as_i64)
        .is_some_and(|stored| now.saturating_sub(stored) < TTL_SECONDS)
}

fn stale_warning(value: &Value, now: i64) -> Vec<String> {
    if fresh(value, now) {
        Vec::new()
    } else {
        vec![stale_offline(value)]
    }
}

fn stale_offline(value: &Value) -> String {
    match stored_stamp(value) {
        Some(when) => format!("using cached results from {when}"),
        None => "using expired cached results".to_owned(),
    }
}

fn stale_network(value: &Value) -> String {
    match stored_stamp(value) {
        Some(when) => format!("using cached results from {when} after a network error"),
        None => "using cached results after a network error".to_owned(),
    }
}

fn stored_stamp(value: &Value) -> Option<String> {
    format_unix_timestamp(value.get("stored_at").and_then(Value::as_i64)?)
}

fn read_cached(path: &Path) -> Option<Value> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn write_cached(path: &Path, value: &impl Serialize) {
    let Ok(bytes) = serde_json::to_vec_pretty(value) else {
        return;
    };
    let _ = write_atomic(path, &bytes);
}

#[derive(Serialize)]
struct CachedProducts<'a> {
    stored_at: i64,
    products: &'a [CatalogProduct],
}

#[derive(Serialize)]
struct CachedCycles<'a> {
    stored_at: i64,
    cycles: &'a [Cycle],
}

impl Serialize for CatalogProduct {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("CatalogProduct", 2)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("aliases", &self.aliases)?;
        state.end()
    }
}

impl Serialize for Cycle {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("Cycle", 4)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field("is_eol", &self.is_eol)?;
        state.serialize_field("eol_from", &self.eol_from)?;
        state.serialize_field("eol_from_invalid", &self.eol_from_invalid)?;
        state.end()
    }
}

fn parse_cached_products(value: &Value) -> Result<Vec<CatalogProduct>, String> {
    let items = value
        .get("products")
        .and_then(Value::as_array)
        .ok_or_else(|| "cached product list is not an object".to_owned())?;
    Ok(items.iter().filter_map(cached_product).collect())
}

fn cached_product(value: &Value) -> Option<CatalogProduct> {
    let object = value.as_object()?;
    Some(CatalogProduct {
        name: object.get("name")?.as_str()?.to_owned(),
        aliases: string_list(object.get("aliases")),
    })
}

fn parse_cached_cycles(value: &Value) -> Result<Vec<Cycle>, String> {
    let items = value
        .get("cycles")
        .and_then(Value::as_array)
        .ok_or_else(|| "cached product is not an object".to_owned())?;
    Ok(items.iter().filter_map(cached_cycle).collect())
}

fn cached_cycle(value: &Value) -> Option<Cycle> {
    let object = value.as_object()?;
    let name = object.get("name")?.as_str()?.to_owned();
    Some(cycle_from_fields(
        name,
        object.get("is_eol"),
        object.get("eol_from"),
    ))
}

fn parse_product_list(body: &[u8]) -> Result<Vec<CatalogProduct>, String> {
    let value: Value = serde_json::from_slice(body).map_err(|error| error.to_string())?;
    let items = value
        .get("result")
        .and_then(Value::as_array)
        .ok_or_else(|| "endoflife.date product list has no result".to_owned())?;
    Ok(items.iter().filter_map(cached_product).collect())
}

fn parse_product(body: &[u8]) -> Result<Vec<Cycle>, String> {
    let value: Value = serde_json::from_slice(body).map_err(|error| error.to_string())?;
    let result = value
        .get("result")
        .and_then(Value::as_object)
        .ok_or_else(|| "endoflife.date product has no result".to_owned())?;
    let items = result
        .get("releases")
        .and_then(Value::as_array)
        .ok_or_else(|| "endoflife.date product has no releases".to_owned())?;
    Ok(items
        .iter()
        .filter_map(|item| {
            let object = item.as_object()?;
            let name = object.get("name")?.as_str()?.to_owned();
            Some(cycle_from_fields(
                name,
                object.get("isEol"),
                object.get("eolFrom"),
            ))
        })
        .collect())
}

fn cycle_from_fields(name: String, eol_flag: Option<&Value>, eol_date: Option<&Value>) -> Cycle {
    let is_eol = eol_flag.and_then(Value::as_bool);
    let (eol_from, eol_from_invalid) = match eol_date {
        Some(Value::Null) => (None, false),
        Some(Value::String(text)) if is_calendar_date(text) => (Some(text.clone()), false),
        _ => (None, true),
    };
    Cycle {
        name,
        is_eol,
        eol_from,
        eol_from_invalid,
    }
}

fn string_list(value: Option<&Value>) -> Vec<String> {
    value.and_then(Value::as_array).map_or_default(|items| {
        items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect()
    })
}

fn get_json(client: &impl HttpClient, url: &str) -> Result<Vec<u8>, String> {
    let response = client
        .execute(&HttpRequest {
            method: "GET",
            url,
            body: None,
        })
        .map_err(|error| error.message)?;
    accepted(response).map_err(|error| error.message)
}

fn accepted(response: HttpResponse) -> Result<Vec<u8>, HttpError> {
    if response.body.len() > MAX_RESPONSE_BYTES {
        return Err(HttpError {
            message: format!("response is larger than {MAX_RESPONSE_BYTES} bytes"),
        });
    }
    if response.status == 200 {
        return Ok(response.body);
    }
    Err(HttpError {
        message: format!("HTTP status {}", response.status),
    })
}

fn encode_path(name: &str) -> String {
    let mut encoded = String::new();
    for byte in name.bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' || byte == b'.' {
            encoded.push(char::from(byte));
        } else {
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

fn encode_file(name: &str) -> String {
    encode_path(name).replace('.', "_")
}
