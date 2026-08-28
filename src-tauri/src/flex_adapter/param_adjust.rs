//! 参数改写：max_tokens 截断 / 不支持的 response_format 降级 / seed 过滤
use crate::models::ChatCompletionRequest;
pub fn normalize(_req: &mut ChatCompletionRequest) {
    // TODO
}
