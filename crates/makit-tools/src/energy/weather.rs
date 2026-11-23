//! Hourly weather from an EPW file (`epw.py`) and its monthly degree hours
//! (`utilities.py`).

use std::path::Path;

use super::solar::solar_position;
use super::{EnergyError, Monthly};

/// The rows before the hourly data in every EPW file.
const HEADER_ROWS: usize = 8;

/// murb reads every EPW as this year, whatever the file says.
const YEAR: i32 = 2021;

const DAYS_IN_MONTH: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

/// Where the weather was recorded: the EPW's `LOCATION` header.
#[derive(Debug, Clone, PartialEq)]
pub struct Location {
    /// The station, as the file names it ("Toronto.City-Univ.of.Toronto").
    pub city: String,
    pub latitude: f64,
    /// Degrees, east positive.
    pub longitude: f64,
    /// Hours from UTC.
    pub time_zone: f64,
    /// Metres.
    pub elevation: f64,
}

/// One hour of weather, with where the sun is.
#[derive(Debug, Clone, PartialEq)]
pub struct Hour {
    pub month: u32,
    pub day: u32,
    /// 0 to 23: the start of the hour, local standard time (the EPW says 1
    /// to 24, the hour's end).
    pub hour: u32,
    /// Dry-bulb temperature, °C.
    pub temp_air: f64,
    /// Global horizontal, direct normal and diffuse horizontal radiation, Wh/m².
    pub ghi: f64,
    pub dni: f64,
    pub dhi: f64,
    /// True (unrefracted) elevation and azimuth clockwise from north, degrees.
    pub sun_elevation: f64,
    pub sun_azimuth: f64,
}

/// A year of hourly weather.
#[derive(Debug, Clone, PartialEq)]
pub struct Weather {
    pub location: Location,
    pub hours: Vec<Hour>,
}

/// The `LOCATION` header: `LOCATION,city,state,country,source,WMO,lat,lon,TZ,elevation`.
pub fn parse_location(line: &str) -> Result<Location, EnergyError> {
    let error = |message: String| EnergyError::Epw { line: 1, message };
    let line = line.strip_prefix('\u{feff}').unwrap_or(line);
    let fields: Vec<&str> = line.split(',').collect();
    if fields[0].trim() != "LOCATION" {
        return Err(error("the first line is not a LOCATION header".into()));
    }
    if fields.len() < 10 {
        return Err(error(format!(
            "the LOCATION header has {} fields, expected 10",
            fields.len()
        )));
    }
    let number = |i: usize| {
        fields[i].trim().parse::<f64>().map_err(|_| {
            error(format!(
                "LOCATION field {i} ('{}') is not a number",
                fields[i]
            ))
        })
    };
    Ok(Location {
        city: fields[1].to_string(),
        latitude: number(6)?,
        longitude: number(7)?,
        time_zone: number(8)?,
        elevation: number(9)?,
    })
}

impl Weather {
    /// The weather in an EPW file's text. A byte order mark and any of LF,
    /// CRLF or CR line endings are accepted, as murb's reader accepts them.
    pub fn parse(text: &str) -> Result<Weather, EnergyError> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let mut lines = text.split('\n');
        let location = parse_location(lines.next().unwrap_or(""))?;
        let mut hours = Vec::new();
        for (i, line) in text.split('\n').enumerate().skip(HEADER_ROWS) {
            if line.trim().is_empty() {
                continue;
            }
            hours.push(parse_hour(line, i + 1, &location)?);
        }
        if hours.is_empty() {
            return Err(EnergyError::Epw {
                line: HEADER_ROWS + 1,
                message: format!("no hourly data after the {HEADER_ROWS} header lines"),
            });
        }
        Ok(Weather { location, hours })
    }

    /// The weather in the EPW file at `path`. Bytes that are not UTF-8 are
    /// replaced, not refused, as murb's reader does.
    pub fn read(path: &Path) -> Result<Weather, EnergyError> {
        let bytes = std::fs::read(path).map_err(|source| EnergyError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Weather::parse(&String::from_utf8_lossy(&bytes))
    }

    /// `value` of each hour, added up month by month in file order.
    pub fn monthly_sum(&self, value: impl Fn(&Hour) -> f64) -> Monthly {
        let mut out = [0.0; 12];
        for hour in &self.hours {
            out[(hour.month - 1) as usize] += value(hour);
        }
        out
    }
}

