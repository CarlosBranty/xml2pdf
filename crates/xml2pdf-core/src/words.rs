//! Amounts written in Spanish words, as printed on SUNAT documents:
//! `CIENTO DIECIOCHO CON 00/100 SOLES`.

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::{Decimal, RoundingStrategy};

use crate::catalog;

const UNITS: [&str; 30] = [
    "CERO",
    "UNO",
    "DOS",
    "TRES",
    "CUATRO",
    "CINCO",
    "SEIS",
    "SIETE",
    "OCHO",
    "NUEVE",
    "DIEZ",
    "ONCE",
    "DOCE",
    "TRECE",
    "CATORCE",
    "QUINCE",
    "DIECISEIS",
    "DIECISIETE",
    "DIECIOCHO",
    "DIECINUEVE",
    "VEINTE",
    "VEINTIUNO",
    "VEINTIDOS",
    "VEINTITRES",
    "VEINTICUATRO",
    "VEINTICINCO",
    "VEINTISEIS",
    "VEINTISIETE",
    "VEINTIOCHO",
    "VEINTINUEVE",
];
const TENS: [&str; 10] = [
    "",
    "",
    "",
    "TREINTA",
    "CUARENTA",
    "CINCUENTA",
    "SESENTA",
    "SETENTA",
    "OCHENTA",
    "NOVENTA",
];
const HUNDREDS: [&str; 10] = [
    "",
    "CIENTO",
    "DOSCIENTOS",
    "TRESCIENTOS",
    "CUATROCIENTOS",
    "QUINIENTOS",
    "SEISCIENTOS",
    "SETECIENTOS",
    "OCHOCIENTOS",
    "NOVECIENTOS",
];

/// Writes an amount as `<WORDS> CON <CENTS>/100 <CURRENCY NAME>`.
pub fn amount_in_words(amount: Decimal, currency: &str) -> String {
    let amount = amount
        .abs()
        .round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero);
    let integer = amount.trunc().to_u64().unwrap_or_default();
    let cents = (amount.fract() * Decimal::ONE_HUNDRED)
        .to_u64()
        .unwrap_or_default();
    format!(
        "{} CON {cents:02}/100 {}",
        integer_in_words(integer),
        catalog::currency_name(currency)
    )
}

/// Writes a non-negative integer in Spanish words.
pub fn integer_in_words(number: u64) -> String {
    if number == 0 {
        return UNITS[0].to_owned();
    }

    let millions = number / 1_000_000;
    let thousands = number / 1_000 % 1_000;
    let rest = number % 1_000;
    let mut parts = Vec::new();

    match millions {
        0 => {}
        1 => parts.push("UN MILLON".to_owned()),
        _ if millions < 1_000 => parts.push(format!("{} MILLONES", below_thousand(millions, true))),
        _ => parts.push(format!(
            "{} MILLONES",
            shortened(&integer_in_words(millions))
        )),
    }
    match thousands {
        0 => {}
        1 => parts.push("MIL".to_owned()),
        _ => parts.push(format!("{} MIL", below_thousand(thousands, true))),
    }
    if rest > 0 {
        parts.push(below_thousand(rest, false));
    }
    parts.join(" ")
}

/// Words for 1..=999. `before_noun` shortens a trailing `UNO` to `UN`
/// (`VEINTIUN MIL`, `UN MILLON`).
fn below_thousand(number: u64, before_noun: bool) -> String {
    let hundreds = (number / 100) as usize;
    let rest = (number % 100) as usize;
    if number == 100 {
        return "CIEN".to_owned();
    }

    let mut parts = Vec::new();
    if hundreds > 0 {
        parts.push(HUNDREDS[hundreds].to_owned());
    }
    if rest > 0 {
        let words = if rest < 30 {
            UNITS[rest].to_owned()
        } else if rest.is_multiple_of(10) {
            TENS[rest / 10].to_owned()
        } else {
            format!("{} Y {}", TENS[rest / 10], UNITS[rest % 10])
        };
        parts.push(if before_noun {
            shortened(&words)
        } else {
            words
        });
    }
    parts.join(" ")
}

fn shortened(words: &str) -> String {
    match words.strip_suffix("UNO") {
        Some(prefix) => format!("{prefix}UN"),
        None => words.to_owned(),
    }
}
