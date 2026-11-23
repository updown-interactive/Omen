use omen_format::*;
use std::sync::Arc;

#[test]
fn test_decimal_rounding() {
    let d = Decimal::from(12.345);
    // HalfEven rounds 12.345 to 12.34 because 4 is even
    let r1 = d.round_to_scale(2, RoundingMode::HalfEven);
    assert_eq!(r1.to_string(), "12.34");

    // HalfUp rounds 12.345 to 12.35
    let r2 = d.round_to_scale(2, RoundingMode::HalfUp);
    assert_eq!(r2.to_string(), "12.35");

    // TowardZero truncates to 12.34
    let r3 = d.round_to_scale(2, RoundingMode::TowardZero);
    assert_eq!(r3.to_string(), "12.34");

    // Floor of negative
    let neg = Decimal::from(-12.341);
    let r4 = neg.round_to_scale(2, RoundingMode::Floor);
    assert_eq!(r4.to_string(), "-12.35");
}

#[test]
fn test_number_formatting_locales() {
    let num = Decimal::from(1234567.89);

    // US locale
    let ctx_us = FormatContext::new("en-US");
    let opts = NumberOptions::default();
    let s_us = format_number(num, &opts, &ctx_us).unwrap();
    assert_eq!(s_us, "1,234,567.89");

    // German locale
    let ctx_de = FormatContext::new("de-DE");
    let s_de = format_number(num, &opts, &ctx_de).unwrap();
    assert_eq!(s_de, "1.234.567,89");

    // Indian grouping (en-IN / hi-IN): 12,34,567.89
    let ctx_in = FormatContext::new("en-IN");
    let s_in = format_number(num, &opts, &ctx_in).unwrap();
    assert_eq!(s_in, "12,34,567.89");

    // Compact notation
    let opts_compact = NumberOptions::compact();
    assert_eq!(format_number(1500, &opts_compact, &ctx_us).unwrap(), "1.5K");
    assert_eq!(format_number(2_400_000, &opts_compact, &ctx_us).unwrap(), "2.4M");
    assert_eq!(format_number(1_200_000_000, &opts_compact, &ctx_us).unwrap(), "1.2B");
}

#[test]
fn test_currency_formatting() {
    let ctx_us = FormatContext::new("en-US");
    let opts_usd = CurrencyOptions::new(CurrencyCode::USD);
    assert_eq!(
        format_currency(1234.5, &opts_usd, &ctx_us).unwrap(),
        "$1,234.50"
    );

    // Negative accounting notation ($1,234.50)
    let opts_acct = CurrencyOptions::new(CurrencyCode::USD).accounting();
    assert_eq!(
        format_currency(-1234.5, &opts_acct, &ctx_us).unwrap(),
        "($1,234.50)"
    );

    // INR in India
    let ctx_in = FormatContext::new("en-IN");
    let opts_inr = CurrencyOptions::new(CurrencyCode::INR);
    assert_eq!(
        format_currency(123456.75, &opts_inr, &ctx_in).unwrap(),
        "₹1,23,456.75"
    );

    // EUR in Germany (suffix with space)
    let ctx_de = FormatContext::new("de-DE");
    let opts_eur = CurrencyOptions::new(CurrencyCode::EUR);
    assert_eq!(
        format_currency(99.99, &opts_eur, &ctx_de).unwrap(),
        "99,99\u{00A0}€"
    );

    // JPY (0 fraction digits)
    let opts_jpy = CurrencyOptions::new(CurrencyCode::JPY);
    assert_eq!(
        format_currency(1250, &opts_jpy, &ctx_us).unwrap(),
        "¥1,250"
    );
}

#[test]
fn test_percent_formatting() {
    let ctx_us = FormatContext::new("en-US");
    let opts = PercentOptions::default();
    assert_eq!(format_percent(0.25, &opts, &ctx_us).unwrap(), "25%");
    assert_eq!(format_percent(0.045, &opts, &ctx_us).unwrap(), "4.5%");

    let opts_whole = PercentOptions::whole();
    assert_eq!(format_percent(25, &opts_whole, &ctx_us).unwrap(), "25%");

    let opts_signed = PercentOptions::default().signed();
    assert_eq!(format_percent(0.12, &opts_signed, &ctx_us).unwrap(), "+12%");

    // Turkish prefix
    let ctx_tr = FormatContext::new("tr-TR");
    assert_eq!(format_percent(0.25, &opts, &ctx_tr).unwrap(), "%25");
}

