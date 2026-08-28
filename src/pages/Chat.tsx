import React, { useEffect, useRef, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import Icon from '@icons/index';
import { invoke } from '@commands/index';

interface Msg { role: 'user' | 'assistant'; content: string; }

const DEFAULT_KEY = '';

/** 无 typography 插件时的轻量 Markdown 元件样式 */
const MarkdownView: React.FC<{ content: string }> = ({ content }) => (
  <ReactMarkdown remarkPlugins={[remarkGfm]} components={{
    h1: ({ node: _n, ...p }) => <h1 className="text-base font-bold mt-2 mb-1" {...p} />,
    h2: ({ node: _n, ...p }) => <h2 className="text-[15px] font-bold mt-2 mb-1" {...p} />,
    h3: ({ node: _n, ...p }) => <h3 className="text-sm font-bold mt-2 mb-1" {...p} />,
    p: ({ node: _n, ...p }) => <p className="my-1" {...p} />,
    ul: ({ node: _n, ...p }) => <ul className="list-disc pl-5 my-1" {...p} />,
    ol: ({ node: _n, ...p }) => <ol className="list-decimal pl-5 my-1" {...p} />,
    li: ({ node: _n, ...p }) => <li className="my-0.5" {...p} />,
    a: ({ node: _n, ...p }) => <a className="underline" target="_blank" rel="noreferrer" {...p} />,
    code: ({ node: _n, ...p }) => <code className="px-1 py-0.5 rounded bg-neutral-200/70 dark:bg-neutral-800 text-[0.85em]" {...p} />,
    pre: ({ node: _n, ...p }) => <pre className="block p-3 my-2 rounded-lg bg-neutral-200/60 dark:bg-neutral-800 overflow-x-auto text-xs" {...p} />,
    blockquote: ({ node: _n, ...p }) => <blockquote className="border-l-2 pl-3 my-1 text-neutral-500" {...p} />,
    hr: ({ node: _n }) => <hr className="my-2 border-neutral-200 dark:border-neutral-800" />,
    table: ({ node: _n, ...p }) => <table className="border-collapse my-2 text-xs w-full" {...p} />,
    th: ({ node: _n, ...p }) => <th className="border px-2 py-1 bg-neutral-100 dark:bg-neutral-800" {...p} />,
    td: ({ node: _n, ...p }) => <td className="border px-2 py-1" {...p} />,
    img: ({ node: _n, ...p }) => <img className="max-w-full rounded my-1" {...p} />,
  }}>{content}</ReactMarkdown>
);

const Chat: React.FC = () => {
  const [models, setModels] = useState<string[]>([]);
  const [model, setModel] = useState('');
  const [key, setKey] = useState(DEFAULT_KEY);
  const [messages, setMessages] = useState<Msg[]>([]);
  const [input, setInput] = useState('');
  const [loading, setLoading] = useState(false);
  const bottomRef = useRef<HTMLDivElement>(null);

  // 先立即用网关配置别名填充（保证下拉不为空、秒开），再后台拉上游真实模型升级。
  // 注意：@commands/index 的 invoke 会解包 ApiResponse，因此 load_config 直接返回配置对象、
  //       而 list_upstream_models 直接返回 [{id, group}, ...] 数组。
  const loadModels = async () => {
    // 1) 别名兜底：来自网关配置（invoke 已解包，cfg 即 AppConfig）
    try {
      const cfg: any = await invoke('load_config');
      const aliases: string[] = (cfg?.model_aliases ?? []).map((a: any) => a.alias).filter(Boolean);
      if (aliases.length) {
        setModels(aliases);
        setModel((cur) => cur || aliases[0] || '');
      }
    } catch { /* 忽略 */ }
    // 2) 上游真实模型列表（invoke 解包后 resp 即数组 [{id,group}]）；成功则替换
    try {
      const resp: any = await invoke('list_upstream_models');
      const arr: any[] = Array.isArray(resp) ? resp : ((resp?.data as any[]) ?? []);
      const ids: string[] = arr.map((m: any) => m.id).filter(Boolean);
      if (ids.length) {
        setModels(ids);
        setModel((cur) => (cur && ids.includes(cur) ? cur : ids[0]));
      }
    } catch { /* 忽略上游拉取失败，保留别名 */ }
  };

  useEffect(() => { loadModels(); /* eslint-disable-next-line */ }, []);
  useEffect(() => { bottomRef.current?.scrollIntoView({ behavior: 'smooth' }); }, [messages]);

  const send = async () => {
    const text = input.trim();
    if (!text || !model || loading) return;
    const history: Msg[] = [...messages, { role: 'user', content: text }];
    setMessages([...history, { role: 'assistant', content: '' }]);
    setInput('');
    setLoading(true);
    try {
      // invoke 会解包 ApiResponse：成功 → resp 即 {content, raw}；失败 → invoke 直接 throw（含后端 error 信息）
      const resp: any = await invoke('gateway_chat', {
        model,
        messages: history.map(m => ({ role: m.role, content: m.content })),
        key,
      });
      const content = resp?.content ?? '';
      setMessages((prev) => {
        const next = [...prev];
        next[next.length - 1] = { role: 'assistant', content };
        return next;
      });
    } catch (e: any) {
      setMessages((prev) => {
        const next = [...prev];
        next[next.length - 1] = { role: 'assistant', content: `⚠️ ${e?.message ?? String(e)}` };
        return next;
      });
    } finally { setLoading(false); }
  };

  const newChat = () => { setMessages([]); setInput(''); };

  return (
    <div className="flex flex-col h-[calc(100vh-8.5rem)]">
      {/* 顶部：模型选择 + Key + 新对话 */}
      <div className="flex flex-wrap items-center gap-2 px-4 py-3 border-b border-neutral-200/70 dark:border-neutral-800/70">
        <Icon name="activity" size={16} />
        <select
          value={model}
          onChange={(e) => setModel(e.target.value)}
          className="rounded-pill bg-neutral-100 dark:bg-neutral-900 px-3 py-1.5 text-sm outline-none focus:ring-2 focus:ring-neutral-400/40"
        >
          {models.length === 0 && <option value="">（未加载模型）</option>}
          {models.map((m) => <option key={m} value={m}>{m}</option>)}
        </select>
        <input
          value={key}
          onChange={(e) => setKey(e.target.value)}
          placeholder="Client Key"
          className="flex-1 min-w-[10rem] rounded-pill bg-neutral-100 dark:bg-neutral-900 px-3 py-1.5 text-xs font-mono outline-none focus:ring-2 focus:ring-neutral-400/40"
        />
        <button onClick={newChat}
          className="pill-btn pill-variant-ghost !py-1.5 !px-3 text-xs">新对话</button>
      </div>

      {/* 消息区 */}
      <div className="flex-1 overflow-y-auto px-4 py-4 space-y-4">
        {messages.length === 0 && (
          <div className="h-full flex flex-col items-center justify-center text-neutral-400">
            <Icon name="zap" size={30} />
            <p className="mt-3 text-sm">选择一个模型，输入内容开始对话</p>
            <p className="mt-1 text-xs">支持 Markdown · 流式输出</p>
          </div>
        )}
        {messages.map((m, i) => (
          <div key={i} className={`flex ${m.role === 'user' ? 'justify-end' : 'justify-start'}`}>
            <div className={
              m.role === 'user'
                ? 'max-w-[80%] rounded-[1.25rem] px-4 py-2.5 bg-neutral-900 text-white dark:bg-white dark:text-black text-sm whitespace-pre-wrap whitespace-normal'
                : 'max-w-[85%] rounded-[1.25rem] px-4 py-2.5 bg-neutral-100 dark:bg-neutral-900 text-sm'
            }>
              {m.role === 'user' ? (
                m.content
              ) : (
                <MarkdownView content={m.content} />
              )}
            </div>
          </div>
        ))}
        <div ref={bottomRef} />
      </div>

      {/* 输入区 */}
      <div className="px-4 py-3 border-t border-neutral-200/70 dark:border-neutral-800/70">
        <div className="flex items-end gap-2">
          <textarea
            value={input}
            onChange={(e) => setInput(e.target.value)}
            onKeyDown={(e) => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); send(); } }}
            rows={1}
            placeholder="输入消息，Enter 发送 / Shift+Enter 换行"
            className="flex-1 resize-none rounded-[1.25rem] bg-neutral-100 dark:bg-neutral-900 px-4 py-2.5 text-sm outline-none focus:ring-2 focus:ring-neutral-400/40 max-h-40"
          />
          <button onClick={send} disabled={loading || !input.trim() || !model}
            className="pill-btn pill-variant-primary !py-2.5 !px-4">
            {loading ? <span className="h-4 w-4 rounded-full border-2 border-current/30 border-t-current animate-spin" /> : <Icon name="chevron-right" size={16} />}
          </button>
        </div>
      </div>
    </div>
  );
};

export default Chat;
