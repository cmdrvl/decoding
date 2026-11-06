//! Exact JSON decimals. No binary float conversion or implicit rounding.
use bigdecimal::BigDecimal;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::str::FromStr;

/// Resource limits are refusal boundaries, never precision truncation.
pub const MAX_DECIMAL_DIGITS: u64 = 1024;
pub const MAX_DECIMAL_SCALE: u64 = 1024;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ExactDecimal(pub BigDecimal);

pub fn within_bounds(value: &BigDecimal) -> bool {
    let value = value.normalized();
    value.digits() <= MAX_DECIMAL_DIGITS
        && value.fractional_digit_count().unsigned_abs() <= MAX_DECIMAL_SCALE
}

impl FromStr for ExactDecimal {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        if text.len() > 4096 {
            return Err("decimal token exceeds 4096 bytes".into());
        }
        let value = BigDecimal::from_str(text).map_err(|error| error.to_string())?;
        if !within_bounds(&value) {
            return Err(
                "decimal exceeds 1024 significant digits or normalized absolute scale".into(),
            );
        }
        Ok(Self(value))
    }
}

impl From<u32> for ExactDecimal {
    fn from(value: u32) -> Self {
        Self(BigDecimal::from(value))
    }
}

impl<'de> Deserialize<'de> for ExactDecimal {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Number rejects strings and preserves the JSON token with arbitrary_precision.
        let number = serde_json::Number::deserialize(deserializer)?;
        number.to_string().parse().map_err(serde::de::Error::custom)
    }
}

impl Serialize for ExactDecimal {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut text = self.0.normalized().to_string();
        // Keep the established numeric-scalar wire spelling for whole amounts.
        if !text.chars().any(|ch| matches!(ch, '.' | 'e' | 'E')) {
            text.push_str(".0");
        }
        serde_json::Number::from_str(&text)
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}