#[test]
fn test_date_formatting() {
    let date = PlainDate::new(2026, 11, 23).unwrap();
    let ctx_us = FormatContext::new("en-US");
    let ctx_de = FormatContext::new("de-DE");
    let ctx_ja = FormatContext::new("ja-JP");

    // Medium style
    assert_eq!(
        format_date(date, &DateOptions::medium(), &ctx_us).unwrap(),
        "Nov 23, 2026"
    );
    assert_eq!(
        format_date(date, &DateOptions::medium(), &ctx_de).unwrap(),
        "23. Nov 2026"
    );
    assert_eq!(
        format_date(date, &DateOptions::medium(), &ctx_ja).unwrap(),
        "2026年11月23日"
    );

    // Long style
    assert_eq!(
        format_date(date, &DateOptions::long(), &ctx_us).unwrap(),
        "November 23, 2026"
    );

    // Custom pattern
    let opts_pat = DateOptions::pattern("yyyy/MM/dd");
    assert_eq!(format_date(date, &opts_pat, &ctx_us).unwrap(), "2026/11/23");

    // Leap year check
    assert!(PlainDate::new(2024, 2, 29).is_ok());
    assert!(PlainDate::new(2023, 2, 29).is_err());
}

#[test]
fn test_time_formatting() {
    let time = PlainTime::from_hms(14, 30, 45).unwrap();
    let ctx_us = FormatContext::new("en-US");
    let ctx_de = FormatContext::new("de-DE");

    // US 12h
    assert_eq!(
        format_time(time, &TimeOptions::short(), &ctx_us).unwrap(),
        "02:30 PM"
    );
    assert_eq!(
        format_time(time, &TimeOptions::medium(), &ctx_us).unwrap(),
        "02:30:45 PM"
    );

    // DE 24h
    assert_eq!(
        format_time(time, &TimeOptions::short(), &ctx_de).unwrap(),
        "14:30"
    );
}

#[test]
fn test_datetime_and_timezone_conversion() {
    // 2026-11-23 00:00:00 UTC = 1795392000 seconds
    let instant = Instant::from_epoch_seconds(1795392000);
    let ctx = FormatContext::new("en-US");

    // In India (UTC+05:30), 00:00:00 UTC becomes 05:30:00 on the same date
    let ist = TimeZoneId::named("Asia/Kolkata");
    let opts = DateTimeOptions::short().time_zone(ist);
    let s = format_instant(instant, &opts, &ctx).unwrap();
    assert!(s.contains("05:30"));
}

#[test]
fn test_relative_time_and_boundaries() {
    let clock = Arc::new(MockClock::at(100_000));
    let opts = RelativeOptions::natural().with_clock(clock.clone());
    let ctx = FormatContext::new("en-US");

    // Just now (< 45s)
    let t1 = Instant::from_epoch_seconds(99_980);
    assert_eq!(format_relative(t1, &opts, &ctx).unwrap(), "just now");

    // Next update boundary for "just now"
    let boundary = next_update_boundary(t1, clock.now());
    assert_eq!(boundary.as_secs(), 25);

    // 5 minutes ago
    let t2 = Instant::from_epoch_seconds(99_700);
    assert_eq!(format_relative(t2, &opts, &ctx).unwrap(), "5 minutes ago");

    // Next update boundary for 5 minutes ago should be 60 seconds
    let b2 = next_update_boundary(t2, clock.now());
    assert_eq!(b2.as_secs(), 60);

    // Yesterday
    let t3 = Instant::from_epoch_seconds(100_000 - 86400);
    assert_eq!(format_relative(t3, &opts, &ctx).unwrap(), "yesterday");

    // Future
    let t4 = Instant::from_epoch_seconds(100_000 + 3600);
    assert_eq!(format_relative(t4, &opts, &ctx).unwrap(), "in 1 hour");
}

