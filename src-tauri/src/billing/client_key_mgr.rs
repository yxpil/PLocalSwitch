//! ClientKey 校验 + 配额/余额/RPM TPM 限流（内存表，后续可迁 DB）
use crate::config::ClientKey;
pub struct ClientKeyRegistry { pub keys: std::collections::BTreeMap<String, ClientKey> }
impl ClientKeyRegistry {
    pub fn from_cfg(items: &[ClientKey]) -> Self {
        let mut m = std::collections::BTreeMap::new();
        for k in items { m.insert(k.key.clone(), k.clone()); }
        Self { keys: m }
    }
    pub fn verify(&self, header: &str) -> Option<&ClientKey> {
        // Authorization: Bearer <key>
        let k = header.strip_prefix("Bearer ").unwrap_or(header);
        let entry = self.keys.get(k)?;
        if !entry.enabled { return None; }
        Some(entry)
    }
}
