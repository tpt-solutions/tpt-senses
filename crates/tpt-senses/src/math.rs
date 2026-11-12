//! Minimal floating-point helpers that work without `std` (no `libm` dependency).

/// Square root for non-negative finite inputs, using Newton's method.
///
/// Inputs that are NaN or not positive return `0.0`; infinite inputs are returned unchanged.
/// Callers in this crate only pass sums of squares, so the negative branch is never reached in
/// practice.
#[must_use]
#[allow(
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::float_cmp,
    reason = "exponent extraction is exact; the convergence test is an intentional exact check"
)]
pub(crate) fn sqrt(x: f64) -> f64 {
    if x.is_nan() || x <= 0.0 {
        return 0.0;
    }
    if x.is_infinite() {
        return x;
    }
    // Initial guess: 2^(e/2), within a factor of sqrt(2) of the true root, so Newton converges
    // in a handful of steps.
    let exponent = ((x.to_bits() >> 52) & 0x7ff) as i64 - 1023;
    let mut guess = f64::from_bits(((exponent / 2 + 1023) as u64) << 52);
    for _ in 0..64 {
        let next = f64::midpoint(guess, x / guess);
        if next == guess {
            break;
        }
        guess = next;
    }
    guess
}

#[cfg(test)]
#[allow(
    clippy::float_cmp,
    reason = "edge-case results are exact by definition"
)]
mod tests {
    use super::sqrt;

    #[test]
    fn sqrt_squares_back_to_input() {
        for x in [0.25, 1.0, 2.0, 9.0, 1e-9, 1e9, 123.456, 1e300] {
            let r = sqrt(x);
            assert!(((r * r) - x).abs() <= 1e-12 * x, "sqrt({x}) = {r}");
        }
    }

    #[test]
    fn sqrt_edge_cases() {
        assert_eq!(sqrt(0.0), 0.0);
        assert_eq!(sqrt(-4.0), 0.0);
        assert_eq!(sqrt(f64::NAN), 0.0);
        assert_eq!(sqrt(f64::INFINITY), f64::INFINITY);
    }
}