#[test]
fn test_duration_formatting() {
    let ctx = FormatContext::new("en-US");

    // Clock style
    let opts_clock = DurationOptions::clock();
    assert_eq!(format_duration(5528, &opts_clock, &ctx).unwrap(), "01:32:08");

    // Human style
    let opts_human = DurationOptions::human();
    assert_eq!(
        format_duration(5528, &opts_human, &ctx).unwrap(),
        "1 hour, 32 minutes, 8 seconds"
    );

    // Short style
    let opts_short = DurationOptions::short();
    assert_eq!(format_duration(5528, &opts_short, &ctx).unwrap(), "1h 32m 8s");
}

#[test]
fn test_measurement_and_conversion() {
    let ctx = FormatContext::new("en-US");

    // Standard formatting
    let dist = Measurement::kilometers(15);
    let opts = MeasurementOptions::unit(Unit::Kilometer);
    assert_eq!(format_measurement(dist, &opts, &ctx).unwrap(), "15\u{00A0}km");

    // Convert km to miles (15 km ≈ 9.32 miles)
    let opts_mi = MeasurementOptions::unit(Unit::Mile).precision(2, 2);
    assert_eq!(format_measurement(dist, &opts_mi, &ctx).unwrap(), "9.32\u{00A0}mi");

    // Affine Temperature conversion: 0°C -> 32°F
    let temp_c = Measurement::celsius(0);
    let opts_f = MeasurementOptions::unit(Unit::Fahrenheit);
    assert_eq!(format_measurement(temp_c, &opts_f, &ctx).unwrap(), "32\u{00A0}°F");
}

#[test]
fn test_byte_sizes_and_data_rates() {
    let ctx = FormatContext::new("en-US");

    // IEC Binary (1024)
    let opts_bin = ByteOptions::binary();
    assert_eq!(format_bytes(1024, &opts_bin, &ctx).unwrap(), "1\u{00A0}KiB");
    assert_eq!(format_bytes(1572864, &opts_bin, &ctx).unwrap(), "1.5\u{00A0}MiB");

    // SI Decimal (1000)
    let opts_dec = ByteOptions::decimal();
    assert_eq!(format_bytes(1000, &opts_dec, &ctx).unwrap(), "1\u{00A0}kB");
    assert_eq!(format_bytes(1500000, &opts_dec, &ctx).unwrap(), "1.5\u{00A0}MB");

    // Data rates
    let opts_rate = DataRateOptions::bytes_per_second();
    assert_eq!(
        format_data_rate(12_500_000.0, &opts_rate, &ctx).unwrap(),
        "12.5\u{00A0}MB/s"
    );

    let opts_bits = DataRateOptions::bits_per_second();
    assert_eq!(
        format_data_rate(100_000_000.0, &opts_bits, &ctx).unwrap(),
        "100\u{00A0}Mbps"
    );
}

#[test]
fn test_text_and_grapheme_truncation() {
    let ctx = FormatContext::new("en-US");

    // Title case
    let opts_title = TextOptions::title_case();
    assert_eq!(
        format_text("hello world from omen", &opts_title, &ctx).unwrap(),
        "Hello World From Omen"
    );

    // Grapheme truncation: Unicode emoji sequence must not be sliced by byte
    let text = "Hello 👩‍💻🚀 World";
    let opts_trunc = TextOptions::truncate(8);
    let truncated = format_text(text, &opts_trunc, &ctx).unwrap();
    // 8 graphemes: 'H', 'e', 'l', 'l', 'o', ' ', '👩‍💻', '🚀' + '…'
    assert_eq!(truncated, "Hello 👩‍💻🚀…");

    // Turkish dotted/dotless i
    let ctx_tr = FormatContext::new("tr-TR");
    let opts_up = TextOptions::uppercase();
    assert_eq!(format_text("istanbul", &opts_up, &ctx_tr).unwrap(), "İSTANBUL");
}

