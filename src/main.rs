use chrono::{DateTime, Local, NaiveDateTime, TimeZone, Utc};
use clap::{Parser, ValueEnum};
use serde_json::json;

const EXIT_PARSE: i32 = 3;
const EXIT_TZ: i32 = 4;

#[derive(Copy, Clone, Debug, ValueEnum)]
enum TzChoice {
    Utc,
    Local,
}

impl TzChoice {
    fn as_str(&self) -> &'static str {
        match self {
            TzChoice::Utc => "UTC",
            TzChoice::Local => "local",
        }
    }
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum TsUnit {
    Seconds,
    Millis,
}

#[derive(Parser, Debug)]
#[command(name = "timeparse")]
#[command(about = "Parse a unix timestamp or a formatted datetime (YYYY/MM/DD HH:MM:SS).")]
struct Args {
    /// Timestamp (seconds/millis) OR formatted datetime: YYYY/MM/DD HH:MM:SS
    input: String,

    /// Output unix seconds only (single line)
    #[arg(long, conflicts_with_all = ["json"])]
    unix: bool,

    /// Output JSON only (single line)
    #[arg(long, conflicts_with_all = ["unix"])]
    json: bool,

    /// Custom output format (strftime). Only applies to string outputs (default RFC3339).
    #[arg(long)]
    format: Option<String>,

    /// Timezone used to interpret formatted input (YYYY/MM/DD HH:MM:SS). Default: local
    #[arg(long, value_enum, default_value_t = TzChoice::Local)]
    input_tz: TzChoice,

    /// Timezone used for formatted output. Default: UTC
    #[arg(long, value_enum, default_value_t = TzChoice::Utc)]
    output_tz: TzChoice,

    /// When INPUT is numeric, force interpretation: seconds or millis.
    /// If omitted, seconds vs millis is auto-detected.
    #[arg(long, value_enum)]
    ts: Option<TsUnit>,
}

#[derive(Debug)]
enum ParsedAs {
    Timestamp { unit: TsUnit, raw: i64 },
    Formatted,
}

fn die(code: i32, msg: impl AsRef<str>) -> ! {
    eprintln!("{}", msg.as_ref());
    std::process::exit(code);
}

/// Convert a numeric timestamp into a UTC DateTime, using forced or autodetected unit.
fn parse_timestamp_to_utc(
    raw: i64,
    forced: Option<TsUnit>,
) -> Result<(DateTime<Utc>, TsUnit), String> {
    let unit = forced.unwrap_or_else(|| {
        if raw.abs() >= 1_000_000_000_000 {
            TsUnit::Millis
        } else {
            TsUnit::Seconds
        }
    });

    let (secs, nanos) = match unit {
        TsUnit::Seconds => (raw, 0u32),
        TsUnit::Millis => {
            let secs = raw / 1000;
            let ms = (raw % 1000).abs() as u32;
            (secs, ms * 1_000_000)
        }
    };

    let dt = Utc
        .timestamp_opt(secs, nanos)
        .single()
        .ok_or_else(|| "Invalid unix timestamp".to_string())?;

    Ok((dt, unit))
}

/// Parse either numeric timestamp OR formatted datetime into UTC.
fn parse_input_to_utc(
    input: &str,
    input_tz: TzChoice,
    forced_ts: Option<TsUnit>,
) -> Result<(DateTime<Utc>, ParsedAs), (i32, String)> {
    // 1) numeric timestamp
    if let Ok(raw) = input.parse::<i64>() {
        return parse_timestamp_to_utc(raw, forced_ts)
            .map(|(dt, unit)| (dt, ParsedAs::Timestamp { unit, raw }))
            .map_err(|e| (EXIT_PARSE, e));
    }

    // 2) formatted datetime: YYYY/MM/DD HH:MM:SS
    let naive = NaiveDateTime::parse_from_str(input, "%Y/%m/%d %H:%M:%S").map_err(|_| {
        (
            EXIT_PARSE,
            "Expected format: YYYY/MM/DD HH:MM:SS".to_string(),
        )
    })?;

    let utc_dt = match input_tz {
        TzChoice::Utc => Utc.from_utc_datetime(&naive),
        TzChoice::Local => {
            let local_dt = Local.from_local_datetime(&naive).single().ok_or_else(|| {
                (
                    EXIT_TZ,
                    "Ambiguous or non-existent local time (DST transition)".to_string(),
                )
            })?;
            local_dt.with_timezone(&Utc)
        }
    };

    Ok((utc_dt, ParsedAs::Formatted))
}

