// Pure TZif (v2/v3) reader + IANA zone resolution. No external crates.
//
// Looks up `/usr/share/zoneinfo/<id>` first (Unix), then falls back to embedded
// bytes for the vendored zones under `src/tzdata/`.

use std::fs;
use std::path::PathBuf;

// Embedded TZif blobs for portable / offline use.
static EMBEDDED: &[(&str, &[u8])] = &[
    ("UTC", include_bytes!("tzdata/UTC")),
    ("Etc/UTC", include_bytes!("tzdata/Etc/UTC")),
    ("Asia/Tehran", include_bytes!("tzdata/Asia/Tehran")),
    ("Asia/Tokyo", include_bytes!("tzdata/Asia/Tokyo")),
    ("Europe/London", include_bytes!("tzdata/Europe/London")),
    (
        "America/New_York",
        include_bytes!("tzdata/America/New_York"),
    ),
];

#[derive(Debug)]
struct LocalTimeType {
    // Seconds east of UTC.
    utoff: i32,
}

#[derive(Debug)]
struct TzDatabase {
    // Transition unix times (sorted ascending), 64-bit section.
    transitions: Vec<i64>,
    // Index into `types` for each transition.
    trans_types: Vec<u8>,
    types: Vec<LocalTimeType>,
    // POSIX TZ string after the last transition (may be empty).
    posix: String,
}

