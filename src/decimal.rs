//! Fixed-point decimal type with 4 decimal places precision.
//!
//! Uses `rust_decimal` internally with scale enforcement to ensure
//! consistent monetary calculations without floating-point errors.

use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::ops::{Add, AddAssign, Sub, SubAssign};
use std::str::FromStr;

/// A decimal type that maintains exactly 4 decimal places of precision.
///
/// This type wraps `rust_decimal::Decimal` and ensures consistent scale
/// for all arithmetic operations, suitable for monetary calculations.
///
/// # Examples
///
/// ```
/// use std::str::FromStr;
/// use payments_engine::Decimal4;
///
/// let amount = Decimal4::from_str("10.5").unwrap();
/// assert_eq!(amount.to_string(), "10.5000");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Decimal4(Decimal);

impl Decimal4 {
    /// The number of decimal places to maintain.
    pub const SCALE: u32 = 4;

    /// Zero value.
    pub const ZERO: Self = Decimal4(Decimal::ZERO);

    /// Creates a new `Decimal4` from a `Decimal`, normalizing to 4 decimal places.
    pub fn new(value: Decimal) -> Self {
        let mut normalized = value;
        normalized.rescale(Self::SCALE);
        Decimal4(normalized)
    }

    /// Returns `true` if this value is zero.
    pub fn is_zero(&self) -> bool {
        self.0.is_zero()
    }
}

impl FromStr for Decimal4 {
    type Err = rust_decimal::Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let trimmed = s.trim();
        let decimal = Decimal::from_str(trimmed)?;
        Ok(Decimal4::new(decimal))
    }
}

impl fmt::Display for Decimal4 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.4}", self.0)
    }
}

impl Add for Decimal4 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Decimal4::new(self.0 + rhs.0)
    }
}

impl AddAssign for Decimal4 {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
        self.0.rescale(Self::SCALE);
    }
}

impl Sub for Decimal4 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Decimal4::new(self.0 - rhs.0)
    }
}

impl SubAssign for Decimal4 {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
        self.0.rescale(Self::SCALE);
    }
}

impl Serialize for Decimal4 {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("{:.4}", self.0))
    }
}

impl<'de> Deserialize<'de> for Decimal4 {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Decimal4::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_str_normalizes_scale() {
        let d = Decimal4::from_str("1.0").unwrap();
        assert_eq!(d.to_string(), "1.0000");

        let d = Decimal4::from_str("1.5").unwrap();
        assert_eq!(d.to_string(), "1.5000");

        let d = Decimal4::from_str("1.1234").unwrap();
        assert_eq!(d.to_string(), "1.1234");

        let d = Decimal4::from_str("  2.5  ").unwrap();
        assert_eq!(d.to_string(), "2.5000");
    }

    #[test]
    fn test_arithmetic_preserves_scale() {
        let a = Decimal4::from_str("1.5").unwrap();
        let b = Decimal4::from_str("2.5").unwrap();

        assert_eq!((a + b).to_string(), "4.0000");
        assert_eq!((b - a).to_string(), "1.0000");
    }

    #[test]
    fn test_zero_constant() {
        assert!(Decimal4::ZERO.is_zero());
    }

    #[test]
    fn test_negative_values() {
        let positive = Decimal4::from_str("1.0").unwrap();
        let negative = Decimal4::from_str("-1.0").unwrap();

        assert_eq!((positive - negative).to_string(), "2.0000");
        assert_eq!((negative - positive).to_string(), "-2.0000");
    }
}