#[test]
fn test_list_and_plural_formatting() {
    let ctx_us = FormatContext::new("en-US");
    let ctx_de = FormatContext::new("de-DE");

    // Oxford comma in English
    let opts_conj = ListOptions::conjunction();
    assert_eq!(
        format_list(&["Alice", "Bob", "Charlie"], &opts_conj, &ctx_us).unwrap(),
        "Alice, Bob, and Charlie"
    );

    // German conjunction
    assert_eq!(
        format_list(&["Alice", "Bob", "Charlie"], &opts_conj, &ctx_de).unwrap(),
        "Alice, Bob und Charlie"
    );

    // Disjunction
    let opts_disj = ListOptions::disjunction();
    assert_eq!(
        format_list(&["Red", "Green", "Blue"], &opts_disj, &ctx_us).unwrap(),
        "Red, Green, or Blue"
    );

    // Ordinal numbers
    assert_eq!(format_ordinal(1, &ctx_us).unwrap(), "1st");
    assert_eq!(format_ordinal(2, &ctx_us).unwrap(), "2nd");
    assert_eq!(format_ordinal(3, &ctx_us).unwrap(), "3rd");
    assert_eq!(format_ordinal(4, &ctx_us).unwrap(), "4th");
    assert_eq!(format_ordinal(21, &ctx_us).unwrap(), "21st");
    assert_eq!(format_ordinal(1, &ctx_de).unwrap(), "1.");
}

#[test]
fn test_localized_messages() {
    let ctx = FormatContext::new("en-US");

    let template = "Hello {name}, you have {count, plural, one {# item} other {# items}} in your cart.";
    let args = [
        ("name", MessageArg::from("Ada")),
        ("count", MessageArg::from(1)),
    ];
    let msg1 = format_message(template, &args, &ctx).unwrap();
    assert_eq!(msg1, "Hello Ada, you have 1 item in your cart.");

    let args_plural = [
        ("name", MessageArg::from("Ada")),
        ("count", MessageArg::from(5)),
    ];
    let msg2 = format_message(template, &args_plural, &ctx).unwrap();
    assert_eq!(msg2, "Hello Ada, you have 5 items in your cart.");
}

#[test]
fn test_phone_and_address_formatting() {
    let ctx = FormatContext::new("en-US");

    let phone = PhoneNumber::parse("+14155552671", None).unwrap();
    assert_eq!(
        format_phone(&phone, &PhoneOptions::national(), &ctx).unwrap(),
        "(415) 555-2671"
    );
    assert_eq!(
        format_phone(&phone, &PhoneOptions::international(), &ctx).unwrap(),
        "+1 415 555 2671"
    );
    assert_eq!(
        format_phone(&phone, &PhoneOptions::e164(), &ctx).unwrap(),
        "+14155552671"
    );

    let address = PostalAddress {
        recipient: Some("Ada Lovelace".into()),
        street: "123 Main St".into(),
        locality: "San Francisco".into(),
        admin_area: Some("CA".into()),
        postal_code: "94105".into(),
        country_code: "US".into(),
    };
    let addr_str = format_address(&address, &AddressOptions::single_line(), &ctx).unwrap();
    assert_eq!(
        addr_str,
        "Ada Lovelace, 123 Main St, San Francisco, CA 94105, US"
    );
}

#[test]
fn test_parsing_api() {
    let ctx_us = FormatContext::new("en-US");
    let ctx_de = FormatContext::new("de-DE");

    // US number parsing
    let dec_us = parse::parse_number("1,234,567.89", &NumberOptions::default(), &ctx_us).unwrap();
    assert_eq!(dec_us.to_string(), "1234567.89");

    // German number parsing
    let dec_de = parse::parse_number("1.234.567,89", &NumberOptions::default(), &ctx_de).unwrap();
    assert_eq!(dec_de.to_string(), "1234567.89");

    // Currency parsing
    let money = parse::parse_currency("$1,234.50", &CurrencyOptions::new(CurrencyCode::USD), &ctx_us).unwrap();
    assert_eq!(money.amount.to_string(), "1234.50");
    assert_eq!(money.currency, CurrencyCode::USD);

    // Measurement parsing
    let m = parse::parse_measurement("15.5 km", &ctx_us).unwrap();
    assert_eq!(m.value, 15.5);
    assert_eq!(m.unit, Unit::Kilometer);
}