impl TzDatabase {
    fn offset_at(&self, unix_sec: i64) -> i32 {
        if self.transitions.is_empty() {
            return self.types.first().map(|t| t.utoff).unwrap_or(0);
        }

        // Before first transition -> type 0.
        if unix_sec < self.transitions[0] {
            return self.types[0].utoff;
        }

        // Binary search last transition <= unix_sec.
        let mut lo = 0usize;
        let mut hi = self.transitions.len();

        while lo < hi {
            let mid = (lo + hi) / 2;
            if self.transitions[mid] <= unix_sec {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }

        let idx = lo - 1;

        // After last transition -> prefer POSIX TZ rules when present.
        if idx + 1 == self.transitions.len() && !self.posix.is_empty() {
            if let Some(off) = posix_offset_at(&self.posix, unix_sec) {
                return off;
            }
        }

        let ti = self.trans_types[idx] as usize;
        self.types.get(ti).map(|t| t.utoff).unwrap_or(0)
    }
}

// Load TZif bytes from OS zoneinfo or embedded fallback.
fn load_tzif_bytes(iana_id: &str) -> Option<Vec<u8>> {
    // Prefer system zoneinfo on Unix.
    #[cfg(unix)]
    {
        let path = PathBuf::from("/usr/share/zoneinfo").join(iana_id);
        if let Ok(bytes) = fs::read(&path) {
            if bytes.starts_with(b"TZif") {
                return Some(bytes);
            }
        }

        // macOS sometimes uses /var/db/timezone/zoneinfo
        let path2 = PathBuf::from("/var/db/timezone/zoneinfo").join(iana_id);
        if let Ok(bytes) = fs::read(&path2) {
            if bytes.starts_with(b"TZif") {
                return Some(bytes);
            }
        }
    }

    for (id, bytes) in EMBEDDED {
        if *id == iana_id {
            return Some(bytes.to_vec());
        }
    }

    // Case-insensitive embedded match.
    let lower = iana_id.to_ascii_lowercase();

    for (id, bytes) in EMBEDDED {
        if id.to_ascii_lowercase() == lower {
            return Some(bytes.to_vec());
        }
    }
    None
}

fn read_i32_be(data: &[u8], off: &mut usize) -> Option<i32> {
    if *off + 4 > data.len() {
        return None;
    }

    let v = i32::from_be_bytes(data[*off..*off + 4].try_into().ok()?);
    *off += 4;
    Some(v)
}

fn read_i64_be(data: &[u8], off: &mut usize) -> Option<i64> {
    if *off + 8 > data.len() {
        return None;
    }

    let v = i64::from_be_bytes(data[*off..*off + 8].try_into().ok()?);
    *off += 8;
    Some(v)
}

fn read_u8(data: &[u8], off: &mut usize) -> Option<u8> {
    if *off >= data.len() {
        return None;
    }

    let v = data[*off];
    *off += 1;
    Some(v)
}

// Parse a TZif v2/v3 blob (skips the 32-bit section when version ≥ 2).
fn parse_tzif(data: &[u8]) -> Option<TzDatabase> {
    if data.len() < 44 || &data[0..4] != b"TZif" {
        return None;
    }

    let version = data[4];

    let mut off = 20;
    let _ttisutcnt = read_i32_be(data, &mut off)? as usize;
    let _ttisstdcnt = read_i32_be(data, &mut off)? as usize;
    let leapcnt = read_i32_be(data, &mut off)? as usize;
    let timecnt = read_i32_be(data, &mut off)? as usize;
    let typecnt = read_i32_be(data, &mut off)? as usize;
    let charcnt = read_i32_be(data, &mut off)? as usize;

    // Skip v1 (32-bit) body.
    off = 44;
    off += timecnt * 4; // transition times
    off += timecnt; // type indices
    off += typecnt * 6; // ttinfo
    off += charcnt; // abbreviations
    off += leapcnt * 8; // leap seconds (v1: 4+4)
    off += _ttisstdcnt;
    off += _ttisutcnt;

    if version == 0 || version == b'\0' {
        // Pure v1 — re-parse from the start of the body with 32-bit times.
        return parse_tzif_body_v1(data);
    }

    // v2/v3 second header
    if off + 44 > data.len() || &data[off..off + 4] != b"TZif" {
        return parse_tzif_body_v1(data);
    }

    off += 20;
    let ttisutcnt2 = read_i32_be(data, &mut off)? as usize;
    let ttisstdcnt2 = read_i32_be(data, &mut off)? as usize;
    let leapcnt2 = read_i32_be(data, &mut off)? as usize;
    let timecnt2 = read_i32_be(data, &mut off)? as usize;
    let typecnt2 = read_i32_be(data, &mut off)? as usize;
    let charcnt2 = read_i32_be(data, &mut off)? as usize;

    let mut transitions = Vec::with_capacity(timecnt2);

    for _ in 0..timecnt2 {
        transitions.push(read_i64_be(data, &mut off)?);
    }

    let mut trans_types = Vec::with_capacity(timecnt2);

    for _ in 0..timecnt2 {
        trans_types.push(read_u8(data, &mut off)?);
    }

    let mut types = Vec::with_capacity(typecnt2);

    for _ in 0..typecnt2 {
        let utoff = read_i32_be(data, &mut off)?;
        let _is_dst = read_u8(data, &mut off)?;
        let _abbrind = read_u8(data, &mut off)?;
        types.push(LocalTimeType { utoff });
    }

    off += charcnt2; // skip abbrs
    off += leapcnt2 * 12; // v2 leap: 8+4
    off += ttisstdcnt2;
    off += ttisutcnt2;

    let mut posix = String::new();

    if off < data.len() {
        // NL + TZ string + NL
        let rest = &data[off..];

        if let Some(first) = rest.iter().position(|&b| b == b'\n') {
            let after = &rest[first + 1..];

            if let Some(second) = after.iter().position(|&b| b == b'\n') {
                posix = String::from_utf8_lossy(&after[..second]).into_owned();
            } else if !after.is_empty() {
                posix = String::from_utf8_lossy(after).into_owned();
            }
        }
    }

    if types.is_empty() {
        return None;
    }

    Some(TzDatabase {
        transitions,
        trans_types,
        types,
        posix,
    })
}

fn parse_tzif_body_v1(data: &[u8]) -> Option<TzDatabase> {
    if data.len() < 44 {
        return None;
    }

    let mut off = 20;
    let ttisutcnt = read_i32_be(data, &mut off)? as usize;
    let ttisstdcnt = read_i32_be(data, &mut off)? as usize;
    let leapcnt = read_i32_be(data, &mut off)? as usize;
    let timecnt = read_i32_be(data, &mut off)? as usize;
    let typecnt = read_i32_be(data, &mut off)? as usize;
    let charcnt = read_i32_be(data, &mut off)? as usize;

    let mut transitions = Vec::with_capacity(timecnt);

    for _ in 0..timecnt {
        transitions.push(read_i32_be(data, &mut off)? as i64);
    }

    let mut trans_types = Vec::with_capacity(timecnt);

    for _ in 0..timecnt {
        trans_types.push(read_u8(data, &mut off)?);
    }

    let mut types = Vec::with_capacity(typecnt);

    for _ in 0..typecnt {
        let utoff = read_i32_be(data, &mut off)?;
        let _is_dst = read_u8(data, &mut off)?;
        let _abbrind = read_u8(data, &mut off)?;
        types.push(LocalTimeType { utoff });
    }

    off += charcnt;
    off += leapcnt * 8;
    off += ttisstdcnt;
    off += ttisutcnt;
    let _ = off;

    if types.is_empty() {
        return None;
    }

    Some(TzDatabase {
        transitions,
        trans_types,
        types,
        posix: String::new(),
    })
}

// Parse `±HH[:MM[:SS]]` or `H` style offset used in POSIX TZ (hours west of UTC
// when no sign, but `<…>` form uses explicit signed offset after the name).
fn parse_posix_time_offset(s: &str) -> Option<(i32, &str)> {
    // Returns (seconds east of UTC from the *numeric* field as written after the
    // abbreviation — POSIX treats that number as hours *west* of UTC unless the
    // abbr was quoted with `<>` and we apply the same convention: std offset in
    // the TZ string is "time to add to local to get UTC" i.e. west-positive.
    // We convert to east-of-UTC by negating.)
    let s = s.trim_start();

    if s.is_empty() {
        return None;
    }

    let mut chars = s.char_indices();

    let (sign, rest) = match chars.next()? {
        (_, '+') => (1i32, chars.as_str()),
        (_, '-') => (-1i32, chars.as_str()),
        (0, c) if c.is_ascii_digit() => (1i32, s),
        _ => return None,
    };

    let mut idx = 0usize;
    let bytes = rest.as_bytes();

    while idx < bytes.len() && bytes[idx].is_ascii_digit() {
        idx += 1;
    }

    if idx == 0 {
        return None;
    }

    let hours: i32 = rest[..idx].parse().ok()?;
    let mut minutes = 0i32;
    let mut seconds = 0i32;
    let mut rest2 = &rest[idx..];

    if rest2.starts_with(':') {
        rest2 = &rest2[1..];
        let mut j = 0usize;
        let b = rest2.as_bytes();

        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }

        minutes = rest2[..j].parse().ok()?;
        rest2 = &rest2[j..];

        if rest2.starts_with(':') {
            rest2 = &rest2[1..];
            let mut k = 0usize;
            let b2 = rest2.as_bytes();

            while k < b2.len() && b2[k].is_ascii_digit() {
                k += 1;
            }

            seconds = rest2[..k].parse().unwrap_or(0);
            rest2 = &rest2[k..];
        }
    }

