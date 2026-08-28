//! SSE 输出归一化：OpenAI 标准 data: {...} / data: [DONE] 格式
use crate::models::SseChunk;
pub fn serialize_chunk(c: &SseChunk) -> Vec<u8> {
    let mut out = Vec::with_capacity(256);
    // NOTE: 字段 c.event / c.data 不存在于结构体 SseChunk。
    // 我们直接把整个 chunk 序列化成 JSON 作为 data: 行；
    // 如果整个 chunk 各字段全空 → 视为 [DONE] 终止帧。
    let is_done = c.id.is_none()
        && c.object.is_none()
        && c.created.is_none()
        && c.model.is_none()
        && c.choices.is_empty();
    if is_done {
        out.extend_from_slice(b"data: [DONE]\n\n");
    } else {
        out.extend_from_slice(b"data: ");
        let json = serde_json::to_string(c).unwrap_or_default();
        out.extend_from_slice(json.as_bytes());
        out.extend_from_slice(b"\n\n");
    }
    out
}
pub const DONE_BYTES: &[u8] = b"data: [DONE]\n\n";
