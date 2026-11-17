//! Labels for the SUNAT catalogs printed on a document.

/// Catalog 01: document type title.
pub fn document_title(type_code: &str) -> &'static str {
    match type_code {
        "01" => "FACTURA ELECTRÓNICA",
        "03" => "BOLETA DE VENTA ELECTRÓNICA",
        "07" => "NOTA DE CRÉDITO ELECTRÓNICA",
        "08" => "NOTA DE DÉBITO ELECTRÓNICA",
        "09" => "GUÍA DE REMISIÓN REMITENTE",
        "31" => "GUÍA DE REMISIÓN TRANSPORTISTA",
        _ => "COMPROBANTE ELECTRÓNICO",
    }
}

/// Catalog 01: short document name, used when referring to another document.
pub fn document_name(type_code: &str) -> &'static str {
    match type_code {
        "01" => "FACTURA",
        "03" => "BOLETA DE VENTA",
        "07" => "NOTA DE CRÉDITO",
        "08" => "NOTA DE DÉBITO",
        "09" | "31" => "GUÍA DE REMISIÓN",
        _ => "DOCUMENTO",
    }
}

/// Catalog 06: identity document type.
pub fn identity_label(doc_type: &str) -> &'static str {
    match doc_type {
        "0" => "DOC. TRIB. NO DOM.",
        "1" => "DNI",
        "4" => "CARNET DE EXTRANJERÍA",
        "6" => "RUC",
        "7" => "PASAPORTE",
        "A" => "CÉD. DIPLOMÁTICA",
        _ => "DOCUMENTO",
    }
}

/// ISO 4217 currency name in plural, as used in the amount in words.
pub fn currency_name(currency: &str) -> &'static str {
    match currency {
        "PEN" => "SOLES",
        "USD" => "DÓLARES AMERICANOS",
        "EUR" => "EUROS",
        _ => "UNIDADES MONETARIAS",
    }
}

/// Currency symbol printed next to amounts. Unknown currencies print their code.
pub fn currency_symbol(currency: &str) -> &str {
    match currency {
        "PEN" => "S/",
        "USD" => "$",
        "EUR" => "€",
        other => other,
    }
}

/// Catalog 03: short unit of measure. Unknown codes print as they are.
pub fn unit_label(unit_code: &str) -> &str {
    match unit_code {
        "NIU" => "UND",
        "ZZ" => "SERV",
        "KGM" => "KG",
        "GRM" => "G",
        "LTR" => "L",
        "MTR" => "M",
        "MTK" => "M2",
        "MTQ" => "M3",
        "BX" => "CAJA",
        "PK" => "PAQ",
        "DZN" => "DOC",
        "HUR" => "HORA",
        "DAY" => "DÍA",
        "MON" => "MES",
        "GLL" => "GAL",
        "TNE" => "TN",
        "BG" => "BOLSA",
        "BO" => "BOT",
        "SET" => "JGO",
        other => other,
    }
}

/// Catalog 09: credit note reason.
pub fn credit_reason(code: &str) -> &'static str {
    match code {
        "01" => "ANULACIÓN DE LA OPERACIÓN",
        "02" => "ANULACIÓN POR ERROR EN EL RUC",
        "03" => "CORRECCIÓN POR ERROR EN LA DESCRIPCIÓN",
        "04" => "DESCUENTO GLOBAL",
        "05" => "DESCUENTO POR ÍTEM",
        "06" => "DEVOLUCIÓN TOTAL",
        "07" => "DEVOLUCIÓN POR ÍTEM",
        "08" => "BONIFICACIÓN",
        "09" => "DISMINUCIÓN EN EL VALOR",
        "10" => "OTROS CONCEPTOS",
        "11" => "AJUSTES DE OPERACIONES DE EXPORTACIÓN",
        "12" => "AJUSTES AFECTOS AL IVAP",
        "13" => "AJUSTES - MONTOS Y/O FECHAS DE PAGO",
        _ => "",
    }
}

/// Catalog 10: debit note reason.
pub fn debit_reason(code: &str) -> &'static str {
    match code {
        "01" => "INTERESES POR MORA",
        "02" => "AUMENTO EN EL VALOR",
        "03" => "PENALIDADES / OTROS CONCEPTOS",
        "10" => "AJUSTES DE OPERACIONES DE EXPORTACIÓN",
        "11" => "AJUSTES AFECTOS AL IVAP",
        _ => "",
    }
}