    // POSIX: positive means west of UTC -> east offset is negative of that.
    let west = sign * (hours * 3600 + minutes * 60 + seconds);
    Some((-west, rest2))
}

fn skip_posix_abbr(s: &str) -> Option<&str> {
    let s = s.trim_start();

    if s.starts_with('<') {
        let end = s.find('>')?;
        Some(&s[end + 1..])
    } else {
        let bytes = s.as_bytes();
        let mut i = 0;

        while i < bytes.len() && bytes[i].is_ascii_alphabetic() {
            i += 1;
        }

        if i < 3 {
            // Allow shorter only if we already had `<>` form; alpha form needs ≥3
            // per POSIX, but zoneinfo sometimes uses shorter — accept ≥1 letter.
            if i == 0 {
                return None;
            }
        }

        Some(&s[i..])
    }
}

// Returns UTC offset (east of UTC) from a POSIX TZ string at `unix_sec`, or None.
fn posix_offset_at(tz: &str, unix_sec: i64) -> Option<i32> {
    let tz = tz.trim();

    if tz.is_empty() {
        return None;
    }

    // Split std / dst / rules: STD offset [DST [offset] [,start[/time],end[/time]]]
    let (std_part, rest) = split_std_dst(tz)?;
    let after_abbr = skip_posix_abbr(std_part)?;
    let (std_off, _) = parse_posix_time_offset(after_abbr)?;

    if rest.is_empty() {
        // Fixed offset only (e.g. `<+0330>-3:30` or `UTC0`).
        return Some(std_off);
    }

    // DST abbreviation + optional offset
    let after_dst_abbr = skip_posix_abbr(rest)?;

    let (dst_off, after_dst_off) = if after_dst_abbr.is_empty() || after_dst_abbr.starts_with(',') {
        // Default DST = STD + 1 hour
        (std_off + 3600, after_dst_abbr)
    } else if after_dst_abbr.starts_with('+')
        || after_dst_abbr.starts_with('-')
        || after_dst_abbr
            .as_bytes()
            .first()
            .map(|b| b.is_ascii_digit())
            .unwrap_or(false)
    {
        parse_posix_time_offset(after_dst_abbr)?
    } else {
        (std_off + 3600, after_dst_abbr)
    };

    if !after_dst_off.starts_with(',') {
        // No rules -> treat as always std (or always dst if only names?) — use std.
        return Some(std_off);
    }

    let rules = &after_dst_off[1..];
    let (start_rule, end_rule) = rules.split_once(',')?;

    let year = civil_year_from_unix(unix_sec);
    let start_utc = rule_to_unix(year, start_rule, std_off)?;
    let end_utc = rule_to_unix(year, end_rule, dst_off)?;

    let in_dst = if start_utc < end_utc {
        unix_sec >= start_utc && unix_sec < end_utc
    } else {
        // Southern hemisphere: DST spans year boundary
        unix_sec >= start_utc || unix_sec < end_utc
    };

    Some(if in_dst { dst_off } else { std_off })
}