/// One data row, `line` being its 1-based line number.
fn parse_hour(text: &str, line: usize, location: &Location) -> Result<Hour, EnergyError> {
    let error = |message: String| EnergyError::Epw { line, message };
    let fields: Vec<&str> = text.split(',').collect();
    if fields.len() < 16 {
        return Err(error(format!(
            "expected at least 16 fields, found {}",
            fields.len()
        )));
    }
    let whole = |i: usize| {
        fields[i]
            .trim()
            .parse::<u32>()
            .map_err(|_| error(format!("field {i} ('{}') is not a whole number", fields[i])))
    };
    let number = |i: usize| {
        fields[i]
            .trim()
            .parse::<f64>()
            .map_err(|_| error(format!("field {i} ('{}') is not a number", fields[i])))
    };
    let (month, day, hour) = (whole(1)?, whole(2)?, whole(3)?);
    if !(1..=12).contains(&month) || day == 0 || day > DAYS_IN_MONTH[(month - 1) as usize] {
        return Err(error(format!(
            "month {month}, day {day} is not a date in {YEAR}, the year murb reads every weather file as"
        )));
    }
    if !(1..=24).contains(&hour) {
        return Err(error(format!("hour {hour} is outside 1 to 24")));
    }
    let hour = hour - 1;
    let (sun_elevation, sun_azimuth) = solar_position(
        YEAR,
        month,
        day,
        hour,
        location.time_zone,
        location.latitude,
        location.longitude,
    );
    Ok(Hour {
        month,
        day,
        hour,
        temp_air: number(6)?,
        ghi: number(13)?,
        dni: number(14)?,
        dhi: number(15)?,
        sun_elevation,
        sun_azimuth,
    })
}

/// A year of weather summed up month by month for the heat balance: hours,
/// hours needing heating or cooling, and degree hours (kKh) against the air
/// and against the ground (the year's mean temperature). Named as murb's
/// `MonthlyWeather`.
#[derive(Debug, Clone, PartialEq)]
pub struct DegreeHours {
    pub hours: Monthly,
    pub htg_hours: Monthly,
    pub clg_hours: Monthly,
    pub htg_degree_hrs: Monthly,
    pub clg_degree_hrs: Monthly,
    pub htg_degree_hrs_w_clg_setpoint: Monthly,
    pub htg_degree_hrs_ground: Monthly,
    pub clg_degree_hrs_ground: Monthly,
    pub htg_degree_hrs_w_clg_setpoint_ground: Monthly,
}

/// Python's `max(a, 0.0)`: `a` unless zero is greater.
fn max0(a: f64) -> f64 {
    if 0.0 > a {
        0.0
    } else {
        a
    }
}

