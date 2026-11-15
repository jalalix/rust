# JalaliX - Rust

Jalali (Persian / Shamsi) calendar on top of a millisecond Unix timestamp. Pure Rust - **zero external crate dependencies** (`std` only).

[![crates.io](https://img.shields.io/crates/v/jalalix.svg)](https://crates.io/crates/jalalix)
[![license](https://img.shields.io/crates/l/jalalix.svg)](./LICENSE)

```toml
[dependencies]
jalalix = "0.2.0"
```

```rust
use jalalix::JalaliX;

let mut date = JalaliX::createFromString("2025-02-25").unwrap();

date.format("YYYY/MM/DD"); // 1403/12/07
date.formatWith("DD MMM YYYY", true, false); // ۰۷ اسفند ۱۴۰۳

JalaliX::parseJalali("1403/12/07").unwrap().toISODate(jalalix::Calendar::Gregorian); // 2025-02-25
```

`toISODate` uses wall-clock getters in the active timezone. Prefer it over raw UTC formatting when you need a calendar date string.

## Install

```toml
[dependencies]
jalalix = "0.2.0"
```

```rust
use jalalix::JalaliX;
```

## Usage

### create / format

```rust
use jalalix::{Calendar, JalaliFormatOptions, JalaliX};

let mut now = JalaliX::now();
now.format("YYYY/MM/DD");
now.format("WN، DD MMM YYYY");

let opts = JalaliFormatOptions {
    calendar: Calendar::Jalali,
    persian_digits: true,
};
now.formatWithOptions("YYYY/MM/DD", &opts);

// Positional: formatWith(pattern, persian_digits, gregorian)
now.formatWith("DD MMM YYYY", false, true);
```

`create(timestamp_ms)` takes Unix milliseconds. `createFromString` accepts `YYYY-MM-DD` and ISO-like datetimes.

### parseJalali

```rust
JalaliX::parseJalali("1403/12/07").unwrap();
JalaliX::parseJalali("۱۴۰۳-۱۲-۰۷ 14:30").unwrap();
JalaliX::parseJalali("1403.12.07").unwrap();
```

Accepts `/`, `-`, or `.` as the date separator.

### normalizeQueryDate

Soft-normalize loose query / form date values to Gregorian `YYYY-MM-DD HH:mm:ss` (or `QueryDateValue::Null`). Does not panic on junk input.

```rust
use jalalix::{JalaliNormalizeOptions, JalaliX, QueryDateValue};

let opts = Some(JalaliNormalizeOptions {
    timezone: Some("Asia/Tehran".into()),
    ..Default::default()
});

JalaliX::normalizeQueryDate(QueryDateValue::Str("1403/12/07".into()), opts.clone());
// QueryDateValue::Str("2025-02-25 00:00:00".into())

JalaliX::normalizeQueryDate(QueryDateValue::Str("undefined".into()), None);
// QueryDateValue::Null

JalaliX::normalizeQueryDate(QueryDateValue::Int(1_740_430_800_000), opts);
```

Handles null sentinels, unix timestamps (seconds or milliseconds; non-positive → `Null`), Persian/Arabic-Indic digits, `/` `-` `.` separators, and optional offset suffixes.

Detects Jalali when the year is in `jalali_year_min`…`jalali_year_max` (default `1`…`1899`) **and** the civil date is valid; otherwise Gregorian when valid, or returns the digit-cleaned string.

### toISODate

```rust
use jalalix::Calendar;

let mut d = JalaliX::createFromString("2025-02-25").unwrap();
d.toISODate(Calendar::Gregorian); // 2025-02-25
d.toISODate(Calendar::Jalali);    // 1403-12-07
```

### add / start / end

```rust
let mut d = JalaliX::createFromString("2025-02-25").unwrap();
d.addMonths(1).getStartOfMonth().format("YYYY/MM/DD"); // e.g. 1404/01/01

let mut tehran = d.setTimezone(Some("Asia/Tehran")).unwrap();
tehran.addDays(7).format("YYYY/MM/DD");
tehran.getEndOfWeek().format("YYYY/MM/DD");
```

### Persian / Arabic digits

```rust
JalaliX::toPersian("1403/12/07");   // ۱۴۰۳/۱۲/۰۷
JalaliX::fromPersian("۱۴۰۳/۱۲/۰۷"); // 1403/12/07
JalaliX::fromPersian("١٤٠٣/١٢/٠٧"); // 1403/12/07 (Arabic-Indic)
```

## Timezone

Internal storage is always a **UTC Unix millisecond instant** plus an IANA timezone id. `setTimezone` is **instant-preserving**: the timestamp stays the same; only the zone (and therefore wall clock / Jalali day) changes.

```rust
let d = JalaliX::create(1_740_430_800_000); // 2025-02-25T00:30:00+03:30

let mut tehran = d.setTimezone(Some("Asia/Tehran")).unwrap();
tehran.format("YYYY/MM/DD"); // 1403/12/07
tehran.format("HH:mm");    // 00:30

let mut utc = d.setTimezone(Some("UTC")).unwrap();
utc.format("YYYY/MM/DD"); // 1403/12/06
utc.format("HH:mm");    // 21:00

tehran.getTime() == utc.getTime(); // true
```

- **Aliases:** `utc` / `gmt` → `UTC`; `local` / `default` / `system` → host zone (from `TZ` or `/etc/localtime`) when detectable, else `UTC`.
- **Invalid zone** → `Err(String)`.
- Offsets come from a pure TZif reader: OS `/usr/share/zoneinfo/<id>` first, then vendored embeds (`UTC`, `Etc/UTC`, `Asia/Tehran`, `Asia/Tokyo`, `Europe/London`, `America/New_York`).

```rust
JalaliX::createFromStringWithOptions("2025-02-25", &JalaliOptions {
    timezone: Some("Asia/Tehran".into()),
})?;
```

## Months

Easy to get wrong:

- `getMonth` → **1…12** (Farvardin = 1)
- `setMonth` / `setFullYear` month argument → **0…11**, same as JS `Date` (Farvardin = 0)

```rust
let mut d = JalaliX::now();
d.setFullYear(1404, Some(0), Some(1)); // 1 Farvardin 1404
d.getMonth(); // 1
```

## Format tokens

| Token               | Meaning                         |
| ------------------- | ------------------------------- |
| `YYYY` `YY`         | year                            |
| `MMM` `MM` `M`      | month name / number             |
| `DD` `D`            | day                             |
| `HH` `hh` `mm` `ss` | time                            |
| `A` `a`             | meridiem (ق.ظ. / ب.ظ. or AM/PM) |
| `WN` `WL`           | weekday name / letter           |
| `w` `DDD`           | week of year / day of year      |
| `X` `x`             | unix seconds / ms               |

## API

Method names use **camelCase** (matching TypeScript) under `#[allow(non_snake_case)]`. Many instance methods take `&mut self` because the type mutates in place and returns `&mut Self` for chaining.

### Types

| API                      | Description                                                                                        |
| ------------------------ | -------------------------------------------------------------------------------------------------- |
| `JalaliOptions`          | Optional `timezone: Option<String>` for create/parse.                                              |
| `JalaliNormalizeOptions` | `timezone`, `jalali_year_min`, `jalali_year_max` (defaults `1` / `1899`) for `normalizeQueryDate`. |
| `JalaliFormatOptions`    | `calendar: Calendar`, `persian_digits: bool` for `formatWithOptions`.                              |
| `Calendar`               | `Calendar::Jalali` or `Calendar::Gregorian`.                                                       |
| `QueryDateValue`         | Loose input/output for `normalizeQueryDate`: `Null`, `Bool`, `Int`, `Float`, `Str`.                |

### Static (`JalaliX`)

| API                                          | Description                                                                              |
| -------------------------------------------- | ---------------------------------------------------------------------------------------- |
| `now()`                                      | Current time from `SystemTime` (UTC clock).                                              |
| `create(timestamp_ms)`                       | Build from Unix milliseconds (timezone `UTC`).                                           |
| `createFromString(date)`                     | Build from `YYYY-MM-DD` or ISO-like Gregorian string.                                    |
| `createFromStringWithOptions(date, options)` | `createFromString` with optional timezone.                                               |
| `parseJalali(str)`                           | Parse a Jalali date/time string (`/` `-` `.`, Persian/Arabic digits).                    |
| `parseJalaliWithOptions(str, options)`       | `parseJalali` with optional timezone.                                                    |
| `normalizeQueryDate(value, options)`         | Soft-normalize loose query values → Gregorian datetime string or `QueryDateValue::Null`. |
| `isValidDate(timestamp_ms)`                  | `true` for any finite `i64` timestamp in this crate.                                     |
| `toPersian(str)`                             | Replace Western digits `0-9` with Persian `۰-۹`.                                         |
| `fromPersian(str)`                           | Replace Persian `۰-۹` and Arabic-Indic `٠-٩` with Western digits.                        |
| `compare(a, b)`                              | `true` if `a` is strictly before `b` (by unix ms).                                       |

### Instance - getters

| API                                                                  | Description                                            |
| -------------------------------------------------------------------- | ------------------------------------------------------ |
| `getFullYear()`                                                      | Jalali year in the active timezone.                    |
| `getMonth()`                                                         | Jalali month **1…12** (Farvardin = 1).                 |
| `getDate()`                                                          | Jalali day of month **1…31**.                          |
| `getDay()`                                                           | Weekday **1…7** (Saturday = 1 … Friday = 7).           |
| `getHours()` / `getMinutes()` / `getSeconds()` / `getMilliseconds()` | Wall-clock time in the active timezone.                |
| `getTime()`                                                          | Unix timestamp in milliseconds (UTC instant).          |
| `valueOf()`                                                          | Alias of `getTime()`.                                  |
| `getTimezone()`                                                      | Active IANA timezone id.                               |
| `getWeekOfYear()`                                                    | Jalali week-of-year (1-based).                         |
| `getDayOfYear()`                                                     | Day of Jalali year (1…365/366).                        |
| `getDayISO()`                                                        | ISO-style weekday index used internally for week math. |
| `getHoursISO()`                                                      | Hours in 0…23 for formatting helpers.                  |
| `getMeridiem()`                                                      | Localized meridiem string slice (ق.ظ./ب.ظ. or AM/PM).  |

### Instance - setters

| API                                | Description                                                    |
| ---------------------------------- | -------------------------------------------------------------- |
| `setTime(ms)`                      | Set the UTC instant from unix milliseconds.                    |
| `setMilliseconds(ms)`              | Set milliseconds (wall clock).                                 |
| `setSeconds(sec, ms?)`             | Set seconds (and optional ms).                                 |
| `setMinutes(min, sec?, ms?)`       | Set minutes (and optional lower units).                        |
| `setHours(h, min?, sec?, ms?)`     | Set hours (and optional lower units).                          |
| `setDate(day)`                     | Set Jalali day of month.                                       |
| `setMonth(month, date?)`           | Set Jalali month **0…11** (Farvardin = 0); optional day.       |
| `setFullYear(year, month?, date?)` | Set Jalali year; optional month **0…11** and day.              |
| `setTimezone(tz)`                  | Instant-preserving zone change; returns a new owned `JalaliX`. |
| `clone()`                          | Copy with the same instant and timezone.                       |

### Instance - math, range, format

| API                                                                               | Description                                                  |
| --------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| `addYears(n)` / `addMonths(n)` / `addWeeks(n)` / `addDays(n)`                     | Add calendar units; mutate in place; return `&mut Self`.     |
| `getStartOfYear()` / `getStartOfMonth()` / `getStartOfWeek()` / `getStartOfDay()` | Start of unit (`&mut Self`).                                 |
| `getEndOfYear()` / `getEndOfMonth()` / `getEndOfWeek()` / `getEndOfDay()`         | End of unit (`&mut Self`).                                   |
| `format(pattern)`                                                                 | Format with default Jalali calendar and Western digits.      |
| `formatWith(pattern, persian_digits, gregorian)`                                  | Positional format options.                                   |
| `formatWithOptions(pattern, options)`                                             | Format with `JalaliFormatOptions`.                           |
| `toISODate(calendar)`                                                             | `YYYY-MM-DD` in `Calendar::Gregorian` or `Calendar::Jalali`. |
| `toString()` / `toDateString()` / `toLocaleDateString()` / `toLocaleTimeString()` | String views of the date/time.                               |
| `Display` (`{date}` in `format!`)                                                 | Same general string view as `toString`.                      |

## Links

- https://crates.io/crates/jalalix
- https://docs.rs/jalalix
- https://jalalix.com
- https://github.com/jalalix/rust
- [CHANGELOG.md](./CHANGELOG.md)

## License

MIT
