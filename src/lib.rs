// JalaliX — Jalali (Persian / Shamsi) calendar on a Unix-millisecond timestamp.
//
// Pure Rust (`std` only). IANA timezones via a vendored/OS TZif reader
// ([`tz`]); see [`JalaliX::setTimezone`].

#![allow(clippy::too_many_arguments)]

mod tz;

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

const LEAP_YEAR_DIVISION: f64 = 0.24219858156;
const DATE_LENGTH: i64 = 86_400_000;
const FIRST_DAY_OF_WEEK: i32 = 6;
const BASE_YEAR: i32 = 1348;
const BASE_MONTH: i32 = 10;
const BASE_DAY: i32 = 11;

const WEEK_DAYS_NAMES: [&str; 7] = [
    "شنبه",
    "یکشنبه",
    "دوشنبه",
    "سه شنبه",
    "چهار شنبه",
    "پنج شنبه",
    "جمعه",
];

const WEEK_DAYS_LETTERS: [&str; 7] = ["ش", "ی", "د", "س", "چ", "پ", "ج"];

const WEEK_DAYS_GREGORIAN_NAMES: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];

const WEEK_DAYS_GREGORIAN_LETTERS: [&str; 7] = ["S", "M", "T", "W", "T", "F", "S"];

const MONTHS_NAMES: [&str; 12] = [
    "فروردین",
    "اردیبهشت",
    "خرداد",
    "تیر",
    "مرداد",
    "شهریور",
    "مهر",
    "آبان",
    "آذر",
    "دی",
    "بهمن",
    "اسفند",
];

const MONTHS_GREGORIAN_NAMES: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

const PERSIAN_NUMBERS: [char; 10] = ['۰', '۱', '۲', '۳', '۴', '۵', '۶', '۷', '۸', '۹'];
const AM: &str = "ق.ظ.";
const PM: &str = "ب.ظ.";
const AM_GREGORIAN: &str = "AM";
const PM_GREGORIAN: &str = "PM";

// Optional create / parse options.
#[derive(Debug, Clone, Default)]
pub struct JalaliOptions {
    // IANA timezone id or alias (e.g. `"UTC"`, `"Asia/Tehran"`, `"local"`).
    pub timezone: Option<String>,
}

// Calendar selection for formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Calendar {
    // Jalali (Persian / Shamsi) calendar.
    #[default]
    Jalali,
    // Gregorian calendar.
    Gregorian,
}

// Format options matching the TypeScript `{ calendar, persianDigits }` object.
#[derive(Debug, Clone, Default)]
pub struct JalaliFormatOptions {
    // Which calendar to use when formatting.
    pub calendar: Calendar,
    // When `true`, Western digits are converted to Persian numerals.
    pub persian_digits: bool,
}

// Internal Jalali date components cached on [`JalaliX`].
#[derive(Debug, Clone, Copy)]
struct JalaliDate {
    year: i32,
    month: i32,
    day: i32,
    hours: i32,
    min: i32,
    sec: i32,
    ms: i32,
    is_leap_year: bool,
}

impl Default for JalaliDate {
    fn default() -> Self {
        Self {
            year: BASE_YEAR,
            month: BASE_MONTH,
            day: BASE_DAY,
            hours: 0,
            min: 0,
            sec: 0,
            ms: 0,
            is_leap_year: false,
        }
    }
}

// Jalali (Persian) calendar date backed by a UTC Unix timestamp in milliseconds.
#[derive(Debug, Clone)]
pub struct JalaliX {
    // UTC instant (Unix milliseconds).
    timestamp_ms: i64,
    jalali: JalaliDate,
    is_converted: bool,
    // Canonical IANA id (e.g. `"Asia/Tehran"`) or `"UTC"`.
    timezone: String,
}

impl Default for JalaliX {
    fn default() -> Self {
        Self::now()
    }
}

#[allow(non_snake_case)]
impl JalaliX {
    // Builds a `JalaliX` from a Unix timestamp and timezone label.
    fn new_with_timestamp(timestamp_ms: i64, timezone: String) -> Self {
        let mut j = JalaliDate::default();
        let (h, m, s, ms) = Self::utc_hms(timestamp_ms);
        j.hours = h;
        j.min = m;
        j.sec = s;
        j.ms = ms;
        let mut this = Self {
            timestamp_ms,
            jalali: j,
            is_converted: false,
            timezone,
        };
        this.toJalali();
        this
    }

    // Creates a `JalaliX` for the current time (UTC clock from [`SystemTime`]).
    //
    // Returns a new `JalaliX`.
    pub fn now() -> Self {
        let ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);

