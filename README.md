# JalaliX

Jalali (Persian / Shamsi) calendar library. Pure Rust — **zero external crate dependencies** (`std` only).

[![crates.io](https://img.shields.io/crates/v/jalalix.svg)](https://crates.io/crates/jalalix)
[![license](https://img.shields.io/crates/l/jalalix.svg)](./LICENSE)

```toml
[dependencies]
jalalix = "0.1"
```

```rust
use jalalix::JalaliX;

let mut date = JalaliX::createFromString("2025-02-25").unwrap();

date.format("YYYY/MM/DD"); // 1403/12/07
date.formatWith("DD MMM YYYY", true, false); // ۰۷ اسفند ۱۴۰۳

JalaliX::parseJalali("1403/12/07")
    .unwrap()
    .toISODate(jalalix::Calendar::Gregorian); // 2025-02-25
```

Algorithm matches the TypeScript package: base date **1348/10/11** (= 1970-01-01), `leapYearDivision = 0.24219858156`.

## Timezone

Internal storage is always a **UTC Unix millisecond instant** plus an IANA timezone id. `setTimezone` is **instant-preserving**: the timestamp stays the same; only the zone (and therefore wall clock / Jalali day) changes.

```rust
let d = JalaliX::create(1_740_430_800_000);
let mut tehran = d.setTimezone(Some("Asia/Tehran")).unwrap();
tehran.format("YYYY/MM/DD"); // 1403/12/07
tehran.format("HH:mm:ss");   // 00:30:00

let mut utc = d.setTimezone(Some("UTC")).unwrap();
utc.format("YYYY/MM/DD"); // 1403/12/06
utc.format("HH:mm:ss");   // 21:00:00
```

- **Aliases:** `utc` / `gmt` -> `UTC`; `local` / `default` / `system` -> host zone (from `TZ` or `/etc/localtime`) when detectable, else `UTC`.
- **Invalid zone** -> `Err`.
- Offsets come from a pure TZif reader: OS `/usr/share/zoneinfo/<id>` first, then vendored embeds (`UTC`, `Etc/UTC`, `Asia/Tehran`, `Asia/Tokyo`, `Europe/London`, `America/New_York`). No crates.

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
```

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
d.addMonths(1).getStartOfMonth().format("YYYY/MM/DD"); // 1404/01/01
```

### Persian digits

```rust
JalaliX::toPersian("1403/12/07");   // ۱۴۰۳/۱۲/۰۷
JalaliX::fromPersian("۱۴۰۳/۱۲/۰۷"); // 1403/12/07
```

## API notes

Method names use **camelCase** (matching TypeScript) under `#[allow(non_snake_case)]`.

| Method                     | Notes                                                         |
| -------------------------- | ------------------------------------------------------------- |
| `getMonth`                 | Returns **1–12**                                              |
| `setMonth` / `setFullYear` | Month argument is **0-based** (Farvardin = 0), like JS `Date` |
| Internal storage           | `timestamp_ms: i64` (UTC Unix ms)                             |

## License

MIT