/// murb's `get_degree_hours_from_weather_data`. An hour at exactly the
/// heating set-point counts as a heating hour, and one at exactly the cooling
/// set-point as a cooling hour, as in murb.
pub fn degree_hours(weather: &Weather, setpoint_htg: f64, setpoint_clg: f64) -> DegreeHours {
    let t_mean =
        weather.hours.iter().fold(0.0, |sum, h| sum + h.temp_air) / weather.hours.len() as f64;
    let ground_htg = max0(setpoint_htg - t_mean) / 1000.0;
    let ground_clg = max0(t_mean - setpoint_clg) / 1000.0;
    let ground_htg_w_clg = max0(setpoint_clg - t_mean) / 1000.0;
    DegreeHours {
        hours: weather.monthly_sum(|_| 1.0),
        htg_hours: weather.monthly_sum(|h| if h.temp_air > setpoint_htg { 0.0 } else { 1.0 }),
        clg_hours: weather.monthly_sum(|h| if h.temp_air < setpoint_clg { 0.0 } else { 1.0 }),
        htg_degree_hrs: weather.monthly_sum(|h| max0(setpoint_htg - h.temp_air) / 1000.0),
        clg_degree_hrs: weather.monthly_sum(|h| max0(h.temp_air - setpoint_clg) / 1000.0),
        htg_degree_hrs_w_clg_setpoint: weather
            .monthly_sum(|h| max0(setpoint_clg - h.temp_air) / 1000.0),
        htg_degree_hrs_ground: weather.monthly_sum(|_| ground_htg),
        clg_degree_hrs_ground: weather.monthly_sum(|_| ground_clg),
        htg_degree_hrs_w_clg_setpoint_ground: weather.monthly_sum(|_| ground_htg_w_clg),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::energy::test_support::{assert_close, assert_monthly, parity, synthetic_epw};
    use crate::energy::EnergyError;

    fn synthetic_text() -> String {
        std::fs::read_to_string(synthetic_epw()).unwrap()
    }

    /// The synthetic file with its 1-based line `n` replaced by `with`.
    fn with_line(n: usize, with: &str) -> String {
        let mut lines: Vec<String> = synthetic_text().lines().map(String::from).collect();
        lines[n - 1] = with.to_string();
        lines.join("\n")
    }

    /// Line `n` of the synthetic file with field `field` replaced by `value`.
    fn with_field(n: usize, field: usize, value: &str) -> String {
        let text = synthetic_text();
        let mut fields: Vec<String> = text
            .lines()
            .nth(n - 1)
            .unwrap()
            .split(',')
            .map(String::from)
            .collect();
        fields[field] = value.to_string();
        with_line(n, &fields.join(","))
    }

    fn epw_error(text: &str) -> (usize, String) {
        match Weather::parse(text) {
            Err(err @ EnergyError::Epw { line, .. }) => (line, err.to_string()),
            other => panic!("expected an EPW error, got {other:?}"),
        }
    }

    #[test]
    fn the_synthetic_weather_matches_the_python_reader() {
        let w = Weather::read(&synthetic_epw()).unwrap();
        let fixture = &parity()["weather"];
        assert_eq!(w.location.city, fixture["city"].as_str().unwrap());
        assert_close(
            "latitude",
            w.location.latitude,
            fixture["latitude"].as_f64().unwrap(),
        );
        assert_close(
            "longitude",
            w.location.longitude,
            fixture["longitude"].as_f64().unwrap(),
        );
        assert_close(
            "time zone",
            w.location.time_zone,
            fixture["time_zone"].as_f64().unwrap(),
        );
        assert_close(
            "elevation",
            w.location.elevation,
            fixture["elevation"].as_f64().unwrap(),
        );
        assert_eq!(w.hours.len(), 8760);
        assert_eq!(w.hours.len() as u64, fixture["rows"].as_u64().unwrap());
        for sample in fixture["samples"].as_array().unwrap() {
            let i = sample["index"].as_u64().unwrap() as usize;
            let h = &w.hours[i];
            let int = |k: &str| sample[k].as_u64().unwrap() as u32;
            let num = |k: &str| sample[k].as_f64().unwrap();
            assert_eq!(
                (h.month, h.day, h.hour),
                (int("month"), int("day"), int("hour")),
                "row {i}"
            );
            for (name, got) in [
                ("temp_air", h.temp_air),
                ("ghi", h.ghi),
                ("dni", h.dni),
                ("dhi", h.dhi),
                ("sun_elevation", h.sun_elevation),
                ("sun_azimuth", h.sun_azimuth),
            ] {
                assert_close(&format!("row {i} {name}"), got, num(name));
            }
        }
        let dh = degree_hours(&w, 21.0, 24.0);
        let want = &fixture["degree_hours"];
        for (name, got) in [
            ("hours", &dh.hours),
            ("htg_hours", &dh.htg_hours),
            ("clg_hours", &dh.clg_hours),
            ("htg_degree_hrs", &dh.htg_degree_hrs),
            ("clg_degree_hrs", &dh.clg_degree_hrs),
            (
                "htg_degree_hrs_w_clg_setpoint",
                &dh.htg_degree_hrs_w_clg_setpoint,
            ),
            ("htg_degree_hrs_ground", &dh.htg_degree_hrs_ground),
            ("clg_degree_hrs_ground", &dh.clg_degree_hrs_ground),
            (
                "htg_degree_hrs_w_clg_setpoint_ground",
                &dh.htg_degree_hrs_w_clg_setpoint_ground,
            ),
        ] {
            assert_monthly(name, got, &want[name]);
        }
    }

    #[test]
    fn a_byte_order_mark_and_crlf_line_endings_are_accepted() {
        let plain = Weather::parse(&synthetic_text()).unwrap();
        let crlf = format!("\u{feff}{}", synthetic_text().replace('\n', "\r\n"));
        let w = Weather::parse(&crlf).unwrap();
        assert_eq!(w.hours.len(), 8760);
        assert_eq!(w.location.city, "SYNTHETIC CALGARY");
        assert_eq!(w.hours[0], plain.hours[0]);
        // old Mac line endings, which Python's splitlines also reads
        let cr = Weather::parse(&synthetic_text().replace('\n', "\r")).unwrap();
        assert_eq!(cr.hours.len(), 8760);
    }

    #[test]
    fn invalid_utf8_is_replaced_not_fatal() {
        let mut bytes = std::fs::read(synthetic_epw()).unwrap();
        let at = bytes.windows(7).position(|w| w == b"CALGARY").unwrap();
        bytes[at] = 0xE9;
        let path =
            std::env::temp_dir().join(format!("makit-energy-utf8-{}.epw", std::process::id()));
        std::fs::write(&path, &bytes).unwrap();
        let w = Weather::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(w.location.city.contains('\u{FFFD}'), "{}", w.location.city);
        assert_eq!(w.hours.len(), 8760);
    }

    #[test]
    fn blank_lines_after_the_header_are_skipped() {
        let mut lines: Vec<&str> = Vec::new();
        let text = synthetic_text();
        for (i, line) in text.lines().enumerate() {
            lines.push(line);
            if i + 1 == 20 {
                lines.push("");
                lines.push("   ");
            }
        }
        assert_eq!(Weather::parse(&lines.join("\n")).unwrap().hours.len(), 8760);
    }

    #[test]
    fn a_short_row_names_its_line() {
        let (line, message) = epw_error(&with_line(9, "2021,1,1,1,60"));
        assert_eq!(line, 9);
        assert!(message.contains("at least 16 fields"), "{message}");
    }

    #[test]
    fn a_field_that_is_not_a_number_names_its_line() {
        let (line, message) = epw_error(&with_field(10, 6, "warm"));
        assert_eq!(line, 10);
        assert!(message.contains("warm"), "{message}");
    }

    #[test]
    fn an_hour_outside_1_to_24_is_an_error() {
        let (line, message) = epw_error(&with_field(9, 3, "25"));
        assert_eq!(line, 9);
        assert!(message.contains("25"), "{message}");
        assert_eq!(epw_error(&with_field(9, 3, "0")).0, 9);
    }

    #[test]
    fn february_29_is_an_error_naming_its_line() {
        let text = with_field(9, 1, "2");
        let text = {
            let mut fields: Vec<String> = text
                .lines()
                .nth(8)
                .unwrap()
                .split(',')
                .map(String::from)
                .collect();
            fields[2] = "29".to_string();
            let mut lines: Vec<String> = text.lines().map(String::from).collect();
            lines[8] = fields.join(",");
            lines.join("\n")
        };
        let (line, message) = epw_error(&text);
        assert_eq!(line, 9);
        assert!(message.contains("not a date"), "{message}");
        assert!(epw_error(&with_field(9, 1, "13")).1.contains("not a date"));
    }

    #[test]
    fn a_file_without_a_location_header_is_an_error() {
        let text = synthetic_text();
        let without: Vec<&str> = text.lines().skip(1).collect();
        assert_eq!(epw_error(&without.join("\n")).0, 1);
        let (line, message) = epw_error(&with_field(1, 6, "north"));
        assert_eq!(line, 1);
        assert!(message.contains("north"), "{message}");
    }

    #[test]
    fn a_file_with_no_hourly_rows_is_an_error() {
        let text = synthetic_text();
        let header: Vec<&str> = text.lines().take(8).collect();
        let (_, message) = epw_error(&header.join("\n"));
        assert!(message.contains("no hourly data"), "{message}");
        let (_, message) = epw_error("");
        assert!(message.contains("LOCATION"), "{message}");
    }

    #[test]
    fn reading_a_missing_file_names_its_path() {
        let path = std::path::Path::new("nowhere/at-all.epw");
        let err = Weather::read(path).unwrap_err();
        assert!(matches!(err, EnergyError::Io { .. }), "{err:?}");
        let message = err.to_string();
        assert!(
            message.contains("nowhere/at-all.epw") && message.contains("cannot read weather file"),
            "{message}"
        );
    }

    #[test]
    fn parse_location_reads_the_header_alone() {
        let loc = parse_location("\u{feff}LOCATION,Toronto.City-Univ.of.Toronto,ON,CAN,SRC-TMYx,715080,43.6658,-79.3953,-5.0,112.5").unwrap();
        assert_eq!(loc.city, "Toronto.City-Univ.of.Toronto");
        assert_eq!(
            (loc.latitude, loc.longitude, loc.time_zone, loc.elevation),
            (43.6658, -79.3953, -5.0, 112.5)
        );
    }
}
