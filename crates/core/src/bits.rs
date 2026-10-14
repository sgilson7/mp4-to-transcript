//! The models hand back 32-bit floating-point numbers. The core keeps no
//! floats (reference default D2), so it reads them as bit patterns: an order
//! key for comparing two of them, and a fixed-point integer for arithmetic.
//! Both are exact integer operations, so every run gives the same answer.

/// The 32-bit words in little-endian `bytes`. A trailing partial word is
/// ignored; callers check the length first.
pub fn words(bytes: &[u8]) -> Vec<u32> {
    let (whole, _) = bytes.as_chunks::<4>();
    whole.iter().map(|c| u32::from_le_bytes(*c)).collect()
}

/// An integer that sorts the way the number does: `key(a) < key(b)` exactly
/// when `a < b`, for every value that is not NaN. Negative numbers have the
/// sign bit set and grow downward, so their bits are flipped.
pub fn order_key(bits: u32) -> u32 {
    if bits & 0x8000_0000 != 0 {
        !bits
    } else {
        bits | 0x8000_0000
    }
}

/// The number times `2^frac`, rounded toward zero. NaN and infinity read as
/// zero; a magnitude too large for the result saturates.
pub fn fixed(bits: u32, frac: u32) -> i64 {
    let negative = bits & 0x8000_0000 != 0;
    let exponent = ((bits >> 23) & 0xff) as i64;
    let mantissa = (bits & 0x007f_ffff) as i64;
    if exponent == 0xff {
        return 0;
    }
    // value = significand * 2^power
    let (significand, power) = if exponent == 0 {
        (mantissa, -149)
    } else {
        (mantissa | 0x0080_0000, exponent - 150)
    };
    let shift = power + frac as i64;
    let magnitude = if shift >= 0 {
        if shift > 38 {
            i64::MAX / 2
        } else {
            significand << shift
        }
    } else if shift <= -63 {
        0
    } else {
        significand >> -shift
    };
    if negative {
        -magnitude
    } else {
        magnitude
    }
}

/// The integer square root: the largest `r` with `r * r <= n`.
pub fn isqrt(n: u128) -> u128 {
    if n < 2 {
        return n;
    }
    // Newton's method from a power of two above the root.
    let bits = 128 - n.leading_zeros();
    let mut x: u128 = 1 << bits.div_ceil(2);
    loop {
        let y = (x + n / x) / 2;
        if y >= x {
            return x;
        }
        x = y;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Bit patterns worked out by hand from IEEE 754 single precision:
    // 1 = 0x3F80_0000, 0.5 = 0x3F00_0000, 0.25 = 0x3E80_0000,
    // 3 = 0x4040_0000, -2 = 0xC000_0000, -0.5 = 0xBF00_0000.
    const ONE: u32 = 0x3F80_0000;
    const HALF: u32 = 0x3F00_0000;
    const QUARTER: u32 = 0x3E80_0000;
    const THREE: u32 = 0x4040_0000;
    const MINUS_TWO: u32 = 0xC000_0000;
    const MINUS_HALF: u32 = 0xBF00_0000;

    #[test]
    fn words_are_read_little_endian() {
        assert_eq!(
            words(&[0x00, 0x00, 0x80, 0x3F, 0x01, 0x00, 0x00, 0x00, 0xFF]),
            vec![ONE, 1]
        );
    }

    #[test]
    fn order_keys_sort_like_the_numbers() {
        let mut values = [THREE, MINUS_HALF, ONE, MINUS_TWO, 0, QUARTER];
        values.sort_by_key(|b| order_key(*b));
        assert_eq!(values, [MINUS_TWO, MINUS_HALF, 0, QUARTER, ONE, THREE]);
    }

    #[test]
    fn fixed_point_values_are_exact_for_simple_numbers() {
        assert_eq!(fixed(ONE, 20), 1 << 20);
        assert_eq!(fixed(HALF, 20), 1 << 19);
        assert_eq!(fixed(QUARTER, 4), 4);
        assert_eq!(fixed(THREE, 0), 3);
        assert_eq!(fixed(MINUS_TWO, 10), -2048);
        assert_eq!(fixed(MINUS_HALF, 1), -1);
        assert_eq!(fixed(0, 20), 0);
    }

    #[test]
    fn fixed_point_rounds_toward_zero_and_ignores_nan() {
        // 0.25 with no fraction bits is 0; -0.5 likewise.
        assert_eq!(fixed(QUARTER, 0), 0);
        assert_eq!(fixed(MINUS_HALF, 0), 0);
        assert_eq!(fixed(0x7FC0_0000, 20), 0);
        assert_eq!(fixed(0x7F80_0000, 20), 0);
    }

    #[test]
    fn integer_square_roots() {
        assert_eq!(isqrt(0), 0);
        assert_eq!(isqrt(1), 1);
        assert_eq!(isqrt(15), 3);
        assert_eq!(isqrt(16), 4);
        assert_eq!(isqrt(1_000_000), 1000);
        assert_eq!(isqrt(999_999), 999);
    }
}