fn split_std_dst(tz: &str) -> Option<(&str, &str)> {
    // Find end of std name+offset, then remainder is dst…
    // Walk: abbr, then offset, then optional dst.
    let after_abbr = skip_posix_abbr(tz)?;
    let (_, after_off) = parse_posix_time_offset(after_abbr)?;
    // std_part is tz[..len - after_off.len()] relative to after_off position
    let std_end = tz.len() - after_off.len();
    Some((&tz[..std_end], after_off))
}

fn civil_year_from_unix(unix_sec: i64) -> i32 {
    // Approximate via days; good enough for rule evaluation within a year.
    let days = if unix_sec >= 0 {
        unix_sec / 86400
    } else {
        (unix_sec - 86399) / 86400
    };

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    y as i32
}

fn days_from_civil(y: i32, m: i32, d: i32) -> i64 {
    let mut y = y as i64;
    let m = m as i64;
    let d = d as i64;
    y -= if m <= 2 { 1 } else { 0 };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy as u64;
    era * 146_097 + doe as i64 - 719_468
}

// Parse Jn / n / Mm.n.d[/time] -> unix seconds (UTC) for that year's transition.
fn rule_to_unix(year: i32, rule: &str, offset_at_transition: i32) -> Option<i64> {
    let (date_part, time_part) = match rule.split_once('/') {
        Some((d, t)) => (d, t),
        None => (rule, "2:00:00"),
    };

    let time_secs = parse_hhmmss(time_part).unwrap_or(2 * 3600);

    let day_of_year = if let Some(rest) = date_part.strip_prefix('J') {
        // Jn: Julian day 1–365, never Feb 29
        let n: i32 = rest.parse().ok()?;
        julian_n_to_doy(year, n)?
    } else if let Some(rest) = date_part.strip_prefix('M') {
        // Mm.n.d
        parse_m_rule(year, rest)?
    } else {
        // n: day of year 0–365 including leap
        let n: i32 = date_part.parse().ok()?;
        n
    };

    let local_secs =
        days_from_civil(year, 1, 1) * 86400 + day_of_year as i64 * 86400 + time_secs as i64;

    // Transition time is wall time in the *previous* offset (std for start, dst for end
    // per POSIX — we pass the offset that applies just before the transition).
    Some(local_secs - offset_at_transition as i64)
}

