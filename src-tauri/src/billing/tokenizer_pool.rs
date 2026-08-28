//! 分词器池：每个模型 glob 绑定 tiktoken 或 外部 bpe/vocab/merges 文件
use crate::config::TokenizerBinding;
use dashmap::DashMap;
use std::sync::Arc;
pub struct TokenizerPool { pub items: DashMap<String, Arc<TokenizerInstance>> }
pub enum TokenizerInstance { Tiktoken(()), External(()) }  // placeholder
impl TokenizerPool {
    pub fn new(_bindings: &[TokenizerBinding]) -> Self { Self { items: DashMap::new() } }
    pub fn count_tokens(&self, _model: &str, _text: &str) -> Option<u32> { None } // TODO
}
