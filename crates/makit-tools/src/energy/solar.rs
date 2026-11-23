//! The sun: where it is each hour (`solarpos.py`), what it puts through the
//! windows each month (`solargains.py`).

/// Days since 0001-01-01 (which is day 1) in the proleptic Gregorian
/// calendar: Python's `date.toordinal()`.
pub(crate) fn day_ordinal(year: i32, month: u32, day: u32) -> i64 {
    const BEFORE_MONTH: [i64; 12] = [0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334];
    let y = i64::from(year) - 1;
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let leap_day = i64::from(leap && month > 2);
    y * 365 + y / 4 - y / 100
        + y / 400
        + BEFORE_MONTH[(month - 1) as usize]
        + leap_day
        + i64::from(day)
}

/// Where the sun is at the start of local standard hour `hour` (0 to 23) on
/// the given day, seen from `lat`, `lon` (degrees, east positive) in time
/// zone `tz` (hours from UTC): its true (unrefracted) elevation and its
/// azimuth clockwise from north, in degrees.
///
/// The NOAA Solar Calculator equations, as murb's `solarpos.py` writes them.
/// Python's `%` takes the divisor's sign, hence `rem_euclid`.
pub fn solar_position(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    tz: f64,
    lat: f64,
    lon: f64,
) -> (f64, f64) {
    let hour = f64::from(hour);
    // Julian day at the given local-standard instant, converted to UTC
    let jd = day_ordinal(year, month, day) as f64 + 1721424.5 + (hour - tz) / 24.0;
    let jc = (jd - 2451545.0) / 36525.0;

    let geom_mean_long = (280.46646 + jc * (36000.76983 + jc * 0.0003032)).rem_euclid(360.0);
    let geom_mean_anom = 357.52911 + jc * (35999.05029 - 0.0001537 * jc);
    let ecc = 0.016708634 - jc * (0.000042037 + 0.0000001267 * jc);
    let ma = geom_mean_anom.to_radians();
    let eq_of_center = ma.sin() * (1.914602 - jc * (0.004817 + 0.000014 * jc))
        + (2.0 * ma).sin() * (0.019993 - 0.000101 * jc)
        + (3.0 * ma).sin() * 0.000289;
    let true_long = geom_mean_long + eq_of_center;
    let app_long = true_long - 0.00569 - 0.00478 * (125.04 - 1934.136 * jc).to_radians().sin();

    let mean_obliq =
        23.0 + (26.0 + (21.448 - jc * (46.815 + jc * (0.00059 - jc * 0.001813))) / 60.0) / 60.0;
    let obliq_corr = mean_obliq + 0.00256 * (125.04 - 1934.136 * jc).to_radians().cos();

    let decl = (obliq_corr.to_radians().sin() * app_long.to_radians().sin())
        .asin()
        .to_degrees();

    let var_y = (obliq_corr / 2.0).to_radians().tan().powi(2);
    let gml = geom_mean_long.to_radians();
    let eq_time = 4.0
        * (var_y * (2.0 * gml).sin() - 2.0 * ecc * ma.sin()
            + 4.0 * ecc * var_y * ma.sin() * (2.0 * gml).cos()
            - 0.5 * var_y * var_y * (4.0 * gml).sin()
            - 1.25 * ecc * ecc * (2.0 * ma).sin())
        .to_degrees(); // minutes

    let true_solar_min = (hour * 60.0 + eq_time + 4.0 * lon - 60.0 * tz).rem_euclid(1440.0);
    // With true solar time in [0, 1440) the hour angle is in [-180, 180).
    let ha = true_solar_min / 4.0 - 180.0;

    let (lat_r, decl_r, ha_r) = (lat.to_radians(), decl.to_radians(), ha.to_radians());
    let cos_zen =
        (lat_r.sin() * decl_r.sin() + lat_r.cos() * decl_r.cos() * ha_r.cos()).clamp(-1.0, 1.0);
    let zen = cos_zen.acos().to_degrees();
    let elevation = 90.0 - zen;

    let sin_zen = zen.to_radians().sin();
    let azimuth = if sin_zen.abs() < 1e-9 {
        180.0
    } else {
        let cos_az =
            ((lat_r.sin() * cos_zen - decl_r.sin()) / (lat_r.cos() * sin_zen)).clamp(-1.0, 1.0);
        let theta = cos_az.acos().to_degrees();
        if ha > 0.0 {
            (theta + 180.0).rem_euclid(360.0)
        } else {
            (540.0 - theta).rem_euclid(360.0)
        }
    };
    (elevation, azimuth)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::energy::test_support::{assert_close, parity};

    #[test]
    fn day_ordinal_matches_pythons_toordinal() {
        assert_eq!(day_ordinal(1, 1, 1), 1);
        assert_eq!(day_ordinal(2021, 1, 1), 737791);
        assert_eq!(day_ordinal(2021, 12, 31), 738155);
        assert_eq!(day_ordinal(2000, 3, 1), 730180);
    }

    #[test]
    fn solar_position_matches_the_python_port() {
        let cases = parity()["solar_position"].as_array().unwrap();
        assert_eq!(cases.len(), 10);
        for case in cases {
            let int = |k: &str| case[k].as_u64().unwrap() as u32;
            let num = |k: &str| case[k].as_f64().unwrap();
            let (elevation, azimuth) = solar_position(
                case["year"].as_i64().unwrap() as i32,
                int("month"),
                int("day"),
                int("hour"),
                num("tz"),
                num("lat"),
                num("lon"),
            );
            let label = format!(
                "{} {}/{} {}h",
                case["place"].as_str().unwrap(),
                int("month"),
                int("day"),
                int("hour")
            );
            assert_close(&format!("{label} elevation"), elevation, num("elevation"));
            assert_close(&format!("{label} azimuth"), azimuth, num("azimuth"));
        }
    }
}