        Self::new_with_timestamp(ms, "UTC".to_string())
    }

    // Creates a JalaliX instance from a Unix timestamp in milliseconds.
    //
    // * `timestamp_ms` - Milliseconds since Unix epoch.
    //
    // Returns a new `JalaliX`.
    pub fn create(timestamp_ms: i64) -> Self {
        Self::new_with_timestamp(timestamp_ms, "UTC".to_string())
    }

    // Creates a JalaliX instance from a Gregorian date string.
    //
    // * `date` - ISO / `YYYY-MM-DD` string (UTC when no offset is given).
    //
    // Returns a new `JalaliX`, or an error if the string is invalid.
    pub fn createFromString(date: &str) -> Result<Self, String> {
        Self::createFromStringWithOptions(date, None)
    }

    // Creates a JalaliX instance from a Gregorian date string with options.
    //
    // * `date` - ISO / `YYYY-MM-DD` string (UTC when no offset is given).
    // * `options` - Optional configuration including timezone.
    //
    // Returns a new `JalaliX`, or an error if the string is invalid.
    pub fn createFromStringWithOptions(
        date: &str,
        options: Option<JalaliOptions>,
    ) -> Result<Self, String> {
        let ms = parse_datetime(date)?;

        let tz = match options.as_ref().and_then(|o| o.timezone.as_deref()) {
            Some(t) => tz::resolve_timezone(Some(t), "UTC")?,
            None => "UTC".to_string(),
        };

        Ok(Self::new_with_timestamp(ms, tz))
    }

    // Parses a Jalali date string into a `JalaliX` instance.
    //
    // Accepts `YYYY/MM/DD` or `YYYY-MM-DD`, optional time (`HH:mm` / `HH:mm:ss`),
    // and Persian digits.
    //
    // * `value` - Jalali date string (e.g. `"1403/12/07"`, `"۱۴۰۳-۱۲-۰۷ 14:30"`).
    //
    // Returns a new `JalaliX`, or an error if the string is invalid.
    pub fn parseJalali(value: &str) -> Result<Self, String> {
        Self::parseJalaliWithOptions(value, None)
    }

    // Parses a Jalali date string into a `JalaliX` with optional timezone options.
    //
    // * `value` - Jalali date string (e.g. `"1403/12/07"`).
    // * `options` - Optional configuration including timezone.
    //
    // Returns a new `JalaliX`, or an error if the string is invalid.
    pub fn parseJalaliWithOptions(
        value: &str,
        options: Option<JalaliOptions>,
    ) -> Result<Self, String> {
        let normalized = Self::fromPersian(value).trim().to_string();

        let (year, month, day, hours, minutes, seconds) = parse_jalali_string(&normalized)
            .ok_or_else(|| format!("JalaliX.parseJalali: invalid Jalali date \"{value}\""))?;

        if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
            return Err(format!(
                "JalaliX.parseJalali: invalid Jalali date \"{value}\""
            ));
        }

        let mut output = match &options {
            Some(o) => Self::now_with_options(o)?,
            None => Self::now(),
        };

        output.setFullYear(year, Some(month - 1), Some(day));
        output.setHours(hours, Some(minutes), Some(seconds), Some(0));
        Ok(output)
    }

    // Creates a `JalaliX` for now, applying optional timezone options.
    fn now_with_options(options: &JalaliOptions) -> Result<Self, String> {
        let mut out = Self::now();

        if let Some(ref tz) = options.timezone {
            out = out.setTimezone(Some(tz.as_str()))?;
        }

        Ok(out)
    }

    // Checks if the given value is a valid date timestamp.
    //
    // * `timestamp_ms` - The value to check.
    //
    // Returns `true` if valid (any finite `i64` for this crate).
    pub fn isValidDate(timestamp_ms: i64) -> bool {
        // Any finite i64 is a valid timestamp for this crate.
        let _ = timestamp_ms;
        true
    }

    // Converts Western numerals (0-9) to Persian numerals (۰-۹) in a string.
    //
    // * `str` - The string containing numerals to convert.
    //
    // Returns the string with Persian numerals.
    pub fn toPersian(str: &str) -> String {
        let mut out = String::with_capacity(str.len());

        for ch in str.chars() {
            if ch.is_ascii_digit() {
                out.push(PERSIAN_NUMBERS[(ch as u8 - b'0') as usize]);
            } else {
                out.push(ch);
            }
        }

        out
    }

    // Converts Persian numerals (۰-۹) to Western numerals (0-9) in a string.
    //
    // * `str` - The string containing Persian numerals.
    //
    // Returns the string with Western numerals.
    pub fn fromPersian(str: &str) -> String {
        let mut out = String::with_capacity(str.len());

        for ch in str.chars() {
            if let Some(idx) = PERSIAN_NUMBERS.iter().position(|&p| p == ch) {
                out.push((b'0' + idx as u8) as char);
            } else {
                out.push(ch);
            }
        }

        out
    }

    // Compares two JalaliX dates.
    //
    // * `date` - The first date to compare.
    // * `comparing` - The second date to compare against.
    //
    // Returns `true` if `date` is before `comparing`, `false` otherwise.
    pub fn compare(date: &JalaliX, comparing: &JalaliX) -> bool {
        date.getTime() < comparing.getTime()
    }

    // Gets the timestamp in milliseconds since Unix epoch.
    //
    // Returns the Unix timestamp in milliseconds.
    pub fn getTime(&self) -> i64 {
        self.timestamp_ms
    }

    // Returns the primitive value as a Unix timestamp in milliseconds.
    //
    // Returns the Unix timestamp in milliseconds.
    pub fn valueOf(&self) -> i64 {
        self.timestamp_ms
    }

    // Creates a copy of this JalaliX instance.
    //
    // Returns a new `JalaliX` instance with the same timestamp.
    pub fn clone(&self) -> Self {
        self.clone_inner()
    }

    // Deep-clones fields into a new `JalaliX`.
    fn clone_inner(&self) -> Self {
        Self {
            timestamp_ms: self.timestamp_ms,
            jalali: self.jalali,
            is_converted: self.is_converted,
            timezone: self.timezone.clone(),
        }
    }

    // Gets the IANA timezone id of this instance (e.g. `"Asia/Tehran"` or `"UTC"`).
    pub fn getTimezone(&self) -> String {
        self.timezone.clone()
    }

    // Creates a new JalaliX with the same UTC instant in a different timezone.
    //
    // Instant-preserving: `getTime()` is unchanged; wall getters / Jalali day may change.
    //
    // * `timezone` - IANA id (`"Asia/Tehran"`), or alias `utc`/`gmt`/`local`/`default`/`system`.
    //   `None` keeps the current zone.
    //
    // Returns `Err` if the zone cannot be resolved (unknown IANA id).
    pub fn setTimezone(&self, timezone: Option<&str>) -> Result<Self, String> {
        let id = tz::resolve_timezone(timezone, &self.timezone)?;
        let mut out = self.clone_inner();
        out.timezone = id;
        out.is_converted = false;
        out.toJalali();
        Ok(out)
    }

    // Wall-clock milliseconds (UTC instant + zone offset) for civil getters.
    fn wall_ms(&self) -> i64 {
        tz::utc_to_wall_ms(&self.timezone, self.timestamp_ms).unwrap_or(self.timestamp_ms)
    }

    // Checks if the given Jalali year is a leap year.
    //
    // * `year` - The Jalali year to check.
    //
    // Returns `true` if the year is a leap year, `false` otherwise.
    fn checkLeapYear(year: i32) -> bool {
        if year > 0 {
            let check_year = (year as f64 + 2346.0) * LEAP_YEAR_DIVISION;
            check_year - check_year.trunc() < LEAP_YEAR_DIVISION
        } else {
            false
        }
    }

    // Converts the current timestamp to Jalali (Persian) calendar date in the
    // active timezone (wall time).
    //
    // Returns the Jalali date object with year, month, day, hours, minutes,
    // seconds, milliseconds, and leap-year flag.
    fn toJalali(&mut self) -> JalaliDate {
        if !self.is_converted {
            let wall = self.wall_ms();
            let diff_days = wall.div_euclid(DATE_LENGTH);

            self.jalali.year = BASE_YEAR;
            self.jalali.month = BASE_MONTH;
            self.jalali.day = BASE_DAY;

            if diff_days > 0 {
                for _ in 0..diff_days {
                    self.jalali.day += 1;

                    if self.jalali.month < 7 && self.jalali.day > 31 {
                        self.jalali.month += 1;
                        self.jalali.day = 1;
                    } else if self.jalali.month > 6
                        && self.jalali.month < 12
                        && self.jalali.day > 30
                    {
                        self.jalali.month += 1;
                        self.jalali.day = 1;
                    } else if self.jalali.month == 12 {
                        let is_leap = Self::checkLeapYear(self.jalali.year);

                        if is_leap && self.jalali.day > 30 {
                            self.jalali.year += 1;
                            self.jalali.month = 1;
                            self.jalali.day = 1;
                        } else if !is_leap && self.jalali.day > 29 {
                            self.jalali.year += 1;
                            self.jalali.month = 1;
                            self.jalali.day = 1;
                        }
                    }
                }
            } else if diff_days < 0 {
                for _ in 0..(-diff_days) {
                    self.jalali.day -= 1;

                    if self.jalali.day < 1 {
                        if self.jalali.month == 1 {
                            self.jalali.year -= 1;
                            self.jalali.month = 12;
                            let is_leap = Self::checkLeapYear(self.jalali.year);
                            self.jalali.day = if is_leap { 30 } else { 29 };
                        } else if self.jalali.month < 8 {
                            self.jalali.month -= 1;
                            self.jalali.day = 31;
                        } else if self.jalali.month < 13 {
                            self.jalali.month -= 1;
                            self.jalali.day = 30;
                        }
                    }
                }
            }

            let (h, m, s, ms) = Self::utc_hms(wall);
            self.jalali.hours = h;
            self.jalali.min = m;
            self.jalali.sec = s;
            self.jalali.ms = ms;
            self.jalali.is_leap_year = Self::checkLeapYear(self.jalali.year);
            self.is_converted = true;
        }

        self.jalali
    }

    // Converts a Jalali date (wall clock in the active zone) to a UTC timestamp.
    //
    // * `j_date` - The Jalali date object to convert.
    //
    // Returns the UTC timestamp in milliseconds.
    fn toGregorian(&self, j_date: JalaliDate) -> i64 {
        let mut jalali = JalaliDate {
            year: BASE_YEAR,
            month: BASE_MONTH,
            day: BASE_DAY,
            hours: self.gregorian_hours(),
            min: self.gregorian_minutes(),
            sec: self.gregorian_seconds(),
            ms: self.gregorian_milliseconds(),
            is_leap_year: false,
        };

        let mut diff_days: i64 = 0;

        let after_or_eq_base = j_date.year > BASE_YEAR
            || (j_date.year == BASE_YEAR && j_date.month > BASE_MONTH)
            || (j_date.year == BASE_YEAR && j_date.month == BASE_MONTH && j_date.day >= BASE_DAY);

        let before_base = j_date.year < BASE_YEAR
            || (j_date.year == BASE_YEAR && j_date.month < BASE_MONTH)
            || (j_date.year == BASE_YEAR && j_date.month == BASE_MONTH && j_date.day < BASE_DAY);

        if after_or_eq_base {
            loop {
                if jalali.year == j_date.year
                    && jalali.month == j_date.month
                    && jalali.day == j_date.day
                {
                    break;
                }

                diff_days += 1;
                jalali.day += 1;

                if jalali.month < 7 && jalali.day > 31 {
                    jalali.month += 1;
                    jalali.day = 1;
                } else if jalali.month > 6 && jalali.month < 12 && jalali.day > 30 {
                    jalali.month += 1;
                    jalali.day = 1;
                } else if jalali.month == 12 {
                    let is_leap = Self::checkLeapYear(jalali.year);

                    if is_leap && jalali.day > 30 {
                        jalali.year += 1;
                        jalali.month = 1;
                        jalali.day = 1;
                    } else if !is_leap && jalali.day > 29 {
                        jalali.year += 1;
                        jalali.month = 1;
                        jalali.day = 1;
                    }
                }
            }
        } else if before_base {
            loop {
                if jalali.year == j_date.year
                    && jalali.month == j_date.month
                    && jalali.day == j_date.day
                {
                    break;
                }

                diff_days -= 1;
                jalali.day -= 1;

                if jalali.day < 1 {
                    if jalali.month == 1 {
                        jalali.year -= 1;
                        jalali.month = 12;
                        let is_leap = Self::checkLeapYear(jalali.year);
                        jalali.day = if is_leap { 30 } else { 29 };
                    } else if jalali.month < 8 {
                        jalali.month -= 1;
                        jalali.day = 31;
                    } else if jalali.month < 13 {
                        jalali.month -= 1;
                        jalali.day = 30;
                    }
                }
            }
        }

        // Wall HMS in the active timezone -> UTC via offset (DST-safe iterate).
        let hours = self.gregorian_hours() as i64;
        let minutes = self.gregorian_minutes() as i64;
        let seconds = self.gregorian_seconds() as i64;
        let ms = self.gregorian_milliseconds() as i64;

        let wall_as_utc =
            diff_days * DATE_LENGTH + (hours * 3600 + minutes * 60 + seconds) * 1000 + ms;

        let (y, m, d, h, mi, s, mss) = Self::utc_parts(wall_as_utc);
        tz::wall_to_utc_ms(&self.timezone, y, m, d, h, mi, s, mss).unwrap_or(wall_as_utc)
    }

    // Resets the Jalali conversion state and recalculates from the base date.
    //
    // Returns the recalculated Jalali date.
    fn reset(&mut self) -> JalaliDate {
        self.is_converted = false;
        self.jalali.year = BASE_YEAR;
        self.jalali.month = BASE_MONTH;
        self.jalali.day = BASE_DAY;
        self.toJalali()
    }

    // Pads a number with leading zero if it's a single digit.
    //
    // * `number` - The number to pad.
    //
    // Returns the padded string (e.g. `"05"` for 5).
    fn addZero(number: i32) -> String {
        if (0..10).contains(&number) {
            format!("0{number}")
        } else {
            number.to_string()
        }
    }

    // Returns the Persian name of a Jalali month.
    //
    // * `number` - The month number (1-12).
    //
    // Returns the Persian month name (e.g. `"فروردین"`).
    fn getJalaliMonth(number: i32) -> &'static str {
        MONTHS_NAMES[(number - 1) as usize]
    }

    // Returns the English name of a Gregorian month.
    //
    // * `number` - The month number (1-12).
    //
    // Returns the English month name (e.g. `"January"`).
    fn getGregorianMonth(number: i32) -> &'static str {
        MONTHS_GREGORIAN_NAMES[(number - 1) as usize]
    }

    // Splits a millisecond timeline into Gregorian calendar and time parts
    // (treating the value as a civil “as-if-UTC” timeline — used for wall ms too).
    //
    // Returns `(year, month 1-12, day, hour, min, sec, ms)`.
    fn utc_parts(timestamp_ms: i64) -> (i32, i32, i32, i32, i32, i32, i32) {
        let ms = timestamp_ms.rem_euclid(1000) as i32;
        let total_secs = timestamp_ms.div_euclid(1000);
        let sec = total_secs.rem_euclid(60) as i32;
        let total_mins = total_secs.div_euclid(60);
        let min = total_mins.rem_euclid(60) as i32;
        let total_hours = total_mins.div_euclid(60);
        let hour = total_hours.rem_euclid(24) as i32;
        let days = total_hours.div_euclid(24);
        let (y, m, d) = civil_from_days(days);
        (y, m, d, hour, min, sec, ms)
    }

    // Returns `(hour, min, sec, ms)` from a civil millisecond timeline.
    fn utc_hms(timestamp_ms: i64) -> (i32, i32, i32, i32) {
        let (_, _, _, h, m, s, ms) = Self::utc_parts(timestamp_ms);
        (h, m, s, ms)
    }

    // Gregorian full year (wall time in active timezone).
    fn gregorian_full_year(&self) -> i32 {
        Self::utc_parts(self.wall_ms()).0
    }

    // Gregorian month 0-based (wall), matching JS `Date.getMonth()`.
    fn gregorian_month0(&self) -> i32 {
        Self::utc_parts(self.wall_ms()).1 - 1
    }

    // Gregorian day of month (wall).
    fn gregorian_date(&self) -> i32 {
        Self::utc_parts(self.wall_ms()).2
    }

    // Gregorian hours (wall).
    fn gregorian_hours(&self) -> i32 {
        Self::utc_parts(self.wall_ms()).3
    }

    // Gregorian minutes (wall).
    fn gregorian_minutes(&self) -> i32 {
        Self::utc_parts(self.wall_ms()).4
    }

    // Gregorian seconds (wall).
    fn gregorian_seconds(&self) -> i32 {
        Self::utc_parts(self.wall_ms()).5
    }

    // Gregorian milliseconds (wall).
    fn gregorian_milliseconds(&self) -> i32 {
        Self::utc_parts(self.wall_ms()).6
    }

    // JS `Date.getDay()`: 0 = Sunday … 6 = Saturday (wall weekday).
    fn gregorian_weekday(&self) -> i32 {
        // Unix epoch (1970-01-01) was Thursday = 4
        let days = self.wall_ms().div_euclid(DATE_LENGTH);
        ((days + 4).rem_euclid(7)) as i32
    }

    // Returns the day of the year (1-366) in Gregorian calendar (wall).
    fn getGregorianDayOfYear(&self) -> i32 {
        let (y, m, d, _, _, _, _) = Self::utc_parts(self.wall_ms());
        day_of_year_gregorian(y, m, d)
    }

    // Returns the ISO week number of the year in Gregorian calendar (wall).
    fn getGregorianWeekOfYear(&self) -> i32 {
        let (y, m, d, _, _, _, _) = Self::utc_parts(self.wall_ms());
        iso_week_of_year(y, m, d)
    }

    // Returns the English name of the day of the week.
    fn getGregorianWeekDayName(&self) -> &'static str {
        WEEK_DAYS_GREGORIAN_NAMES[self.gregorian_weekday() as usize]
    }

    // Returns the first letter of the day of the week in English.
    fn getGregorianWeekDayLetter(&self) -> &'static str {
        WEEK_DAYS_GREGORIAN_LETTERS[self.gregorian_weekday() as usize]
    }

    // Returns the Persian name of the day of the week.
    fn getWeekDayName(&self) -> &'static str {
        let idx = (self.gregorian_weekday() + FIRST_DAY_OF_WEEK).rem_euclid(7) as usize;
        WEEK_DAYS_NAMES[idx]
    }

    // Returns the first letter of the day of the week in Persian.
    fn getWeekDayLetter(&self) -> &'static str {
        let idx = (self.gregorian_weekday() + FIRST_DAY_OF_WEEK).rem_euclid(7) as usize;
        WEEK_DAYS_LETTERS[idx]
    }

    // Extracts and returns a timezone suffix string for display.
    fn getUTCTimezone(&self) -> String {
        let off = tz::offset_seconds(&self.timezone, self.timestamp_ms).unwrap_or(0);
        let sign = if off >= 0 { '+' } else { '-' };
        let abs = off.abs();
        let hh = abs / 3600;
        let mm = (abs % 3600) / 60;
        format!(" GMT{sign}{hh:02}{mm:02}")
    }

    // Adjusts the date by adding the specified number of days.
    //
    // * `date_delta` - The number of days to add (can be negative).
    //
    // Returns the updated timestamp in milliseconds.
    fn normalizeDate(&mut self, date_delta: i32) -> i64 {
        self.timestamp_ms += date_delta as i64 * DATE_LENGTH;
        self.timestamp_ms
    }

    // Normalizes Jalali date components and sets the internal timestamp accordingly.
    //
    // * `j_date` - The current Jalali date object.
    // * `year` - The target year.
    // * `month` - The target month (1-12).
    // * `day` - The target day of month.
    //
    // Returns the updated timestamp in milliseconds.
    fn normalizeJalali(
        &mut self,
        j_date: JalaliDate,
        mut year: i32,
        mut month: i32,
        mut day: i32,
    ) -> i64 {
        if month < 1 {
            let year_diff = ((1 - month) as f64 / 12.0).ceil() as i32;
            year -= year_diff;
            month = 12 * year_diff + month;
        } else if month > 12 {
            let year_diff = ((month - 1) as f64 / 12.0).floor() as i32;
            year += year_diff;
            month -= 12 * year_diff;
        }

        if month > 6 && month < 12 && day > 30 {
            day = 30;
        } else if month == 12 {
            let is_leap = Self::checkLeapYear(year);

            if is_leap && day > 30 {
                day = 30;
            } else if !is_leap && day > 29 {
                day = 29;
            }
        }

        let mut target = j_date;
        target.year = year;
        target.month = month;
        target.day = day;
        self.timestamp_ms = self.toGregorian(target);
        self.timestamp_ms
    }

    // Returns a string representation of a date in Jalali format.
    //
    // Returns a full date string with weekday, day, month, year, and time.
    pub fn toString(&mut self) -> String {
        let j = self.toJalali();

        format!(
            "{} {} {} {} {}:{}:{}{}",
            self.getWeekDayName(),
            Self::addZero(j.day),
            Self::getJalaliMonth(j.month),
            Self::addZero(j.year),
            j.hours,
            Self::addZero(j.min),
            Self::addZero(j.sec),
            self.getUTCTimezone()
        )
    }

    // Returns a date as a string value (without time).
    //
    // Returns a date string with weekday, day, month, and year.
    pub fn toDateString(&mut self) -> String {
        let j = self.toJalali();

        format!(
            "{} {} {} {}",
            self.getWeekDayName(),
            Self::addZero(j.day),
            Self::getJalaliMonth(j.month),
            Self::addZero(j.year)
        )
    }

    // Returns a date as a string value in locale format (`YYYY/MM/DD`).
    //
    // Returns a locale-formatted date string.
    pub fn toLocaleDateString(&mut self) -> String {
        let j = self.toJalali();

        format!(
            "{}/{}/{}",
            Self::addZero(j.year),
            Self::addZero(j.month),
            Self::addZero(j.day)
        )
    }

    // Returns a time as a string value with meridiem.
    //
    // Returns a time string (e.g. `"10:30:45 ق.ظ."`).
    pub fn toLocaleTimeString(&mut self) -> String {
        let j = self.toJalali();

        format!(
            "{}:{}:{} {}",
            j.hours % 12,
            Self::addZero(j.min),
            Self::addZero(j.sec),
            self.getMeridiem()
        )
    }

    // Gets the Jalali year.
    //
    // Returns the year (e.g. `1403`).
    pub fn getFullYear(&mut self) -> i32 {
        self.toJalali().year
    }

    // Gets the Jalali month (1-12).
    //
    // Farvardin = 1.
    //
    // Returns the month number.
    pub fn getMonth(&mut self) -> i32 {
        self.toJalali().month
    }

    // Gets the day of the month in Jalali calendar (1-31).
    //
    // Returns the day of the month.
    pub fn getDate(&mut self) -> i32 {
        self.toJalali().day
    }

    // Gets the day of the week (Saturday-based Jalali index matching TypeScript).
    //
    // May be `0` for Friday.
    //
    // Returns the day number.
    pub fn getDay(&self) -> i32 {
        (self.gregorian_weekday() - FIRST_DAY_OF_WEEK + 7 + 1).rem_euclid(7)
    }

    // Gets the hours value (wall time in the active timezone).
    //
    // Returns the hours (0-23).
    pub fn getHours(&self) -> i32 {
        self.gregorian_hours()
    }

    // Gets the minutes value (wall time in the active timezone).
    //
    // Returns the minutes (0-59).
    pub fn getMinutes(&self) -> i32 {
        self.gregorian_minutes()
    }

    // Gets the seconds value (wall time in the active timezone).
    //
    // Returns the seconds (0-59).
    pub fn getSeconds(&self) -> i32 {
        self.gregorian_seconds()
    }

    // Gets the milliseconds value (wall time in the active timezone).
    //
    // Returns the milliseconds (0-999).
    pub fn getMilliseconds(&self) -> i32 {
        self.gregorian_milliseconds()
    }

    // Gets the week number of the year in Jalali calendar.
    //
    // Returns the week number (1-53).
    pub fn getWeekOfYear(&mut self) -> i32 {
        let day_of_year = self.getDayOfYear();
        let mut start = self.clone_inner();
        let day_of_week = start.getStartOfYear().getDayISO();
        ((day_of_year + day_of_week - 1) as f64 / 7.0).ceil() as i32
    }

    // Gets the day of the year in Jalali calendar (1-366).
    //
    // Returns the day of the year.
    pub fn getDayOfYear(&mut self) -> i32 {
        let j = self.toJalali();
        let mut output = j.day;

        if j.month < 7 {
            output += (j.month - 1) * 31;
        } else if j.month < 12 {
            output += 186 + (j.month - 7) * 30;
        } else {
            output += 336;
        }

        output
    }

    // Gets the ISO day of the week (1=Monday, 7=Sunday).
    //
    // Returns the ISO day number (1-7).
    pub fn getDayISO(&self) -> i32 {
        ((self.getDay() + 6) % 7) + 1
    }

    // Gets the hour in 12-hour format (1-12).
    //
    // Returns the hour in 12-hour format.
    pub fn getHoursISO(&self) -> i32 {
        let hours = self.getHours();

        if hours > 12 {
            hours - 12
        } else {
            hours
        }
    }

    // Gets the meridiem (AM/PM) in Persian.
    //
    // Returns `"ق.ظ."` (before noon) or `"ب.ظ."` (after noon).
    pub fn getMeridiem(&mut self) -> &'static str {
        let j = self.toJalali();

        if j.hours > 12 {
            PM
        } else {
            AM
        }
    }

    // Returns AM or PM based on the hour in 12-hour format.
    fn getGregorianMeridiem(&self) -> &'static str {
        if self.gregorian_hours() >= 12 {
            PM_GREGORIAN
        } else {
            AM_GREGORIAN
        }
    }

    // Sets the date and time value.
    //
    // * `time` - Milliseconds since midnight, January 1, 1970 GMT.
    //
    // Returns the updated timestamp in milliseconds.
    pub fn setTime(&mut self, time: i64) -> i64 {
        self.timestamp_ms = time;
        self.reset();
        self.getTime()
    }

    // Sets the milliseconds value using wall time in the active timezone.
    //
    // * `ms` - A numeric value equal to the millisecond value.
    //
    // Returns the updated UTC timestamp in milliseconds.
    pub fn setMilliseconds(&mut self, ms: i32) -> i64 {
        let (y, mo, d, h, mi, s, _) = Self::utc_parts(self.wall_ms());

        self.timestamp_ms = tz::wall_to_utc_ms(&self.timezone, y, mo, d, h, mi, s, ms)
            .unwrap_or_else(|_| date_to_timestamp(y, mo, d, h, mi, s, ms));

        self.reset();
        self.getTime()
    }

    // Sets the seconds value using wall time in the active timezone.
    //
    // * `sec` - A numeric value equal to the seconds value.
    // * `ms` - Optional milliseconds value.
    //
    // Returns the updated UTC timestamp in milliseconds.
    pub fn setSeconds(&mut self, sec: i32, ms: Option<i32>) -> i64 {
        let (y, mo, d, h, mi, _, old_ms) = Self::utc_parts(self.wall_ms());
        let ms_v = ms.unwrap_or(old_ms);

        self.timestamp_ms = tz::wall_to_utc_ms(&self.timezone, y, mo, d, h, mi, sec, ms_v)
            .unwrap_or_else(|_| date_to_timestamp(y, mo, d, h, mi, sec, ms_v));

        self.reset();
        self.getTime()
    }

    // Sets the minutes value using wall time in the active timezone.
    //
    // * `min` - A numeric value equal to the minutes value.
    // * `sec` - Optional seconds value.
    // * `ms` - Optional milliseconds value.
    //
    // Returns the updated UTC timestamp in milliseconds.
    pub fn setMinutes(&mut self, min: i32, sec: Option<i32>, ms: Option<i32>) -> i64 {
        let (y, mo, d, h, _, old_s, old_ms) = Self::utc_parts(self.wall_ms());
        let s = sec.unwrap_or(old_s);
        let ms_v = ms.unwrap_or(old_ms);

        self.timestamp_ms = tz::wall_to_utc_ms(&self.timezone, y, mo, d, h, min, s, ms_v)
            .unwrap_or_else(|_| date_to_timestamp(y, mo, d, h, min, s, ms_v));

        self.reset();
        self.getTime()
    }

    // Sets the hour value using wall time in the active timezone.
    //
    // * `hours` - A numeric value equal to the hours value.
    // * `min` - Optional minutes value.
    // * `sec` - Optional seconds value.
    // * `ms` - Optional milliseconds value.
    //
    // Returns the updated UTC timestamp in milliseconds.
    pub fn setHours(
        &mut self,
        hours: i32,
        min: Option<i32>,
        sec: Option<i32>,
        ms: Option<i32>,
    ) -> i64 {
        let (y, mo, d, _, old_mi, old_s, old_ms) = Self::utc_parts(self.wall_ms());
        let mi = min.unwrap_or(old_mi);
        let s = sec.unwrap_or(old_s);
        let ms_v = ms.unwrap_or(old_ms);

        self.timestamp_ms = tz::wall_to_utc_ms(&self.timezone, y, mo, d, hours, mi, s, ms_v)
            .unwrap_or_else(|_| date_to_timestamp(y, mo, d, hours, mi, s, ms_v));

        self.reset();
        self.getTime()
    }

    // Sets the numeric day-of-the-month value using Jalali calendar.
    //
    // * `date` - A numeric value equal to the day of the month.
    //
    // Returns the updated timestamp in milliseconds.
    pub fn setDate(&mut self, date: i32) -> i64 {
        let j = self.toJalali();
        self.normalizeDate(date - j.day);
        self.getTime()
    }

    // Sets the month value using Jalali calendar.
    //
    // * `month` - Zero-based month (Farvardin = 0), matching the TypeScript / `Date` API.
    // * `date` - Optional day of the month; if omitted, the current day is kept.
    //
    // Returns the updated timestamp in milliseconds.
    pub fn setMonth(&mut self, month: i32, date: Option<i32>) -> i64 {
        let j = self.toJalali();
        self.normalizeJalali(j, j.year, month + 1, j.day);
        self.reset();

        if let Some(date) = date {
            let j2 = self.toJalali();
            self.normalizeDate(date - j2.day);
            self.reset();
        }

        self.getTime()
    }

    // Sets the year of the date using Jalali calendar.
    //
    // * `year` - A numeric value for the year.
    // * `month` - Optional zero-based month (Farvardin = 0). Must be specified if `date` is specified.
    // * `date` - Optional day of the month.
    //
    // Returns the updated timestamp in milliseconds.
    pub fn setFullYear(&mut self, year: i32, month: Option<i32>, date: Option<i32>) -> i64 {
        let j = self.toJalali();
        let m = month.map(|x| x + 1).unwrap_or(j.month);
        self.normalizeJalali(j, year, m, j.day);
        self.reset();

        if let Some(date) = date {
            let j2 = self.toJalali();
            self.normalizeDate(date - j2.day);
            self.reset();
        }

        self.getTime()
    }

    // Sets this instance to the start of the Jalali year.
    //
    // Returns this instance modified to the first moment of the year.
    pub fn getStartOfYear(&mut self) -> &mut Self {
        let doy = self.getDayOfYear();
        self.addDays(-doy + 1).getStartOfDay()
    }

    // Sets this instance to the start of the current month.
    //
    // Returns this instance modified to the first moment of the month.
    pub fn getStartOfMonth(&mut self) -> &mut Self {
        let d = self.getDate();
        self.addDays(-d + 1).getStartOfDay()
    }

    // Sets this instance to the start of the current week (Monday).
    //
    // Returns this instance modified to the first moment of the week.
    pub fn getStartOfWeek(&mut self) -> &mut Self {
        let iso = self.getDayISO();
        self.addDays(-iso + 1).getStartOfDay()
    }

    // Sets this instance to the start of the current day (`00:00:00.000`).
    //
    // Returns this instance modified to midnight.
    pub fn getStartOfDay(&mut self) -> &mut Self {
        self.setHours(0, Some(0), Some(0), Some(0));
        self
    }

    // Sets this instance to the end of the Jalali year (`23:59:59.999`).
    //
    // Returns this instance modified to the last moment of the year.
    pub fn getEndOfYear(&mut self) -> &mut Self {
        let j = self.toJalali();
        let last_day = if j.is_leap_year { 366 } else { 365 };
        let doy = self.getDayOfYear();
        self.addDays(last_day - doy).getEndOfDay()
    }

    // Sets this instance to the end of the current month.
    //
    // Returns this instance modified to the last moment of the month.
    pub fn getEndOfMonth(&mut self) -> &mut Self {
        let j = self.toJalali();
        let mut last_day = 31;

        if j.month > 6 && j.month < 12 {
            last_day = 30;
        } else if j.month == 12 {
            last_day = if j.is_leap_year { 30 } else { 29 };
        }

        let d = self.getDate();
        self.addDays(last_day - d).getEndOfDay()
    }

    // Sets this instance to the end of the current week (Sunday).
    //
    // Returns this instance modified to the last moment of the week.
    pub fn getEndOfWeek(&mut self) -> &mut Self {
        let iso = self.getDayISO();
        self.addDays(7 - iso).getEndOfDay()
    }

    // Sets this instance to the end of the current day (`23:59:59.999`).
    //
    // Returns this instance modified to the last moment of the day.
    pub fn getEndOfDay(&mut self) -> &mut Self {
        self.setHours(23, Some(59), Some(59), Some(999));
        self
    }

    // Adds the specified number of years to the date.
    //
    // * `year` - The number of years to add (can be negative).
    //
    // Returns this instance modified.
    pub fn addYears(&mut self, year: i32) -> &mut Self {
        let j = self.toJalali();
        self.normalizeJalali(j, j.year + year, j.month, j.day);
        self.reset();
        self
    }

    // Adds the specified number of months to the date.
    //
    // * `month` - The number of months to add (can be negative).
    //
    // Returns this instance modified.
    pub fn addMonths(&mut self, month: i32) -> &mut Self {
        let j = self.toJalali();
        self.normalizeJalali(j, j.year, j.month + month, j.day);
        self.reset();
        self
    }

    // Adds the specified number of weeks to the date.
    //
    // * `weeks` - The number of weeks to add (can be negative).
    //
    // Returns this instance modified.
    pub fn addWeeks(&mut self, weeks: i32) -> &mut Self {
        self.normalizeDate(weeks * 7);
        self.reset();
        self
    }

    // Adds the specified number of days to the date.
    //
    // * `days` - The number of days to add (can be negative).
    //
    // Returns this instance modified.
    pub fn addDays(&mut self, days: i32) -> &mut Self {
        self.normalizeDate(days);
        self.reset();
        self
    }

    // Returns a calendar date string using local getters (no UTC shift from ISO datetime).
    //
    // * `calendar` - [`Calendar::Gregorian`] -> `YYYY-MM-DD` Gregorian; [`Calendar::Jalali`] -> Jalali `YYYY-MM-DD`.
    //
    // Returns the formatted date string.
    pub fn toISODate(&mut self, calendar: Calendar) -> String {
        match calendar {
            Calendar::Jalali => self.format("YYYY-MM-DD"),
            Calendar::Gregorian => {
                let y = self.gregorian_full_year();
                let m = self.gregorian_month0() + 1;
                let d = self.gregorian_date();
                format!("{}-{}-{}", y, Self::addZero(m), Self::addZero(d))
            }
        }
    }

    // Formats the date with default Jalali calendar and Western digits.
    //
    // * `pattern` - The format string (e.g. `"YYYY/MM/DD"`, `"DD MMM YYYY"`).
    //
    // Returns the formatted date string.
    pub fn format(&mut self, pattern: &str) -> String {
        self.formatWith(pattern, false, false)
    }

    // Formats the date with positional options: `format(str, persianDigits?, gregorian?)`.
    //
    // * `pattern` - The format string (e.g. `"YYYY/MM/DD"`).
    // * `persian_digits` - When `true`, converts digits to Persian numerals.
    // * `gregorian` - When `true`, uses Gregorian calendar.
    //
    // Returns the formatted date string.
    pub fn formatWith(&mut self, pattern: &str, persian_digits: bool, gregorian: bool) -> String {
        self.format_internal(pattern, persian_digits, gregorian)
    }

    // Formats the date with object-style options `{ calendar, persianDigits }`.
    //
    // * `pattern` - The format string (e.g. `"YYYY/MM/DD"`).
    // * `options` - Calendar and Persian-digits options.
    //
    // Returns the formatted date string.
    pub fn formatWithOptions(&mut self, pattern: &str, options: &JalaliFormatOptions) -> String {
        self.format_internal(
            pattern,
            options.persian_digits,
            options.calendar == Calendar::Gregorian,
        )
    }

    // Shared format implementation for token replacement.
    fn format_internal(&mut self, pattern: &str, persian: bool, use_gregorian: bool) -> String {
        let year = if use_gregorian {
            self.gregorian_full_year()
        } else {
            self.getFullYear()
        };

        let month = if use_gregorian {
            self.gregorian_month0() + 1
        } else {
            self.getMonth()
        };

        let week_of_year = if use_gregorian {
            self.getGregorianWeekOfYear()
        } else {
            self.getWeekOfYear()
        };

        let day_of_year = if use_gregorian {
            self.getGregorianDayOfYear()
        } else {
            self.getDayOfYear()
        };

        let date = if use_gregorian {
            self.gregorian_date()
        } else {
            self.getDate()
        };

        let month_name = if use_gregorian {
            Self::getGregorianMonth(month)
        } else {
            Self::getJalaliMonth(month)
        };

        let week_day_name = if use_gregorian {
            self.getGregorianWeekDayName()
        } else {
            self.getWeekDayName()
        };

        let week_day_letter = if use_gregorian {
            self.getGregorianWeekDayLetter()
        } else {
            self.getWeekDayLetter()
        };

        let meridiem = if use_gregorian {
            self.getGregorianMeridiem()
        } else {
            self.getMeridiem()
        };

        let mut placeholders: Vec<String> = Vec::new();

        let mut ph = |val: String| -> String {
            let i = placeholders.len();
            placeholders.push(val);
            format!("\u{E000}{i}\u{E001}")
        };

        fn slice_end(s: &str, n: usize) -> String {
            if s.len() >= n {
                s[s.len() - n..].to_string()
            } else {
                s.to_string()
            }
        }

        let mut output = pattern.to_string();
        let year_str = year.to_string();

        replace_token(&mut output, "YYYY", || ph(year_str.clone()));
        replace_token(&mut output, "YYY", || ph(slice_end(&year_str, 3)));
        replace_token(&mut output, "YY", || ph(slice_end(&year_str, 2)));
        replace_token(&mut output, "Y", || ph(slice_end(&year_str, 1)));

        replace_token(&mut output, "MMM", || ph(month_name.to_string()));
        replace_token(&mut output, "MM", || ph(Self::addZero(month)));
        replace_token(&mut output, "M", || ph(month.to_string()));

        replace_token(&mut output, "W", || ph(Self::addZero(week_of_year)));
        replace_token(&mut output, "w", || ph(week_of_year.to_string()));

        let day_iso = self.getDayISO();

        let day_raw = if use_gregorian {
            let d = self.gregorian_weekday();
            if d == 0 {
                7
            } else {
                d
            }
        } else {
            self.getDay()
        };

        replace_token(&mut output, "DW", || ph(day_iso.to_string()));
        replace_token(&mut output, "dw", || ph(day_raw.to_string()));
        replace_token(&mut output, "WN", || ph(week_day_name.to_string()));
        replace_token(&mut output, "wn", || ph(week_day_name.to_string()));
        replace_token(&mut output, "WL", || ph(week_day_letter.to_string()));
        replace_token(&mut output, "wl", || ph(week_day_letter.to_string()));

        replace_token(&mut output, "DDDD", || ph(Self::addZero(day_of_year)));
        replace_token(&mut output, "DDD", || ph(day_of_year.to_string()));

        replace_token(&mut output, "DD", || ph(Self::addZero(date)));
        replace_token(&mut output, "D", || ph(date.to_string()));

        let hours = self.getHours();
        replace_token(&mut output, "HH", || ph(Self::addZero(hours)));
        replace_token(&mut output, "H", || ph(hours.to_string()));

        let hours_iso = if hours > 12 {
            hours - 12
        } else if hours == 0 {
            12
        } else {
            hours
        };

        replace_token(&mut output, "hh", || ph(Self::addZero(hours_iso)));
        replace_token(&mut output, "h", || ph(hours_iso.to_string()));

        let minutes = self.getMinutes();
        replace_token(&mut output, "mm", || ph(Self::addZero(minutes)));
        replace_token(&mut output, "m", || ph(minutes.to_string()));

        let seconds = self.getSeconds();
        replace_token(&mut output, "ss", || ph(Self::addZero(seconds)));
        replace_token(&mut output, "s", || ph(seconds.to_string()));

        let millis = self.getMilliseconds();
        replace_token(&mut output, "sss", || ph(millis.to_string()));
        replace_token(&mut output, "SSS", || ph(millis.to_string()));

        replace_token(&mut output, "A", || ph(meridiem.to_string()));
        replace_token(&mut output, "a", || ph(meridiem.to_string()));

        let unix_s = self.getTime() / 1000;
        let unix_ms = self.getTime();
        replace_token(&mut output, "X", || ph(unix_s.to_string()));
        replace_token(&mut output, "x", || ph(unix_ms.to_string()));

        for (i, val) in placeholders.iter().enumerate() {
            let key = format!("\u{E000}{i}\u{E001}");
            output = output.replace(&key, val);
        }

        if persian {
            Self::toPersian(&output)
        } else {
            output
        }
    }
}

