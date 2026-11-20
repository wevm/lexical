//! `constants.ts`: `PIXEL_VALUE_REG_EXP` and `COLUMN_WIDTH`.

/// `COLUMN_WIDTH`: the playground theme's `.PlaygroundEditorTheme__tableCell`
/// width, the width a cell without one exports with and a new table's
/// columns get.
pub const COLUMN_WIDTH: f64 = 75.0;

/// `PIXEL_VALUE_REG_EXP.test(value) ? parseFloat(value) : undefined`: the
/// number of a value written `<digits>[.<digits>]px`.
pub fn parse_pixel_value(value: &str) -> Option<f64> {
    let number = value.strip_suffix("px")?;
    let (int, frac) = match number.split_once('.') {
        Some((int, frac)) => (int, Some(frac)),
        None => (number, None),
    };
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !digits(int) || frac.is_some_and(|f| !digits(f)) {
        return None;
    }
    number.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::parse_pixel_value;

    #[test]
    fn pixel_values() {
        assert_eq!(parse_pixel_value("75px"), Some(75.0));
        assert_eq!(parse_pixel_value("12.5px"), Some(12.5));
        for value in [
            "", "px", "75", "75 px", "-1px", ".5px", "5.px", "1e2px", "75pt",
        ] {
            assert_eq!(parse_pixel_value(value), None, "{value}");
        }
    }
}
