//! Reads Xray's geosite/geoip `.dat` databases for the routing-rule browser.
//!
//! The files are protobuf, but materialising every rule would cost hundreds of
//! megabytes on a small VPS. The scanners therefore walk the wire format and
//! keep only the category index (cached) or the requested page.

use std::{
    collections::{HashMap, HashSet},
    env, fs,
    net::{Ipv4Addr, Ipv6Addr},
    path::{Component, Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::UNIX_EPOCH,
};

use serde::Serialize;

/// Xray also probes this directory for geoip.dat/geosite.dat.
pub const DEFAULT_ASSET_DIR: &str = "/usr/local/share/xray";

const MAX_FILE_SIZE: u64 = 128 << 20;
const MAX_PAGE_SIZE: usize = 500;
const MAX_INDEX_SIZE: usize = 1 << 16;
const MAX_CACHED_INDEXES: usize = 8;
const TOO_LARGE: &str = "geodata file is too large to browse";
const INDEX_TOO_LARGE: &str = "geodata category index is too large to browse";
const UNRECOGNIZED: &str = "file is not a geosite or geoip database";
const INVALID_NAME: &str = "invalid geodata file name";
const UNKNOWN_CATEGORY: &str = "unknown geodata category";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Site,
    Ip,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Site => "site",
            Self::Ip => "ip",
        }
    }

    fn other(self) -> Self {
        match self {
            Self::Site => Self::Ip,
            Self::Ip => Self::Site,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeoFile {
    pub name: String,
    pub kind: String,
    pub size: u64,
    pub modified_at: u64,
    pub categories: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GeoCategory {
    pub code: String,
    pub entries: usize,
    pub attributes: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GeoEntry {
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct GeoCategoryPage {
    pub total: usize,
    pub items: Vec<GeoCategory>,
}

#[derive(Debug, Serialize)]
pub struct GeoEntryPage {
    pub total: usize,
    pub items: Vec<GeoEntry>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenIssue {
    pub token: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

struct Scan {
    kind: Kind,
    categories: Vec<GeoCategory>,
    by_code: HashMap<String, usize>,
    usable: usize,
}

struct GeoIndex {
    kind: Kind,
    categories: Vec<GeoCategory>,
    by_code: HashMap<String, usize>,
}

struct CachedIndex {
    path: PathBuf,
    size: u64,
    modified: u128,
    index: Arc<GeoIndex>,
}

static INDEX_CACHE: OnceLock<Mutex<Vec<CachedIndex>>> = OnceLock::new();
static SCAN_LOCK: Mutex<()> = Mutex::new(());

fn cache() -> &'static Mutex<Vec<CachedIndex>> {
    INDEX_CACHE.get_or_init(|| Mutex::new(Vec::new()))
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Directory Xray reads its asset files from. `XRAY_LOCATION_ASSET` wins when
/// set; the Xray default `/usr/local/share/xray` is the fallback.
pub fn asset_dir() -> PathBuf {
    env::var("XRAY_LOCATION_ASSET")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_ASSET_DIR))
}

pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn asset() -> Self {
        Self::new(asset_dir())
    }

    pub fn files(&self) -> Result<Vec<GeoFile>, String> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(format!("读取 Geodata 目录失败：{err}")),
        };
        let mut files = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.to_ascii_lowercase().ends_with(".dat") {
                continue;
            }
            let meta = match entry.metadata() {
                Ok(meta) if meta.is_file() => meta,
                _ => continue,
            };
            let mut file = GeoFile {
                name: name.clone(),
                kind: String::new(),
                size: meta.len(),
                modified_at: modified_millis(&meta),
                categories: 0,
                error: None,
            };
            match self.index(&name) {
                Ok(index) => {
                    file.kind = index.kind.as_str().to_string();
                    file.categories = index.categories.len();
                }
                Err(err) => file.error = Some(err),
            }
            files.push(file);
        }
        files.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(files)
    }

    pub fn categories(
        &self,
        name: &str,
        query: &str,
        offset: usize,
        limit: usize,
    ) -> Result<GeoCategoryPage, String> {
        let index = self.index(name)?;
        let query = query.trim().to_lowercase();
        let matched: Vec<GeoCategory> = index
            .categories
            .iter()
            .filter(|category| query.is_empty() || category.code.contains(&query))
            .cloned()
            .collect();
        let total = matched.len();
        let (from, to) = bounds(total, offset, limit);
        Ok(GeoCategoryPage {
            total,
            items: matched[from..to].to_vec(),
        })
    }

    pub fn entries(
        &self,
        name: &str,
        code: &str,
        query: &str,
        offset: usize,
        limit: usize,
    ) -> Result<GeoEntryPage, String> {
        let index = self.index(name)?;
        let code = code.trim().to_lowercase();
        if !index.by_code.contains_key(&code) {
            return Err(UNKNOWN_CATEGORY.into());
        }
        let (path, _) = resolve(&self.dir, name)?;
        let limit = if limit == 0 || limit > MAX_PAGE_SIZE {
            MAX_PAGE_SIZE
        } else {
            limit
        };
        let _guard = lock(&SCAN_LOCK);
        let bytes = fs::read(&path).map_err(|err| format!("读取 Geodata 文件失败：{err}"))?;
        scan_entries(
            &bytes,
            index.kind,
            &code,
            &query.to_lowercase(),
            offset,
            limit,
        )
    }

    pub fn validate(&self, is_ip: bool, tokens: &[String]) -> Vec<TokenIssue> {
        tokens
            .iter()
            .filter(|token| !token.trim().is_empty())
            .filter_map(|token| self.validate_one(token.trim(), is_ip))
            .collect()
    }

    fn validate_one(&self, token: &str, is_ip: bool) -> Option<TokenIssue> {
        let reference = match parse_reference(token, is_ip) {
            Ok(reference) => reference,
            Err(TokenError::WrongKind) => {
                return Some(TokenIssue {
                    token: token.into(),
                    reason: "wrongKind".into(),
                    file: None,
                    code: None,
                })
            }
            Err(TokenError::Syntax) => {
                return Some(TokenIssue {
                    token: token.into(),
                    reason: "syntax".into(),
                    file: None,
                    code: None,
                })
            }
        };
        let file = reference.file?;
        let index = match self.index(&file) {
            Ok(index) => index,
            Err(_) => {
                return Some(TokenIssue {
                    token: token.into(),
                    reason: "fileMissing".into(),
                    file: Some(file),
                    code: Some(reference.code),
                })
            }
        };
        let Some(&position) = index.by_code.get(&reference.code) else {
            return Some(TokenIssue {
                token: token.into(),
                reason: "categoryMissing".into(),
                file: Some(file),
                code: Some(reference.code),
            });
        };
        let category = &index.categories[position];
        let missing = unknown_attribute(category, &reference.attributes)?;
        Some(TokenIssue {
            token: token.into(),
            reason: "attributeMissing".into(),
            file: Some(file),
            code: Some(missing),
        })
    }

    fn index(&self, name: &str) -> Result<Arc<GeoIndex>, String> {
        let (path, meta) = resolve(&self.dir, name)?;
        let key = (path.clone(), meta.len(), modified_nanos(&meta));
        if let Some(index) = cached(&key) {
            return Ok(index);
        }
        let _guard = lock(&SCAN_LOCK);
        if let Some(index) = cached(&key) {
            return Ok(index);
        }
        let bytes = fs::read(&path).map_err(|err| format!("读取 Geodata 文件失败：{err}"))?;
        let scan = detect_kind(&bytes, name)?;
        let index = Arc::new(GeoIndex {
            kind: scan.kind,
            categories: scan.categories,
            by_code: scan.by_code,
        });
        let mut entries = lock(cache());
        entries.retain(|cached| cached.path != key.0);
        entries.insert(
            0,
            CachedIndex {
                path: key.0,
                size: key.1,
                modified: key.2,
                index: index.clone(),
            },
        );
        entries.truncate(MAX_CACHED_INDEXES);
        Ok(index)
    }
}

fn cached((path, size, modified): &(PathBuf, u64, u128)) -> Option<Arc<GeoIndex>> {
    lock(cache())
        .iter()
        .find(|entry| entry.path == *path && entry.size == *size && entry.modified == *modified)
        .map(|entry| entry.index.clone())
}

fn modified_millis(meta: &fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn modified_nanos(meta: &fs::Metadata) -> u128 {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or(0)
}

fn bounds(total: usize, offset: usize, limit: usize) -> (usize, usize) {
    let from = offset.min(total);
    if limit == 0 {
        return (from, total);
    }
    (from, (from + limit.min(MAX_PAGE_SIZE)).min(total))
}

fn resolve(dir: &Path, name: &str) -> Result<(PathBuf, fs::Metadata), String> {
    let path = Path::new(name);
    let mut components = path.components();
    let simple =
        matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none();
    if name.is_empty() || !simple || !name.to_ascii_lowercase().ends_with(".dat") {
        return Err(INVALID_NAME.into());
    }
    let root = dir
        .canonicalize()
        .map_err(|_| "Geodata 目录不存在".to_string())?;
    let target = dir
        .join(name)
        .canonicalize()
        .map_err(|_| format!("Geodata 文件不存在：{name}"))?;
    if !target.starts_with(&root) {
        return Err(INVALID_NAME.into());
    }
    let meta = fs::metadata(&target).map_err(|err| format!("读取 Geodata 文件失败：{err}"))?;
    if !meta.is_file() {
        return Err(INVALID_NAME.into());
    }
    if meta.len() > MAX_FILE_SIZE {
        return Err(TOO_LARGE.into());
    }
    Ok((target, meta))
}

fn detect_kind(data: &[u8], name: &str) -> Result<Scan, String> {
    let first = if name.to_ascii_lowercase().contains("ip") {
        Kind::Ip
    } else {
        Kind::Site
    };
    let mut first_error = None;
    for kind in [first, first.other()] {
        match scan_index(data, kind) {
            Ok(mut scan) if scan.usable > 0 => {
                scan.kind = kind;
                return Ok(scan);
            }
            Ok(_) => {}
            Err(err) if err == INDEX_TOO_LARGE => return Err(err),
            Err(err) => first_error = first_error.or(Some(err)),
        }
    }
    Err(match first_error {
        Some(err) => format!("{UNRECOGNIZED}: {err}"),
        None => UNRECOGNIZED.into(),
    })
}

fn scan_index(data: &[u8], kind: Kind) -> Result<Scan, String> {
    let mut categories: Vec<GeoCategory> = Vec::new();
    let mut by_code: HashMap<String, usize> = HashMap::new();
    let mut attribute_sets: Vec<HashSet<String>> = Vec::new();
    let mut index_size = 0usize;
    let mut usable = 0usize;

    each_list_entry(data, |entry| {
        let mut count = 0usize;
        let mut attributes = HashSet::new();
        let code = entry_code(entry)?;
        each_payload(entry, |payload| {
            match kind {
                Kind::Site => {
                    let (value, found) = domain_value(payload)?;
                    if value.is_some_and(|value| !value.is_empty()) {
                        attributes.extend(found);
                        if attributes.len() > MAX_INDEX_SIZE {
                            return Err(INDEX_TOO_LARGE.into());
                        }
                        count += 1;
                    }
                }
                Kind::Ip => {
                    if cidr_value(payload)?.is_some() {
                        count += 1;
                    }
                }
            }
            Ok(())
        })?;
        if code.is_empty() {
            return Ok(());
        }
        index_size += 1 + attributes.len();
        if index_size > MAX_INDEX_SIZE {
            return Err(INDEX_TOO_LARGE.into());
        }
        usable += count;
        if let Some(&position) = by_code.get(&code) {
            categories[position].entries += count;
            attribute_sets[position].extend(attributes);
        } else {
            by_code.insert(code.clone(), categories.len());
            categories.push(GeoCategory {
                code,
                entries: count,
                attributes: Vec::new(),
            });
            attribute_sets.push(attributes);
        }
        Ok(())
    })?;

    for (position, attributes) in attribute_sets.into_iter().enumerate() {
        let mut list: Vec<String> = attributes.into_iter().collect();
        list.sort();
        categories[position].attributes = list;
    }
    categories.sort_by(|a, b| a.code.cmp(&b.code));
    let by_code = categories
        .iter()
        .enumerate()
        .map(|(position, category)| (category.code.clone(), position))
        .collect();
    Ok(Scan {
        kind,
        categories,
        by_code,
        usable,
    })
}

fn scan_entries(
    data: &[u8],
    kind: Kind,
    code: &str,
    query: &str,
    offset: usize,
    limit: usize,
) -> Result<GeoEntryPage, String> {
    let mut items = Vec::new();
    let mut total = 0usize;
    each_list_entry(data, |entry| {
        if entry_code(entry)? != code {
            return Ok(());
        }
        each_payload(entry, |payload| {
            let (entry_kind, text) = match kind {
                Kind::Site => {
                    let (value, _) = domain_value(payload)?;
                    let Some(value) = value.filter(|value| !value.is_empty()) else {
                        return Ok(());
                    };
                    (
                        domain_kind(payload),
                        String::from_utf8_lossy(value).into_owned(),
                    )
                }
                Kind::Ip => {
                    let Some(value) = cidr_value(payload)? else {
                        return Ok(());
                    };
                    ("cidr", value)
                }
            };
            if !query.is_empty() && !text.to_lowercase().contains(query) {
                return Ok(());
            }
            if total >= offset && items.len() < limit {
                items.push(GeoEntry {
                    kind: entry_kind.to_string(),
                    value: text,
                });
            }
            total += 1;
            Ok(())
        })
    })?;
    Ok(GeoEntryPage { total, items })
}

fn each_list_entry<'a>(
    data: &'a [u8],
    mut visit: impl FnMut(&'a [u8]) -> Result<(), String>,
) -> Result<(), String> {
    let mut position = 0usize;
    while position < data.len() {
        let (number, wire, consumed) = read_tag(&data[position..])?;
        let start = position + consumed;
        if number == 1 && wire == 2 {
            let (entry, used) = read_bytes(&data[start..])?;
            visit(entry)?;
            position = start + used;
        } else {
            position = start + skip_field(wire, &data[start..])?;
        }
    }
    Ok(())
}

fn entry_code(entry: &[u8]) -> Result<String, String> {
    let mut position = 0usize;
    while position < entry.len() {
        let (number, wire, consumed) = read_tag(&entry[position..])?;
        let start = position + consumed;
        match (number, wire) {
            (1, 2) => {
                let (raw, _) = read_bytes(&entry[start..])?;
                return Ok(String::from_utf8_lossy(raw).to_lowercase());
            }
            _ => position = start + skip_field(wire, &entry[start..])?,
        }
    }
    Ok(String::new())
}

fn each_payload(
    entry: &[u8],
    mut visit: impl FnMut(&[u8]) -> Result<(), String>,
) -> Result<(), String> {
    let mut position = 0usize;
    while position < entry.len() {
        let (number, wire, consumed) = read_tag(&entry[position..])?;
        let start = position + consumed;
        if number == 2 && wire == 2 {
            let (payload, used) = read_bytes(&entry[start..])?;
            visit(payload)?;
            position = start + used;
        } else {
            position = start + skip_field(wire, &entry[start..])?;
        }
    }
    Ok(())
}

fn domain_value(payload: &[u8]) -> Result<(Option<&[u8]>, Vec<String>), String> {
    let mut value = None;
    let mut attributes = Vec::new();
    let mut position = 0usize;
    while position < payload.len() {
        let (number, wire, consumed) = read_tag(&payload[position..])?;
        let start = position + consumed;
        match (number, wire) {
            (2, 2) => {
                let (raw, used) = read_bytes(&payload[start..])?;
                value = Some(raw);
                position = start + used;
            }
            (3, 2) => {
                let (raw, used) = read_bytes(&payload[start..])?;
                if let Some(key) = attribute_key(raw) {
                    attributes.push(key);
                }
                position = start + used;
            }
            _ => position = start + skip_field(wire, &payload[start..])?,
        }
    }
    Ok((value, attributes))
}

fn attribute_key(attribute: &[u8]) -> Option<String> {
    let mut position = 0usize;
    while position < attribute.len() {
        let (number, wire, consumed) = read_tag(&attribute[position..]).ok()?;
        let start = position + consumed;
        if number == 1 && wire == 2 {
            let (raw, _) = read_bytes(&attribute[start..]).ok()?;
            return Some(String::from_utf8_lossy(raw).to_lowercase());
        }
        position = start + skip_field(wire, &attribute[start..]).ok()?;
    }
    None
}

fn domain_kind(payload: &[u8]) -> &'static str {
    let mut match_type = 0u64;
    let mut position = 0usize;
    while position < payload.len() {
        let Ok((number, wire, consumed)) = read_tag(&payload[position..]) else {
            break;
        };
        let start = position + consumed;
        if number == 1 && wire == 0 {
            match read_varint(&payload[start..]) {
                Ok((value, _)) => match_type = value,
                Err(_) => break,
            }
            break;
        }
        match skip_field(wire, &payload[start..]) {
            Ok(used) => position = start + used,
            Err(_) => break,
        }
    }
    match match_type {
        0 => "keyword",
        1 => "regexp",
        3 => "full",
        _ => "domain",
    }
}

fn cidr_value(payload: &[u8]) -> Result<Option<String>, String> {
    let mut address: Option<&[u8]> = None;
    let mut prefix = 0u64;
    let mut position = 0usize;
    while position < payload.len() {
        let (number, wire, consumed) = read_tag(&payload[position..])?;
        let start = position + consumed;
        match (number, wire) {
            (1, 2) => {
                let (raw, used) = read_bytes(&payload[start..])?;
                address = Some(raw);
                position = start + used;
            }
            (2, 0) => {
                let (value, used) = read_varint(&payload[start..])?;
                prefix = value;
                position = start + used;
            }
            _ => position = start + skip_field(wire, &payload[start..])?,
        }
    }
    let Some(address) = address else {
        return Ok(None);
    };
    let rendered = match address.len() {
        4 if prefix <= 32 => format!(
            "{}/{}",
            Ipv4Addr::new(address[0], address[1], address[2], address[3]),
            prefix
        ),
        16 if prefix <= 128 => {
            let mut octets = [0u8; 16];
            octets.copy_from_slice(address);
            format!("{}/{}", Ipv6Addr::from(octets), prefix)
        }
        _ => return Ok(None),
    };
    Ok(Some(rendered))
}

fn read_tag(data: &[u8]) -> Result<(u32, u8, usize), String> {
    let (tag, consumed) = read_varint(data)?;
    if tag == 0 || tag > u32::MAX as u64 {
        return Err("invalid protobuf tag".into());
    }
    Ok(((tag >> 3) as u32, (tag & 7) as u8, consumed))
}

fn read_varint(data: &[u8]) -> Result<(u64, usize), String> {
    let mut value = 0u64;
    for (position, byte) in data.iter().copied().enumerate().take(10) {
        value |= u64::from(byte & 0x7f) << (position * 7);
        if byte & 0x80 == 0 {
            return Ok((value, position + 1));
        }
    }
    Err("truncated protobuf varint".into())
}

fn read_bytes(data: &[u8]) -> Result<(&[u8], usize), String> {
    let (length, consumed) = read_varint(data)?;
    let length = usize::try_from(length).map_err(|_| "invalid protobuf length".to_string())?;
    let end = consumed
        .checked_add(length)
        .filter(|end| *end <= data.len())
        .ok_or_else(|| "truncated protobuf bytes".to_string())?;
    Ok((&data[consumed..end], end))
}

fn skip_field(wire: u8, data: &[u8]) -> Result<usize, String> {
    match wire {
        0 => Ok(read_varint(data)?.1),
        1 => (data.len() >= 8)
            .then_some(8)
            .ok_or_else(|| "truncated protobuf fixed64".to_string()),
        2 => Ok(read_bytes(data)?.1),
        5 => (data.len() >= 4)
            .then_some(4)
            .ok_or_else(|| "truncated protobuf fixed32".to_string()),
        _ => Err("unsupported protobuf wire type".into()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenError {
    Syntax,
    WrongKind,
}

struct Reference {
    file: Option<String>,
    code: String,
    attributes: Vec<String>,
}

fn parse_reference(token: &str, is_ip: bool) -> Result<Reference, TokenError> {
    let mut token = token.trim();
    if token.is_empty() {
        return Err(TokenError::Syntax);
    }
    if is_ip {
        token = cut_negation(token).0;
    }

    let (shorthand, default_file, prefixes): (&str, &str, &[&str]) = if is_ip {
        ("geoip:", "geoip.dat", &["ext:", "ext-ip:"])
    } else {
        (
            "geosite:",
            "geosite.dat",
            &["ext:", "ext-domain:", "ext-site:"],
        )
    };
    let expanded;
    if let Some(rest) = token.strip_prefix(shorthand) {
        expanded = format!("ext:{default_file}:{rest}");
        token = &expanded;
    }

    let other = if is_ip { "geosite:" } else { "geoip:" };
    if token.starts_with(other) {
        return Err(TokenError::WrongKind);
    }

    let Some(rest) = prefixes
        .iter()
        .find_map(|prefix| token.strip_prefix(prefix))
    else {
        return Ok(Reference {
            file: None,
            code: String::new(),
            attributes: Vec::new(),
        });
    };
    if rest.is_empty() {
        return Err(TokenError::Syntax);
    }
    let Some((file, code)) = rest.split_once(':') else {
        return Err(TokenError::Syntax);
    };
    if file.is_empty() {
        return Err(TokenError::Syntax);
    }
    let mut attributes = Vec::new();
    let code = if is_ip {
        cut_negation(code).0.to_string()
    } else {
        let mut parts = code.split('@');
        let category = parts.next().unwrap_or_default().to_string();
        for attribute in parts {
            if attribute.is_empty() {
                return Err(TokenError::Syntax);
            }
            attributes.push(attribute.to_lowercase());
        }
        category
    };
    let code = code.to_lowercase();
    if code.is_empty() {
        return Err(TokenError::Syntax);
    }
    Ok(Reference {
        file: Some(file.to_string()),
        code,
        attributes,
    })
}

fn cut_negation(mut value: &str) -> (&str, bool) {
    let mut negated = false;
    while let Some(rest) = value.strip_prefix('!') {
        value = rest;
        negated = !negated;
    }
    (value, negated)
}

fn unknown_attribute(category: &GeoCategory, wanted: &[String]) -> Option<String> {
    if wanted.is_empty() {
        return None;
    }
    let present: HashSet<&str> = category
        .attributes
        .iter()
        .flat_map(|attribute| {
            let bare = attribute.trim_start_matches('!');
            [attribute.as_str(), bare]
        })
        .collect();
    wanted
        .iter()
        .find(|attribute| {
            !present.contains(attribute.as_str())
                && !present.contains(attribute.trim_start_matches('!'))
        })
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "lumen-geodata-{}-{}",
                std::process::id(),
                NEXT_DIR.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn write(&self, name: &str, data: &[u8]) {
            fs::write(self.0.join(name), data).unwrap();
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn varint(mut value: u64) -> Vec<u8> {
        let mut bytes = Vec::new();
        loop {
            let byte = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                bytes.push(byte);
                return bytes;
            }
            bytes.push(byte | 0x80);
        }
    }

    fn field(number: u32, wire: u8) -> Vec<u8> {
        varint((u64::from(number) << 3) | u64::from(wire))
    }

    fn len_field(number: u32, data: &[u8]) -> Vec<u8> {
        let mut out = field(number, 2);
        out.extend(varint(data.len() as u64));
        out.extend_from_slice(data);
        out
    }

    fn varint_field(number: u32, value: u64) -> Vec<u8> {
        let mut out = field(number, 0);
        out.extend(varint(value));
        out
    }

    fn domain(match_type: u64, value: &str, attributes: &[&str]) -> Vec<u8> {
        let mut out = Vec::new();
        if match_type != 0 {
            out.extend(varint_field(1, match_type));
        }
        out.extend(len_field(2, value.as_bytes()));
        for attribute in attributes {
            let key = len_field(1, attribute.as_bytes());
            out.extend(len_field(3, &key));
        }
        out
    }

    fn site_entry(code: &str, domains: &[Vec<u8>]) -> Vec<u8> {
        let mut out = len_field(1, code.as_bytes());
        for domain in domains {
            out.extend(len_field(2, domain));
        }
        out
    }

    fn cidr(address: [u8; 4], prefix: u64) -> Vec<u8> {
        let mut out = len_field(1, &address);
        if prefix != 0 {
            out.extend(varint_field(2, prefix));
        }
        out
    }

    fn ip_entry(code: &str, cidrs: &[Vec<u8>]) -> Vec<u8> {
        let mut out = len_field(1, code.as_bytes());
        for cidr in cidrs {
            out.extend(len_field(2, cidr));
        }
        out
    }

    fn geosite() -> Vec<u8> {
        let google = site_entry(
            "google",
            &[
                domain(2, "google.com", &[]),
                domain(3, "ads.google.com", &["ads"]),
            ],
        );
        let cn = site_entry("cn", &[domain(2, "baidu.com", &[])]);
        let mut out = len_field(1, &google);
        out.extend(len_field(1, &cn));
        out
    }

    fn geoip() -> Vec<u8> {
        let private = ip_entry("private", &[cidr([10, 0, 0, 0], 8)]);
        len_field(1, &private)
    }

    #[test]
    fn lists_files_and_detects_kinds() {
        let dir = TempDir::new();
        dir.write("geosite.dat", &geosite());
        dir.write("geoip.dat", &geoip());
        // A misleading name must not hide a site database.
        dir.write("myip-custom.dat", &geosite());
        dir.write("notes.txt", b"ignored");

        let files = Store::new(&dir.0).files().unwrap();
        assert_eq!(files.len(), 3);
        let by_name: HashMap<_, _> = files
            .iter()
            .map(|file| (file.name.as_str(), file))
            .collect();
        assert_eq!(by_name["geosite.dat"].kind, "site");
        assert_eq!(by_name["geosite.dat"].categories, 2);
        assert_eq!(by_name["geoip.dat"].kind, "ip");
        assert_eq!(by_name["geoip.dat"].categories, 1);
        assert_eq!(by_name["myip-custom.dat"].kind, "site");
        assert!(by_name["geosite.dat"].modified_at > 0);
        assert_eq!(by_name["geosite.dat"].error, None);
    }

    #[test]
    fn pages_categories_and_entries() {
        let dir = TempDir::new();
        dir.write("geosite.dat", &geosite());
        dir.write("geoip.dat", &geoip());
        let store = Store::new(&dir.0);

        let page = store.categories("geosite.dat", "Goo", 0, 0).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].code, "google");
        assert_eq!(page.items[0].entries, 2);
        assert_eq!(page.items[0].attributes, vec!["ads"]);

        let page = store.entries("geosite.dat", "google", "", 1, 1).unwrap();
        assert_eq!(page.total, 2);
        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].kind, "full");
        assert_eq!(page.items[0].value, "ads.google.com");

        let page = store.entries("geoip.dat", "private", "", 0, 100).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].kind, "cidr");
        assert_eq!(page.items[0].value, "10.0.0.0/8");
    }

    #[test]
    fn validates_routing_tokens() {
        let dir = TempDir::new();
        dir.write("geosite.dat", &geosite());
        dir.write("geoip.dat", &geoip());
        let store = Store::new(&dir.0);

        let tokens = |values: &[&str]| {
            values
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>()
        };
        let issues = store.validate(
            false,
            &tokens(&["geosite:google@ads", "example.com", "geosite:missing"]),
        );
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].reason, "categoryMissing");
        assert_eq!(issues[0].file.as_deref(), Some("geosite.dat"));

        let issues = store.validate(false, &tokens(&["geoip:private"]));
        assert_eq!(issues[0].reason, "wrongKind");
        let issues = store.validate(false, &tokens(&["geosite:google@typo"]));
        assert_eq!(issues[0].reason, "attributeMissing");
        let issues = store.validate(false, &tokens(&["ext:absent.dat:corp"]));
        assert_eq!(issues[0].reason, "fileMissing");
        let issues = store.validate(true, &tokens(&["geoip:private", "10.0.0.0/8"]));
        assert!(issues.is_empty());
    }

    #[test]
    fn rejects_path_traversal_and_non_dat() {
        let dir = TempDir::new();
        dir.write("geosite.dat", &geosite());
        let store = Store::new(&dir.0);
        for name in ["../geosite.dat", "sub/geosite.dat", "geosite.db", ".", ""] {
            assert!(
                store.categories(name, "", 0, 0).is_err(),
                "{name} should be rejected"
            );
        }
    }

    // Run manually with LUMEN_GEO_TEST_DIR pointing at real .dat files:
    // `LUMEN_GEO_TEST_DIR=/tmp/geo cargo test parses_real_databases -- --ignored`
    #[test]
    #[ignore]
    fn parses_real_databases() {
        let dir = std::env::var("LUMEN_GEO_TEST_DIR").expect("LUMEN_GEO_TEST_DIR");
        let store = Store::new(&dir);
        let files = store.files().unwrap();
        assert!(!files.is_empty());
        for file in files {
            assert!(file.error.is_none(), "{}: {:?}", file.name, file.error);
            assert!(file.categories > 0, "{} has no categories", file.name);
            let page = store.categories(&file.name, "", 0, 0).unwrap();
            assert_eq!(page.total, file.categories);
            let first = &page.items[0];
            let entries = store.entries(&file.name, &first.code, "", 0, 10).unwrap();
            assert!(entries.total > 0, "{} has no entries", first.code);
        }
    }

    #[test]
    fn broken_dat_is_listed_with_error() {
        let dir = TempDir::new();
        dir.write("broken.dat", b"not a protobuf database");
        let files = Store::new(&dir.0).files().unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].error.is_some());
    }
}
