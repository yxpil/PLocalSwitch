//! 候选排序 + 截断：AUTOMODE 多级策略键（免费/非量化/大模型/小模型靠后）+ sticky/balance 策略
use crate::router::CandidateNode;
use crate::state::AppState;
use std::sync::Arc;

/// 模型名/端点是否含量化标记：q4_k_m、q6、int4、gptq、awq、gguf、quant 等
pub fn is_quant(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["q2", "q3", "q4", "q5", "q6", "q8", "int4", "int8", "gptq", "awq", "gguf", "quant"]
        .iter().any(|t| n.contains(t))
}

/// 从模型名解析参数量（单位 B）：`llama-3.3-70b` → 70.0、`qwen2.5-1.5b` → 1.5；解析不出 None
/// 启发式：数字后紧跟 `b` 且不被 `bit`/字母数字组合误读（排除 8bit/fp16 之类）
pub fn parse_params_b(name: &str) -> Option<f64> {
    let n = name.to_ascii_lowercase();
    let b = n.as_bytes();
    let mut best: Option<f64> = None;
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() {
            let start = i;
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') { i += 1; }
            // 数字后紧跟 `b`，且 `b` 后不是 `it`（排除 8bit）也不是字母数字粘连
            if i < b.len() && b[i] == b'b' {
                let ok = match b.get(i + 1) {
                    None => true,
                    Some(b'-') | Some(b'_') | Some(b'.') | Some(b'(') | Some(b')') | Some(b'x') | Some(b'/') => true,
                    Some(c) if c.is_ascii_digit() => false, // 如 "8b2"：不是参数标注
                    Some(_) => false,
                };
                if ok {
                    if let Ok(v) = n[start..i].parse::<f64>() {
                        if v > 0.0 && v < 100_000.0 {
                            best = Some(best.map_or(v, |cur: f64| cur.max(v)));
                        }
                    }
                }
            }
        } else {
            i += 1;
        }
    }
    best
}

/// 候选排序 + 截断（max 个）。
/// 多级策略键（每个开关独立控制，全部关闭则保持原有「质量×权重」行为）：
///   1. prefer_free        免费源优先
///   2. prefer_non_quant   非量化优先
///   3. prefer_large       大模型优先（参数量降序，未标注排后）
///   4. deprioritize_small 小模型（≤32B）靠后
/// 末级排序：balance=质量×权重 动态降序（负载均衡）；sticky=模型名/端点字母序（静态死扛顺序）
pub fn sort_and_trim(state: &Arc<AppState>, cands: &mut Vec<CandidateNode>, max: usize) {
    let (prefer_free, prefer_non_quant, prefer_large, dep_small, sticky) = {
        let am = &state.cfg_swap.load().automode;
        (am.prefer_free, am.prefer_non_quant, am.prefer_large, am.deprioritize_small, am.strategy == "sticky")
    };

    // 预计算每个候选的排序特征（避免比较器里反复解析字符串）
    let feats: Vec<(bool, bool, bool, f64)> = cands.iter().map(|c| {
        let name = format!("{} {}", c.real_model, c.endpoint);
        let params = parse_params_b(&c.real_model);
        (c.free, is_quant(&name), params.map_or(false, |p| p <= 32.0), params.unwrap_or(0.0))
    }).collect();

    let mut idx: Vec<usize> = (0..cands.len()).collect();
    idx.sort_by(|&a, &b| {
        let (fa, qa, sa, pa) = feats[a];
        let (fb, qb, sb, pb) = feats[b];
        if prefer_free { match fb.cmp(&fa) { std::cmp::Ordering::Equal => {}, o => return o } }
        if prefer_non_quant { match qa.cmp(&qb) { std::cmp::Ordering::Equal => {}, o => return o } }
        if prefer_large { match pb.partial_cmp(&pa).unwrap_or(std::cmp::Ordering::Equal) { std::cmp::Ordering::Equal => {}, o => return o } }
        if dep_small { match sa.cmp(&sb) { std::cmp::Ordering::Equal => {}, o => return o } }
        if sticky {
            // 单一顺序死扛：静态字典序，顺序稳定不随质量抖动——第一个能扛就一直扛
            cands[a].real_model.cmp(&cands[b].real_model)
                .then_with(|| cands[a].endpoint.cmp(&cands[b].endpoint))
        } else {
            // 负载均衡：质量×权重 动态降序
            let qa_ = (cands[a].quality as f64).max(1.0) * cands[a].weight;
            let qb_ = (cands[b].quality as f64).max(1.0) * cands[b].weight;
            qb_.partial_cmp(&qa_).unwrap_or(std::cmp::Ordering::Equal)
        }
    });
    *cands = idx.into_iter().map(|i| cands[i].clone()).collect();
    cands.truncate(max);
}