impl fmt::Display for JalaliX {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut c = self.clone_inner();
        write!(f, "{}", c.toString())
    }
}

// Replace all occurrences of `token` in `output` when present (longest tokens first).
fn replace_token<F>(output: &mut String, token: &str, mut make: F)
where
    F: FnMut() -> String,
{
    if output.contains(token) {
        // Replace each occurrence with a fresh placeholder (values are identical for date tokens).
        let replacement = make();
        *output = output.replace(token, &replacement);
    }
}

// Howard Hinnant `civil_from_days` (UTC days since 1970-01-01).
fn civil_from_days(z: i64) -> (i32, i32, i32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as i32, d as i32)
}

// Inverse of [`civil_from_days`]: Gregorian Y/M/D -> days since Unix epoch.
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

// Builds a Unix timestamp (ms) from Gregorian UTC date-time components.
fn date_to_timestamp(y: i32, m: i32, d: i32, h: i32, mi: i32, s: i32, ms: i32) -> i64 {
    let days = days_from_civil(y, m, d);
    days * DATE_LENGTH
        + (h as i64) * 3_600_000
        + (mi as i64) * 60_000
        + (s as i64) * 1000
        + ms as i64
}

// Whether a Gregorian year is a leap year.
fn is_gregorian_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0)
}

