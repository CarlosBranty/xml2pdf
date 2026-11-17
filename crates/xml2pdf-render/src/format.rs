//! Number and date formatting for the printed document.

use rust_decimal::{Decimal, RoundingStrategy};

/// Formats an amount with two decimals and thousands separators: `1,234.50`.
pub fn money(value: Decimal) -> String {
    let rounded = value.round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
    group_thousands(&format!("{rounded:.2}"))
}

/// Formats an amount preceded by the currency symbol: `S/ 1,234.50`.
pub fn money_with_symbol(symbol: &str, value: Decimal) -> String {
    format!("{symbol} {}", money(value))
}

/// Formats a quantity or unit value keeping its significant decimals, with a
/// minimum of two: `2.00`, `0.125`.
pub fn decimals(value: Decimal) -> String {
    let normalized = value.normalize();
    let scale = normalized.scale().max(2) as usize;
    group_thousands(&format!("{normalized:.scale$}"))
}

/// Formats a quantity without padding decimals: `10`, `2.5`, `1,500`.
pub fn quantity(value: Decimal) -> String {
    group_thousands(&value.normalize().to_string())
}

/// Converts an ISO date (`2026-01-15`) into `15/01/2026`. Other inputs are
/// returned unchanged.
pub fn date(iso: &str) -> String {
    let parts: Vec<&str> = iso.get(..10).unwrap_or(iso).split('-').collect();
    match parts.as_slice() {
        [year, month, day] if year.len() == 4 => format!("{day}/{month}/{year}"),
        _ => iso.to_owned(),
    }
}

fn group_thousands(number: &str) -> String {
    let (sign, digits) = match number.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", number),
    };
    let (integer, fraction) = digits.split_once('.').unwrap_or((digits, ""));

    let mut grouped = String::new();
    for (index, digit) in integer.chars().enumerate() {
        if index > 0 && (integer.len() - index) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    if fraction.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}.{fraction}")
    }
}
