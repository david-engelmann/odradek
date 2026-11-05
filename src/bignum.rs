//! Exact whole numbers for the snapshot clock.
//!
//! Snapshot times, signal intervals, and last signals are
//! whole seconds read through one helper whose conversion
//! admits up to 4300 digit characters and whose values are
//! exact at any admitted length (ZH-31). The ages, thresholds,
//! elapsed spans, and windows the tool computes from them are
//! exact too, so the crate carries a whole number as its
//! canonical decimal text and implements the four operations
//! the tool needs over that text. Every value the tool holds
//! is non-negative, so the type carries no sign.

use std::cmp::Ordering;
use std::fmt;

/// A non-negative whole number, held as canonical decimal
/// text with no leading zeros.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BigNum(String);

impl BigNum {
    /// The number a run of decimal digit values names, in the
    /// order the run writes them, leading zeros dropped.
    pub fn from_digit_values(values: &[u32]) -> BigNum {
        let mut text = String::new();
        for value in values {
            text.push(char::from(b'0' + (*value as u8)));
        }
        BigNum::canonical(&text)
    }

    /// The number a small machine value names.
    pub fn from_u64(value: u64) -> BigNum {
        BigNum(value.to_string())
    }

    fn canonical(text: &str) -> BigNum {
        let trimmed = text.trim_start_matches('0');
        BigNum(if trimmed.is_empty() {
            "0".to_string()
        } else {
            trimmed.to_string()
        })
    }

    /// The canonical decimal text of the number.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The difference of two numbers, the first not smaller
    /// than the second. Every subtraction the tool performs
    /// is guarded by a recorded fact that orders its operands.
    pub fn sub(&self, other: &BigNum) -> BigNum {
        let left: Vec<u8> = self.0.bytes().rev().collect();
        let right: Vec<u8> = other.0.bytes().rev().collect();
        let mut out: Vec<u8> = Vec::new();
        let mut borrow: i16 = 0;
        for index in 0..left.len() {
            let mut digit = (left[index] - b'0') as i16 - borrow;
            if index < right.len() {
                digit -= (right[index] - b'0') as i16;
            }
            if digit < 0 {
                digit += 10;
                borrow = 1;
            } else {
                borrow = 0;
            }
            out.push(b'0' + digit as u8);
        }
        out.reverse();
        let text: String = out.iter().map(|byte| *byte as char).collect();
        BigNum::canonical(&text)
    }

    /// The product of the number and one small factor, the
    /// proof multiple and the observation multiple.
    pub fn mul_small(&self, factor: u32) -> BigNum {
        if factor == 0 || self.0 == "0" {
            return BigNum("0".to_string());
        }
        let mut carry: u32 = 0;
        let mut out: Vec<u8> = Vec::new();
        for byte in self.0.bytes().rev() {
            let product = (byte - b'0') as u32 * factor + carry;
            out.push(b'0' + (product % 10) as u8);
            carry = product / 10;
        }
        while carry > 0 {
            out.push(b'0' + (carry % 10) as u8);
            carry /= 10;
        }
        out.reverse();
        let text: String = out.iter().map(|byte| *byte as char).collect();
        BigNum(text)
    }
}

impl Ord for BigNum {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0
            .len()
            .cmp(&other.0.len())
            .then_with(|| self.0.cmp(&other.0))
    }
}

impl PartialOrd for BigNum {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for BigNum {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
