//! JS's `Number#toString()` for finite numbers, which `JSON.stringify` writes.

/// `String(x)` for a JS number: the shortest digits that round-trip, laid
/// out as ECMAScript's Number::toString lays them out (`1e+21`, `1e-7`,
/// `0.000001`, `-0` as `0`).
pub fn number_to_string(x: f64) -> String {
    if x.is_nan() {
        return "NaN".to_owned();
    }
    if x == 0.0 {
        return "0".to_owned();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.to_owned();
    }
    if x < 0.0 {
        return format!("-{}", number_to_string(-x));
    }
    // Rust's `{:e}` writes the shortest round-trip digits, as JS chooses them:
    // `d[.ddd]e<exp>`.
    let exp_form = format!("{x:e}");
    let (mantissa, exponent) = exp_form.split_once('e').expect("exponent");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    // x = 0.d₁d₂…dₖ × 10ⁿ
    let n = exponent.parse::<i32>().expect("exponent") + 1;
    if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        let (int, frac) = digits.split_at(n as usize);
        format!("{int}.{frac}")
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let sign = if e >= 0 { '+' } else { '-' };
        let (first, rest) = digits.split_at(1);
        if rest.is_empty() {
            format!("{first}e{sign}{}", e.abs())
        } else {
            format!("{first}.{rest}e{sign}{}", e.abs())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::number_to_string;

    #[test]
    fn matches_js() {
        // Expected values are what `String(x)` returns in Node.
        let cases: &[(f64, &str)] = &[
            (0.0, "0"),
            (-0.0, "0"),
            (1.0, "1"),
            (-1.5, "-1.5"),
            (2147483647.0, "2147483647"),
            (0.1 + 0.2, "0.30000000000000004"),
            (1e21, "1e+21"),
            (123456789012345680000.0, "123456789012345680000"),
            (1e-6, "0.000001"),
            (1e-7, "1e-7"),
            (1.5e-7, "1.5e-7"),
            (1e300, "1e+300"),
            (5e-324, "5e-324"),
            (f64::MAX, "1.7976931348623157e+308"),
            (100.0, "100"),
            (0.5, "0.5"),
        ];
        for &(x, want) in cases {
            assert_eq!(number_to_string(x), want, "{x:e}");
        }
    }
}
