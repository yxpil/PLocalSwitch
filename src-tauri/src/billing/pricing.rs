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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ModelRate;

    fn rate(model: &str, pin: f64, pout: f64) -> ModelRate {
        ModelRate { model: model.into(), upstream_cost_per_m_input: 0.0, upstream_cost_per_m_output: 0.0, client_price_per_m_input: pin, client_price_per_m_output: pout }
    }

    #[test]
    fn exact_match_takes_priority_over_glob() {
        let rates = vec![rate("gpt-4*", 1.0, 2.0), rate("gpt-4o", 3.0, 4.0)];
        // 精确匹配 gpt-4o 命中，而非通配 gpt-4*
        let r = find_rate(&rates, "gpt-4o").unwrap();
        assert_eq!(r.client_price_per_m_input, 3.0);
    }

    #[test]
    fn glob_prefix_matches() {
        let rates = vec![rate("gpt-3.5*", 0.5, 1.5)];
        let r = find_rate(&rates, "gpt-3.5-turbo").unwrap();
        assert_eq!(r.client_price_per_m_input, 0.5);
        // 不匹配前缀
        assert!(find_rate(&rates, "gpt-4").is_none());
    }

    #[test]
    fn calc_per_million_math() {
        let r = rate("m", 2.0, 4.0); // 每 1M token 输入 2 元 / 输出 4 元
        let c = calc(Some(&r), 1_000_000, 500_000);
        assert!((c.input_cny - 2.0).abs() < 1e-9);
        assert!((c.output_cny - 2.0).abs() < 1e-9);
        assert!((c.total_cny - 4.0).abs() < 1e-9);
    }

    #[test]
    fn calc_unknown_rate_is_zero() {
        let c = calc(None, 1000, 1000);
        assert_eq!(c.total_cny, 0.0);
    }
}
