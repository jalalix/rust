// Shared golden parity tests (`testdata/golden.json`).
//
// The golden file is embedded so the crate always ships with the shared fixture.
// Cases mirror `fixtures/golden.json` / TypeScript `golden.test.mjs`.

use jalalix::{Calendar, JalaliFormatOptions, JalaliOptions, JalaliX};

const GOLDEN_JSON: &str =
    include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/testdata/golden.json"));

fn create_from_iso(iso: &str) -> JalaliX {
    JalaliX::createFromString(iso).expect("createFromString")
}

fn create_from_ms_tz(ms: i64, timezone: &str) -> JalaliX {
    JalaliX::create(ms)
        .setTimezone(Some(timezone))
        .expect("setTimezone")
}

#[test]
fn golden_file_embedded() {
    assert!(
        GOLDEN_JSON.contains("\"version\": 1") || GOLDEN_JSON.contains("\"version\":1"),
        "golden.json missing version"
    );
    assert!(GOLDEN_JSON.contains("1403/12/07"));
    assert!(GOLDEN_JSON.contains("Asia/Tehran"));
    assert!(GOLDEN_JSON.contains("add_months_start_of_month"));
}

#[test]
fn golden_calendar() {
    // iso_date_local_create
    let mut d = create_from_iso("2025-02-25");
    assert_eq!(d.format("YYYY/MM/DD"), "1403/12/07");
    assert_eq!(d.toISODate(Calendar::Gregorian), "2025-02-25");
    assert_eq!(d.toISODate(Calendar::Jalali), "1403-12-07");

    // utc_noon_create
    let mut d = create_from_ms_tz(1_740_484_800_000, "UTC");
    assert_eq!(d.format("YYYY/MM/DD"), "1403/12/07");
    assert_eq!(d.toISODate(Calendar::Gregorian), "2025-02-25");
    assert_eq!(d.toISODate(Calendar::Jalali), "1403-12-07");
}

#[test]
fn golden_parse() {
    let mut d = JalaliX::parseJalali("1403/12/07").unwrap();
    assert_eq!(d.toISODate(Calendar::Gregorian), "2025-02-25");
    assert_eq!(d.format("YYYY/MM/DD"), "1403/12/07");

    let mut d = JalaliX::parseJalali("1403-12-07").unwrap();
    assert_eq!(d.toISODate(Calendar::Gregorian), "2025-02-25");

    let mut d = JalaliX::parseJalali("۱۴۰۳/۱۲/۰۷").unwrap();
    assert_eq!(d.toISODate(Calendar::Gregorian), "2025-02-25");
    assert_eq!(d.format("YYYY/MM/DD"), "1403/12/07");
}

#[test]
fn golden_format() {
    let mut d = create_from_iso("2025-02-25");
    let opts = JalaliFormatOptions {
        calendar: Calendar::Jalali,
        persian_digits: true,
    };
    assert_eq!(d.formatWithOptions("DD MMM YYYY", &opts), "۰۷ اسفند ۱۴۰۳");

    let opts_g = JalaliFormatOptions {
        calendar: Calendar::Gregorian,
        persian_digits: false,
    };
    assert_eq!(d.formatWithOptions("YYYY-MM-DD", &opts_g), "2025-02-25");

    assert_eq!(JalaliX::toPersian("1403/12/07"), "۱۴۰۳/۱۲/۰۷");
    assert_eq!(JalaliX::fromPersian("۱۴۰۳/۱۲/۰۷"), "1403/12/07");
}

#[test]
fn golden_timezone() {
    let ms = 1_740_430_800_000_i64;

    let mut tehran = create_from_ms_tz(ms, "Asia/Tehran");
    assert_eq!(tehran.format("YYYY/MM/DD"), "1403/12/07");
    assert_eq!(tehran.toISODate(Calendar::Jalali), "1403-12-07");
    assert_eq!(tehran.toISODate(Calendar::Gregorian), "2025-02-25");
    assert_eq!(tehran.format("HH:mm:ss"), "00:30:00");
    assert_eq!(tehran.getHours(), 0);
    assert_eq!(tehran.getMinutes(), 30);
    assert_eq!(tehran.getTime(), ms);

    let mut utc = create_from_ms_tz(ms, "UTC");
    assert_eq!(utc.format("YYYY/MM/DD"), "1403/12/06");
    assert_eq!(utc.toISODate(Calendar::Jalali), "1403-12-06");
    assert_eq!(utc.toISODate(Calendar::Gregorian), "2025-02-24");
    assert_eq!(utc.format("HH:mm:ss"), "21:00:00");
    assert_eq!(utc.getHours(), 21);
    assert_eq!(utc.getMinutes(), 0);
    assert_eq!(utc.getTime(), ms);

    let noon = 1_740_484_800_000_i64;
    let mut tehran_noon = create_from_ms_tz(noon, "Asia/Tehran");
    assert_eq!(tehran_noon.format("YYYY/MM/DD"), "1403/12/07");
    assert_eq!(tehran_noon.toISODate(Calendar::Gregorian), "2025-02-25");
    assert_eq!(tehran_noon.format("HH:mm:ss"), "15:30:00");
    assert_eq!(tehran_noon.getHours(), 15);
    assert_eq!(tehran_noon.getMinutes(), 30);

    let aliased = JalaliX::create(noon).setTimezone(Some("utc")).unwrap();
    assert_eq!(aliased.getTimezone(), "UTC");
}

#[test]
fn golden_mutations() {
    let mut d = create_from_iso("2025-02-25");
    d.addMonths(1).getStartOfMonth();
    assert_eq!(d.format("YYYY/MM/DD"), "1404/01/01");

    let mut d = create_from_iso("2025-02-25");
    d.setFullYear(1404, Some(0), Some(1));
    assert_eq!(d.getMonth(), 1);
    assert_eq!(d.format("YYYY/MM/DD"), "1404/01/01");
}

#[test]
fn golden_create_with_timezone_option() {
    // Ensure JalaliOptions path used by contract still works for ISO create.
    let opts = JalaliOptions {
        timezone: Some("UTC".to_string()),
    };

    let mut d = JalaliX::createFromStringWithOptions("2025-02-25", Some(opts)).unwrap();
    assert_eq!(d.format("YYYY/MM/DD"), "1403/12/07");
    assert_eq!(d.getTimezone(), "UTC");
}