// Day of year (1-366) in the Gregorian calendar.
fn day_of_year_gregorian(y: i32, m: i32, d: i32) -> i32 {
    const CUM: [i32; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let mut n = CUM[(m - 1) as usize] + d;
    if m > 2 && is_gregorian_leap(y) {
        n += 1;
    }
    n
}

// ISO week number of the year (matches JS `getGregorianWeekOfYear`).
fn iso_week_of_year(y: i32, m: i32, d: i32) -> i32 {
    let days = days_from_civil(y, m, d);

    // UTC date, weekday: 0=Sun..6=Sat -> for ISO, treat Sun as 7
    let weekday = ((days + 4).rem_euclid(7)) as i32; // 0=Sun
    let iso_dow = if weekday == 0 { 7 } else { weekday }; // 1=Mon..7=Sun

    // d.setUTCDate(d.getUTCDate() + 4 - (d.getUTCDay() || 7))
    let adj = days + (4 - iso_dow) as i64;
    let (yy, _, _) = civil_from_days(adj);
    let year_start = days_from_civil(yy, 1, 1);

    // JS: Math.ceil(((d.getTime() - yearStart.getTime()) / dateLength + 1) / 7)
    let ms_diff = (adj - year_start) * DATE_LENGTH;
    ((ms_diff as f64 / DATE_LENGTH as f64 + 1.0) / 7.0).ceil() as i32
}

// Parses an ISO / `YYYY-MM-DD` / timestamp string into Unix milliseconds.
fn parse_datetime(s: &str) -> Result<i64, String> {
    let s = s.trim();

    // YYYY-MM-DD
    if s.len() == 10 && s.as_bytes().get(4) == Some(&b'-') && s.as_bytes().get(7) == Some(&b'-') {
        let y: i32 = s[0..4].parse().map_err(|_| format!("invalid date: {s}"))?;
        let m: i32 = s[5..7].parse().map_err(|_| format!("invalid date: {s}"))?;
        let d: i32 = s[8..10].parse().map_err(|_| format!("invalid date: {s}"))?;
        return Ok(date_to_timestamp(y, m, d, 0, 0, 0, 0));
    }

    // YYYY-MM-DDTHH:mm[:ss[.sss]][Z|±HH:mm]
    if s.len() >= 16 && (s.contains('T') || s.contains(' ')) {
        let norm = s.replace(' ', "T");
        let (date_part, rest) = norm
            .split_once('T')
            .ok_or_else(|| format!("invalid date: {s}"))?;
        if date_part.len() != 10 {
            return Err(format!("invalid date: {s}"));
        }
        let y: i32 = date_part[0..4]
            .parse()
            .map_err(|_| format!("invalid date: {s}"))?;
        let m: i32 = date_part[5..7]
            .parse()
            .map_err(|_| format!("invalid date: {s}"))?;
        let d: i32 = date_part[8..10]
            .parse()
            .map_err(|_| format!("invalid date: {s}"))?;

        let mut offset_mins: i32 = 0;
        let mut time_part = rest;

        if let Some(stripped) = time_part
            .strip_suffix('Z')
            .or_else(|| time_part.strip_suffix('z'))
        {
            time_part = stripped;
            offset_mins = 0;
        } else if let Some(pos) = time_part.rfind(['+', '-']) {
            if pos > 0 {
                let sign = if time_part.as_bytes()[pos] == b'+' {
                    1
                } else {
                    -1
                };

                let off = &time_part[pos + 1..];

                let (oh, om) = if off.contains(':') {
                    let (a, b) = off.split_once(':').unwrap();
                    (a.parse::<i32>().unwrap_or(0), b.parse::<i32>().unwrap_or(0))
                } else if off.len() >= 4 {
                    (
                        off[0..2].parse().unwrap_or(0),
                        off[2..4].parse().unwrap_or(0),
                    )
                } else {
                    (0, 0)
                };

                offset_mins = sign * (oh * 60 + om);
                time_part = &time_part[..pos];
            }
        }

        let mut parts = time_part.split(':');

        let h: i32 = parts
            .next()
            .unwrap_or("0")
            .parse()
            .map_err(|_| format!("invalid date: {s}"))?;

        let mi: i32 = parts
            .next()
            .unwrap_or("0")
            .parse()
            .map_err(|_| format!("invalid date: {s}"))?;

        let sec_part = parts.next().unwrap_or("0");

        let (sec_str, ms_str) = if let Some((a, b)) = sec_part.split_once('.') {
            (a, b)
        } else {
            (sec_part, "0")
        };

        let sec: i32 = sec_str.parse().unwrap_or(0);
        let mut ms_digits = ms_str.chars().take(3).collect::<String>();

        while ms_digits.len() < 3 {
            ms_digits.push('0');
        }

        let ms: i32 = ms_digits.parse().unwrap_or(0);

        let utc = date_to_timestamp(y, m, d, h, mi, sec, ms) - (offset_mins as i64) * 60_000;

        return Ok(utc);
    }

    // bare timestamp number as string
    if let Ok(ms) = s.parse::<i64>() {
        return Ok(ms);
    }

    Err(format!("invalid date: {s}"))
}

// Parses a Jalali `YYYY/MM/DD` or `YYYY-MM-DD` string (optional time).
fn parse_jalali_string(s: &str) -> Option<(i32, i32, i32, i32, i32, i32)> {
    // ^(\d{4})[/-](\d{1,2})[/-](\d{1,2})(?:[ T](\d{1,2}):(\d{1,2})(?::(\d{1,2}))?)?$
    let bytes = s.as_bytes();

    if bytes.len() < 8 {
        return None;
    }

    let mut i = 0;

    let take_digits = |i: &mut usize, min: usize, max: usize| -> Option<i32> {
        let start = *i;

        while *i < bytes.len() && bytes[*i].is_ascii_digit() {
            *i += 1;
        }

        let len = *i - start;

        if len < min || len > max {
            return None;
        }

        std::str::from_utf8(&bytes[start..*i]).ok()?.parse().ok()
    };

    let year = take_digits(&mut i, 4, 4)?;

    if i >= bytes.len() || (bytes[i] != b'/' && bytes[i] != b'-') {
        return None;
    }

    i += 1;
    let month = take_digits(&mut i, 1, 2)?;

    if i >= bytes.len() || (bytes[i] != b'/' && bytes[i] != b'-') {
        return None;
    }

    i += 1;
    let day = take_digits(&mut i, 1, 2)?;

    let mut hours = 0;
    let mut minutes = 0;
    let mut seconds = 0;

    if i < bytes.len() {
        if bytes[i] != b' ' && bytes[i] != b'T' {
            return None;
        }

        i += 1;
        hours = take_digits(&mut i, 1, 2)?;

        if i >= bytes.len() || bytes[i] != b':' {
            return None;
        }

        i += 1;
        minutes = take_digits(&mut i, 1, 2)?;

        if i < bytes.len() {
            if bytes[i] != b':' {
                return None;
            }

            i += 1;
            seconds = take_digits(&mut i, 1, 2)?;
        }
    }

    if i != bytes.len() {
        return None;
    }

    Some((year, month, day, hours, minutes, seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn golden_create_2025_02_25() {
        let mut d = JalaliX::createFromString("2025-02-25").unwrap();
        assert_eq!(d.format("YYYY/MM/DD"), "1403/12/07");
    }

    #[test]
    fn parse_jalali_roundtrip() {
        let mut d = JalaliX::parseJalali("1403/12/07").unwrap();
        assert_eq!(d.toISODate(Calendar::Gregorian), "2025-02-25");
    }

    #[test]
    fn persian_digits() {
        assert_eq!(JalaliX::toPersian("1403/12/07"), "۱۴۰۳/۱۲/۰۷");
        assert_eq!(JalaliX::fromPersian("۱۴۰۳/۱۲/۰۷"), "1403/12/07");
    }

    #[test]
    fn parse_jalali_persian_digits() {
        let mut d = JalaliX::parseJalali("۱۴۰۳-۱۲-۰۷").unwrap();
        assert_eq!(d.format("YYYY/MM/DD"), "1403/12/07");
    }

    #[test]
    fn get_month_1_based() {
        let mut d = JalaliX::createFromString("2025-02-25").unwrap();
        assert_eq!(d.getMonth(), 12);
        assert_eq!(d.getFullYear(), 1403);
        assert_eq!(d.getDate(), 7);
    }

    #[test]
    fn add_months_start_of_month() {
        let mut d = JalaliX::createFromString("2025-02-25").unwrap();
        d.addMonths(1).getStartOfMonth();
        assert_eq!(d.format("YYYY/MM/DD"), "1404/01/01");
    }

    #[test]
    fn format_persian_options() {
        let mut d = JalaliX::createFromString("2025-02-25").unwrap();
        let opts = JalaliFormatOptions {
            calendar: Calendar::Jalali,
            persian_digits: true,
        };
        let s = d.formatWithOptions("DD MMM YYYY", &opts);
        assert!(s.contains("اسفند"));
        assert!(s.contains("۱۴۰۳"));
    }

    #[test]
    fn leap_year_check() {
        // 1403 is a leap year in this algorithm
        assert!(JalaliX::checkLeapYear(1403));
    }

    #[test]
    fn set_timezone_instant_preserving() {
        let d = JalaliX::create(1_740_430_800_000);
        let d2 = d.setTimezone(Some("UTC")).unwrap();
        assert_eq!(d2.getTimezone(), "UTC");
        let d3 = d.setTimezone(Some("Asia/Tehran")).unwrap();
        assert_eq!(d3.getTimezone(), "Asia/Tehran");
        assert_eq!(d.getTime(), d3.getTime());
        assert!(d.setTimezone(Some("Mars/Olympus")).is_err());
    }

    #[test]
    fn create_from_timestamp() {
        let ms = date_to_timestamp(2025, 2, 25, 0, 0, 0, 0);
        let mut d = JalaliX::create(ms);
        assert_eq!(d.format("YYYY/MM/DD"), "1403/12/07");
    }

    #[test]
    fn compare_and_clone() {
        let a = JalaliX::createFromString("2025-02-25").unwrap();
        let b = JalaliX::createFromString("2025-02-26").unwrap();
        assert!(JalaliX::compare(&a, &b));
        let c = a.clone();
        assert_eq!(a.getTime(), c.getTime());
    }

    #[test]
    fn golden_timezone_tehran() {
        let d = JalaliX::create(1_740_430_800_000);
        let mut tehran = d.setTimezone(Some("Asia/Tehran")).unwrap();
        assert_eq!(tehran.format("YYYY/MM/DD"), "1403/12/07");
        assert_eq!(tehran.format("HH:mm:ss"), "00:30:00");
        assert_eq!(
            format!(
                "{}-{:02}-{:02}",
                tehran.gregorian_full_year(),
                tehran.gregorian_month0() + 1,
                tehran.gregorian_date()
            ),
            "2025-02-25"
        );
    }

    #[test]
    fn golden_timezone_same_instant_utc() {
        let d = JalaliX::create(1_740_430_800_000);
        let mut utc = d.setTimezone(Some("UTC")).unwrap();
        assert_eq!(utc.format("YYYY/MM/DD"), "1403/12/06");
        assert_eq!(utc.format("HH:mm:ss"), "21:00:00");
        assert_eq!(
            format!(
                "{}-{:02}-{:02}",
                utc.gregorian_full_year(),
                utc.gregorian_month0() + 1,
                utc.gregorian_date()
            ),
            "2025-02-24"
        );
    }

    #[test]
    fn golden_timezone_utc_noon_tehran() {
        let d = JalaliX::create(1_740_484_800_000);
        let mut tehran = d.setTimezone(Some("Asia/Tehran")).unwrap();
        assert_eq!(tehran.format("YYYY/MM/DD"), "1403/12/07");
        assert_eq!(tehran.format("HH:mm:ss"), "15:30:00");
    }

    #[test]
    fn alias_utc() {
        let d = JalaliX::create(1_740_484_800_000);
        let d2 = d.setTimezone(Some("utc")).unwrap();
        assert_eq!(d2.getTimezone(), "UTC");
    }
}
