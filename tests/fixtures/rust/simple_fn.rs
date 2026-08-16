//! Standalone helper functions with lifetimes and Result/Option returns.

use std::error::Error;
use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub enum MathError {
    DivisionByZero,
    NegativePrice,
    InvalidPercentage,
}

impl fmt::Display for MathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MathError::DivisionByZero => write!(f, "Cannot divide by zero"),
            MathError::NegativePrice => write!(f, "Price cannot be negative"),
            MathError::InvalidPercentage => write!(f, "Percentage must be between 0 and 100"),
        }
    }
}

impl Error for MathError {}

/// Pure mathematical addition of two numbers.
pub fn add_numbers(a: i64, b: i64) -> i64 {
    a + b
}

/// Formats first and last name with an optional lifetime-bound prefix.
pub fn format_user_name<'a>(first: &'a str, last: &'a str, prefix: Option<&'a str>) -> String {
    let full = format!("{} {}", first.trim(), last.trim()).trim().to_string();
    match prefix {
        Some(p) if !p.trim().is_empty() => format!("{} {}", p.trim(), full),
        _ => full,
    }
}

/// Divides two numbers returning a Result.
pub fn divide_safe(numerator: f64, denominator: f64) -> Result<f64, MathError> {
    if denominator.abs() < f64::EPSILON {
        return Err(MathError::DivisionByZero);
    }
    Ok(numerator / denominator)
}

/// Computes discounted price given base price and percentage.
pub fn calculate_discount(price: f64, percentage: f64) -> Result<f64, MathError> {
    if price < 0.0 {
        return Err(MathError::NegativePrice);
    }
    if !(0.0..=100.0).contains(&percentage) {
        return Err(MathError::InvalidPercentage);
    }
    let discount = (price * percentage) / 100.0;
    Ok((price - discount).round())
}

/// Extracts prefix and suffix slices from a string with lifetime bounds.
pub fn extract_prefix_and_suffix<'a>(text: &'a str, split_char: char) -> Option<(&'a str, &'a str)> {
    let pos = text.find(split_char)?;
    Some((&text[..pos], &text[pos + 1..]))
}
