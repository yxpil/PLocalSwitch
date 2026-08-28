//! 环形定长样本缓冲（每个 node_id 固定最大 samples=2000，防内存爆炸）
pub struct RingBuf<T> { buf: Vec<T>, cap: usize, head: usize }
impl<T> RingBuf<T> {
    pub fn new(cap: usize) -> Self { Self { buf: Vec::with_capacity(cap), cap: cap.max(1), head: 0 } }
    pub fn push(&mut self, v: T) {
        if self.buf.len() < self.cap { self.buf.push(v); } else { self.buf[self.head] = v; self.head = (self.head + 1) % self.cap; }
    }
    pub fn iter(&self) -> impl Iterator<Item = &T> { let (a, b) = self.buf.split_at(self.head); b.iter().chain(a.iter()) }
    pub fn len(&self) -> usize { self.buf.len() }
    pub fn is_empty(&self) -> bool { self.buf.is_empty() }
}
