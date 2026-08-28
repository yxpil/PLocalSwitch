//! 按模型匹配采购价/售价
use crate::config::ModelRate;
pub fn find_rate<'a>(rates: &'a [ModelRate], model: &str) -> Option<&'a ModelRate> {
    rates.iter().find(|r| r.model == model).or_else(|| rates.iter().find(|r| glob(&r.model, model)))
}
fn glob(pat: &str, s: &str) -> bool { if let Some(p) = pat.strip_suffix('*') { s.starts_with(p) } else { false } }
pub struct RateCost { pub input_cny: f64, pub output_cny: f64, pub total_cny: f64 }
pub fn calc(rate: Option<&ModelRate>, inp: u32, out: u32) -> RateCost {
    match rate {
        Some(r) => {
            let i = (inp as f64 / 1_000_000.0) * r.client_price_per_m_input;
            let o = (out as f64 / 1_000_000.0) * r.client_price_per_m_output;
            RateCost { input_cny: i, output_cny: o, total_cny: i + o }
        }
        None => RateCost { input_cny: 0.0, output_cny: 0.0, total_cny: 0.0 },
    }
}