fn format_output(utc_dt: DateTime<Utc>, output_tz: TzChoice, fmt: Option<&str>) -> String {
    match (output_tz, fmt) {
        (TzChoice::Utc, Some(f)) => utc_dt.format(f).to_string(),
        (TzChoice::Local, Some(f)) => utc_dt.with_timezone(&Local).format(f).to_string(),
        (TzChoice::Utc, None) => utc_dt.to_rfc3339(),
        (TzChoice::Local, None) => utc_dt.with_timezone(&Local).to_rfc3339(),
    }
}

fn main() {
    let args = Args::parse();

    let (utc_dt, parsed_as) = match parse_input_to_utc(&args.input, args.input_tz, args.ts) {
        Ok(v) => v,
        Err((code, msg)) => die(code, format!("Error: {msg}")),
    };

    // Always compute canonical unix outputs from UTC
    let unix_seconds = utc_dt.timestamp();
    let unix_millis = utc_dt.timestamp_millis();

    if args.unix {
        println!("{unix_seconds}");
        return;
    }

    if args.json {
        let (parsed_as_str, ts_unit_str) = match parsed_as {
            ParsedAs::Timestamp { unit, .. } => (
                "timestamp",
                Some(match unit {
                    TsUnit::Seconds => "seconds",
                    TsUnit::Millis => "millis",
                }),
            ),
            ParsedAs::Formatted => ("formatted", None),
        };

        let rfc3339_out = format_output(utc_dt, args.output_tz, None);

        let obj = json!({
            "schema_version": 1,
            "input": args.input,
            "parsed_as": parsed_as_str,
            "ts_unit": ts_unit_str,
            "input_tz": args.input_tz.as_str(),
            "output_tz": args.output_tz.as_str(),
            "unix_seconds": unix_seconds,
            "unix_millis": unix_millis,
            "rfc3339": rfc3339_out
        });

        println!("{}", serde_json::to_string_pretty(&obj).unwrap());
        return;
    }

    // Default: single-line string output (RFC3339 unless --format provided)
    let out = format_output(utc_dt, args.output_tz, args.format.as_deref());
    println!("{out}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn parses_seconds_timestamp() {
        let (dt, parsed_as) = parse_input_to_utc("1700000000", TzChoice::Utc, None).unwrap();
        assert_eq!(dt.timestamp(), 1_700_000_000);

        match parsed_as {
            ParsedAs::Timestamp { unit, raw } => {
                assert_eq!(raw, 1_700_000_000);
                assert!(matches!(unit, TsUnit::Seconds));
            }
            _ => panic!("expected timestamp parse"),
        }
    }

    #[test]
    fn parses_millis_timestamp_autodetect() {
        let (dt, parsed_as) = parse_input_to_utc("1700000000123", TzChoice::Utc, None).unwrap();
        assert_eq!(dt.timestamp(), 1_700_000_000);
        assert_eq!(dt.timestamp_millis(), 1_700_000_000_123);

        match parsed_as {
            ParsedAs::Timestamp { unit, .. } => assert!(matches!(unit, TsUnit::Millis)),
            _ => panic!("expected timestamp parse"),
        }
    }

    #[test]
    fn parses_millis_timestamp_forced() {
        let (dt, parsed_as) =
            parse_input_to_utc("1700000000", TzChoice::Utc, Some(TsUnit::Millis)).unwrap();
        assert_eq!(dt.timestamp_millis(), 1_700_000_000);

        match parsed_as {
            ParsedAs::Timestamp { unit, .. } => assert!(matches!(unit, TsUnit::Millis)),
            _ => panic!("expected timestamp parse"),
        }
    }

    #[test]
    fn parses_formatted_datetime_as_utc_when_input_tz_utc() {
        let (dt, parsed_as) =
            parse_input_to_utc("2025/12/20 11:10:11", TzChoice::Utc, None).unwrap();

        let expected = Utc.with_ymd_and_hms(2025, 12, 20, 11, 10, 11).unwrap();
        assert_eq!(dt, expected);

        assert!(matches!(parsed_as, ParsedAs::Formatted));
    }

    #[test]
    fn rejects_unknown_format() {
        let err = parse_input_to_utc("2025-12-20 11:10:11", TzChoice::Utc, None).unwrap_err();
        assert_eq!(err.0, EXIT_PARSE);
    }

    #[test]
    fn formats_default_rfc3339_utc() {
        let dt = Utc.with_ymd_and_hms(2025, 12, 20, 11, 10, 11).unwrap();
        let out = format_output(dt, TzChoice::Utc, None);
        assert!(out.starts_with("2025-12-20T11:10:11"));
    }

    #[test]
    fn formats_custom_format_utc() {
        let dt = Utc.with_ymd_and_hms(2025, 12, 20, 11, 10, 11).unwrap();
        let out = format_output(dt, TzChoice::Utc, Some("%Y/%m/%d %H:%M:%S"));
        assert_eq!(out, "2025/12/20 11:10:11");
    }
}
