use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalcRequest {
    pub expression: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalcResponse {
    pub result: f64,
    pub formatted: String,
    pub expression: String,
}

pub fn calculate(request: &CalcRequest) -> Result<CalcResponse> {
    let expr = request.expression.trim();

    let cleaned = expr
        .replace("×", "*")
        .replace("÷", "/")
        .replace("^", ".powf")
        .replace("%", "/100.0*");

    let result: f64 = meval::eval_str(&cleaned)?;

    let formatted = if result.fract() == 0.0 && result.abs() < 1e15 {
        format!("{}", result as i64)
    } else {
        format!("{:.10}", result)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string()
    };

    Ok(CalcResponse {
        result,
        formatted,
        expression: request.expression.clone(),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitConvertRequest {
    pub value: f64,
    pub from_unit: String,
    pub to_unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitConvertResponse {
    pub result: f64,
    pub formatted: String,
}

pub fn convert_unit(request: &UnitConvertRequest) -> Result<UnitConvertResponse> {
    let from = request.from_unit.to_lowercase();
    let to = request.to_unit.to_lowercase();
    let value = request.value;

    let result = match (from.as_str(), to.as_str()) {
        ("km", "mi") => value * 0.621371,
        ("mi", "km") => value * 1.60934,
        ("m", "ft") => value * 3.28084,
        ("ft", "m") => value * 0.3048,
        ("kg", "lb") => value * 2.20462,
        ("lb", "kg") => value * 0.453592,
        ("c", "f") => value * 9.0 / 5.0 + 32.0,
        ("f", "c") => (value - 32.0) * 5.0 / 9.0,
        ("l", "gal") => value * 0.264172,
        ("gal", "l") => value * 3.78541,
        ("cm", "in") => value * 0.393701,
        ("in", "cm") => value * 2.54,
        ("g", "oz") => value * 0.035274,
        ("oz", "g") => value * 28.3495,
        ("usd", "krw") => value * 1350.0,
        ("krw", "usd") => value / 1350.0,
        _ => anyhow::bail!("Unsupported conversion: {} to {}", from, to),
    };

    Ok(UnitConvertResponse {
        result,
        formatted: format!("{:.4}", result)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_calc() {
        let req = CalcRequest {
            expression: "2 + 2".to_string(),
        };
        let res = calculate(&req).unwrap();
        assert_eq!(res.result, 4.0);
    }

    #[test]
    fn test_complex_calc() {
        let req = CalcRequest {
            expression: "100 * 1.1".to_string(),
        };
        let res = calculate(&req).unwrap();
        assert!((res.result - 110.0).abs() < 1e-10);
    }
}