fn julian_n_to_doy(year: i32, n: i32) -> Option<i32> {
    // Jn: day 1–365, never counts Feb 29. Returns 0-based day-of-year.
    if !(1..=365).contains(&n) {
        return None;
    }

    if is_leap(year) && n >= 60 {
        Some(n) // J60 -> Mar 1 -> doy index 60 in a leap year
    } else {
        Some(n - 1)
    }
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

fn parse_m_rule(year: i32, rest: &str) -> Option<i32> {
    // m.n.d — month m, n-th weekday d (0=Sun), n=5 means last
    let mut parts = rest.split('.');
    let m: i32 = parts.next()?.parse().ok()?;
    let n: i32 = parts.next()?.parse().ok()?;
    let d: i32 = parts.next()?.parse().ok()?;

    if !(1..=12).contains(&m) || !(1..=5).contains(&n) || !(0..=6).contains(&d) {
        return None;
    }

    // Find 0-based doy
    let days_in_month = [
        31,
        28 + is_leap(year) as i32,
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

    let dim = days_in_month[(m - 1) as usize];
    // weekday of day 1 of month
    let day1 = days_from_civil(year, m, 1);
    let wday1 = ((day1 + 4).rem_euclid(7)) as i32; // 0=Sun
    let first = 1 + (d - wday1).rem_euclid(7);

    if n == 5 {
        let mut day = first;

        while day + 7 <= dim {
            day += 7;
        }

        Some((days_from_civil(year, m, day) - days_from_civil(year, 1, 1)) as i32)
    } else {
        let day = first + (n - 1) * 7;

        if day > dim {
            return None;
        }

        Some((days_from_civil(year, m, day) - days_from_civil(year, 1, 1)) as i32)
    }
}

fn parse_hhmmss(s: &str) -> Option<i32> {
    let mut parts = s.split(':');
    let h: i32 = parts.next()?.parse().ok()?;
    let m: i32 = parts.next().unwrap_or("0").parse().unwrap_or(0);
    let sec: i32 = parts.next().unwrap_or("0").parse().unwrap_or(0);
    Some(h * 3600 + m * 60 + sec)
}

// Detect host local IANA id from `TZ`, `/etc/localtime` symlink, or fall back to UTC.
pub fn detect_host_timezone() -> String {
    if let Ok(tz) = std::env::var("TZ") {
        let tz = tz.trim();

        if !tz.is_empty() {
            if tz.starts_with(':') {
                let path = &tz[1..];

                if let Some(id) = iana_from_zoneinfo_path(path) {
                    return id;
                }
            } else if !tz.contains('<') && tz.contains('/') {
                // Looks like Area/City
                if zone_exists(tz) {
                    return canonicalize_id(tz);
                }
            }
        }
    }

    #[cfg(unix)]
    {
        if let Ok(link) = fs::read_link("/etc/localtime") {
            if let Some(id) = iana_from_zoneinfo_path(&link.to_string_lossy()) {
                return id;
            }
        }
        // Some systems copy the file; try resolving realpath-style via /etc/localtime
        // already handled. Fall through.
    }

    "UTC".to_string()
}

fn iana_from_zoneinfo_path(path: &str) -> Option<String> {
    const MARKERS: &[&str] = &["/zoneinfo/", "/zoneinfo"];

    for marker in MARKERS {
        if let Some(pos) = path.find(marker) {
            let mut id = &path[pos + marker.len()..];
            if id.starts_with('/') {
                id = &id[1..];
            }

            if !id.is_empty() && !id.contains("..") {
                return Some(canonicalize_id(id));
            }
        }
    }

    None
}

fn canonicalize_id(id: &str) -> String {
    // Preserve known embedded casing; otherwise keep as given with standard Area/City.
    let lower = id.to_ascii_lowercase();

    if lower == "utc" || lower == "gmt" || lower == "etc/utc" {
        return if lower == "etc/utc" {
            "Etc/UTC".to_string()
        } else {
            "UTC".to_string()
        };
    }

    for (known, _) in EMBEDDED {
        if known.to_ascii_lowercase() == lower {
            return (*known).to_string();
        }
    }

    // Title-case path segments lightly: keep underscores, capitalize after /
    id.split('/')
        .map(|seg| {
            if seg.is_empty() {
                return String::new();
            }

            // Keep existing mixed case if already has uppercase
            if seg.chars().any(|c| c.is_ascii_uppercase()) {
                return seg.to_string();
            }

            let mut chars = seg.chars();
            let mut out = String::new();

            if let Some(f) = chars.next() {
                out.extend(f.to_uppercase());
                out.extend(chars);
            }

            out
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn zone_exists(id: &str) -> bool {
    load_tzif_bytes(id).is_some() || load_tzif_bytes(&canonicalize_id(id)).is_some()
}

// Resolve user timezone input to a canonical IANA id, or error if invalid.
pub fn resolve_timezone(input: Option<&str>, current: &str) -> Result<String, String> {
    match input {
        None => Ok(current.to_string()),
        Some(tz) => {
            let trimmed = tz.trim();

            if trimmed.is_empty() {
                return Err("invalid timezone: empty".to_string());
            }

            let lower = trimmed.to_ascii_lowercase();

            if lower == "default" || lower == "local" || lower == "system" {
                return Ok(detect_host_timezone());
            }

            if lower == "utc" || lower == "gmt" {
                return Ok("UTC".to_string());
            }

            let canon = canonicalize_id(trimmed);

            if load_tzif_bytes(&canon).is_some() {
                return Ok(canon);
            }

            // Try original
            if load_tzif_bytes(trimmed).is_some() {
                return Ok(canon);
            }

            Err(format!("invalid timezone: {trimmed}"))
        }
    }
}

// UTC offset in seconds (east of UTC) for `unix_ms` in the given IANA zone.
pub fn offset_seconds(iana_id: &str, unix_ms: i64) -> Result<i32, String> {
    if iana_id == "UTC" || iana_id.eq_ignore_ascii_case("gmt") {
        return Ok(0);
    }

    let bytes = load_tzif_bytes(iana_id).ok_or_else(|| format!("invalid timezone: {iana_id}"))?;
    let db = parse_tzif(&bytes).ok_or_else(|| format!("corrupt tzdata: {iana_id}"))?;

    let unix_sec = if unix_ms >= 0 {
        unix_ms / 1000
    } else {
        // floor div toward -inf
        unix_ms.div_euclid(1000)
    };

    Ok(db.offset_at(unix_sec))
}

// Convert a wall-clock (Y-M-D h:m:s.ms) in `iana_id` to UTC unix milliseconds.
//
// Iterates to converge across DST boundaries.
pub fn wall_to_utc_ms(
    iana_id: &str,
    y: i32,
    m: i32,
    d: i32,
    h: i32,
    mi: i32,
    s: i32,
    ms: i32,
) -> Result<i64, String> {
    let wall_as_utc = days_from_civil(y, m, d) * 86_400_000
        + (h as i64) * 3_600_000
        + (mi as i64) * 60_000
        + (s as i64) * 1000
        + ms as i64;

    if iana_id == "UTC" {
        return Ok(wall_as_utc);
    }

    // Iterate: guess UTC = wall - offset(guess)
    let mut utc = wall_as_utc;

    for _ in 0..4 {
        let off = offset_seconds(iana_id, utc)? as i64;
        let next = wall_as_utc - off * 1000;
        if next == utc {
            return Ok(utc);
        }
        utc = next;
    }

    Ok(utc)
}

// Wall-clock unix-ms equivalent: UTC instant + zone offset (for civil getters).
pub fn utc_to_wall_ms(iana_id: &str, unix_ms: i64) -> Result<i64, String> {
    let off = offset_seconds(iana_id, unix_ms)? as i64;
    Ok(unix_ms + off * 1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tehran_offset_golden() {
        let off = offset_seconds("Asia/Tehran", 1_740_430_800_000).unwrap();
        assert_eq!(off, 12_600);
    }

    #[test]
    fn utc_offset_zero() {
        assert_eq!(offset_seconds("UTC", 1_740_430_800_000).unwrap(), 0);
    }

    #[test]
    fn aliases() {
        assert_eq!(resolve_timezone(Some("utc"), "UTC").unwrap(), "UTC");
        assert_eq!(resolve_timezone(Some("gmt"), "UTC").unwrap(), "UTC");
        let local = resolve_timezone(Some("local"), "UTC").unwrap();
        assert!(!local.is_empty());
    }

    #[test]
    fn invalid_zone() {
        assert!(resolve_timezone(Some("Mars/Olympus"), "UTC").is_err());
    }
}
