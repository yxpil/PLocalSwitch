/**
 * ============================================================
 *  GatewaySection = 网关配置（/settings#gateway）真实数据
 *  - 概览卡：监听地址 / 上游节点数 / Client Key 数 / 模型别名数
 *  - Client Keys 列表（从后端 gateway.yaml 读取）
 *  - 上游节点组概览（节点数 / 协议 / enabled）
 *  全部来自 load_config（桌面端 Tauri IPC → 真实 gateway.yaml），无 demo。
 * ============================================================
 */
import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import PillCard from '@components/ui/PillCard';
import PillButton from '@components/ui/PillButton';
import PillBadge from '@components/ui/PillBadge';
import Icon from '@icons/index';
import PillSwitch from '@components/ui/PillSwitch';
import { invoke } from '@commands/index';
import { accessHost } from '../../utils/net';

/* utils */
function fmtInt(n: number) { return n.toLocaleString('en-US'); }
function maskKey(k: string): string {
  if (!k) return '';
  if (k.length <= 8) return k.slice(0, 2) + '****' + k.slice(-2);
  return k.slice(0, 4) + '\u00b7\u00b7\u00b7' + k.slice(-4);
}

interface NodeView {
  id: string;
  endpoint: string;
  api_key: string;
  protocol: string;
  enabled: boolean;
}
interface GroupView {
  id: string;
  enabled: boolean;
  nodeCount: number;
  protocols: string[];
  nodes: NodeView[];
}

const GatewaySection: React.FC = () => {
  const { t } = useTranslation();
  const [copyToast, setCopyToast] = useState<string | null>(null);
  const [listen, setListen] = useState('');
  const [keys, setKeys] = useState<any[]>([]);
  const [groups, setGroups] = useState<GroupView[]>([]);
  const [aliases, setAliases] = useState<any[]>([]);
  const [aliasCount, setAliasCount] = useState(0);
  const [automode, setAutomode] = useState(true);
  const [preferFree, setPreferFree] = useState(false);
  const [preferNonQuant, setPreferNonQuant] = useState(false);
  const [preferLarge, setPreferLarge] = useState(false);
  const [depSmall, setDepSmall] = useState(false);
  const [strategy, setStrategy] = useState('balance');

  const load = async () => {
    try {
      const cfg: any = await invoke('load_config');
      setAutomode(cfg?.automode?.enabled !== false);
      setPreferFree(cfg?.automode?.prefer_free === true);
      setPreferNonQuant(cfg?.automode?.prefer_non_quant === true);
      setPreferLarge(cfg?.automode?.prefer_large === true);
      setDepSmall(cfg?.automode?.deprioritize_small === true);
      setStrategy(cfg?.automode?.strategy === 'sticky' ? 'sticky' : 'balance');
      setListen(accessHost(cfg?.http?.listen));
      setAliasCount((cfg?.model_aliases ?? []).length);
      setAliases((cfg?.model_aliases ?? []).map((a: any) => ({
        alias: a.alias ?? '',
        real_model: a.real_model ?? '',
        group: a.group ?? 'default',
        enabled: a.enabled !== false,
      })));
      setKeys((cfg?.billing?.client_keys ?? []).map((k: any) => ({
        name: k.name || 'key',
        group: k.group || 'default',
        masked: maskKey(k.key || ''),
        rpm: k.rpm ?? 0,
        tpm: k.tpm ?? 0,
        enabled: k.enabled !== false,
      })));
      setGroups((cfg?.node_groups ?? []).map((g: any) => ({
        id: g.id,
        enabled: g.enabled !== false,
        nodeCount: (g.nodes ?? []).length,
        protocols: Array.from(new Set<string>((g.nodes ?? []).flatMap((n: any) => [n.protocol, ...(n.protocol_hints ?? [])]))),
        nodes: (g.nodes ?? []).map((n: any) => ({
          id: n.id ?? '',
          endpoint: n.endpoint ?? '',
          api_key: (n.api_keys ?? [])[0] ?? '',
          protocol: (n.protocol_hints ?? [])[0] ?? '',
          enabled: n.enabled !== false,
        })),
      })));
    } catch { /* 无后端时忽略 */ }
  };

  useEffect(() => { load(); }, []);

  // 分组下拉选项：真实上游组 + 「自动匹配」（让路由自动落到有节点的组）
  const groupOptions = ['auto', ...groups.map((g) => g.id)];

  const onCopy = async (text: string, label = '已复制') => {
    try { await navigator.clipboard.writeText(text); } catch { /* noop */ }
    setCopyToast(label);
    setTimeout(() => setCopyToast(null), 1500);
  };

  const toggleKey = async (i: number) => {
    const next = keys.map((k, idx) => idx === i ? { ...k, enabled: !k.enabled } : k);
    setKeys(next);
    try {
      const cfg: any = await invoke('load_config');
      cfg.billing.client_keys = next.map((k, idx) => ({
        ...(cfg.billing.client_keys[idx] ?? {}),
        enabled: k.enabled,
      }));
      await invoke('save_config', { cfg });
    } catch { /* 静默 */ }
  };

  /* AUTOMODE 开关：开启后 model="AUTOMODE" 自动在全部可用模型间尝试降级 */
  const toggleAutomode = async (v: boolean) => {
    setAutomode(v);
    try {
      const cfg: any = await invoke('load_config');
      cfg.automode = { ...(cfg.automode ?? {}), enabled: v };
      await invoke('save_config', { cfg });
    } catch { /* 静默 */ }
  };

  /* AUTOMODE 排序策略字段统一保存：prefer_free / prefer_non_quant / prefer_large / deprioritize_small / strategy */
  const setAm = async (field: string, v: any) => {
    try {
      const cfg: any = await invoke('load_config');
      cfg.automode = { ...(cfg.automode ?? {}), [field]: v };
      await invoke('save_config', { cfg });
    } catch { /* 静默 */ }
  };
  const togglePreferFree = async (v: boolean) => { setPreferFree(v); await setAm('prefer_free', v); };
  const togglePreferNonQuant = async (v: boolean) => { setPreferNonQuant(v); await setAm('prefer_non_quant', v); };
  const togglePreferLarge = async (v: boolean) => { setPreferLarge(v); await setAm('prefer_large', v); };
  const toggleDepSmall = async (v: boolean) => { setDepSmall(v); await setAm('deprioritize_small', v); };
  const changeStrategy = async (v: string) => { setStrategy(v); await setAm('strategy', v); };

  /* 前端自动生成 Client Key：生成 sk-… 写入配置并立即热更新，方便直接复制使用 */
  const genKey = async () => {
    const hex = Array.from(crypto.getRandomValues(new Uint8Array(24)))
      .map((b) => b.toString(16).padStart(2, '0')).join('');
    const newKey = 'sk-' + hex;
    try {
      const cfg: any = await invoke('load_config');
      const arr: any[] = cfg.billing?.client_keys ?? [];
      arr.push({
        key: newKey,
        name: `key${arr.length + 1}`,
        group: 'default',
        rpm: 60, tpm: 100000, concurrency: 0, balance_cny: 0,
        daily_hard_quota_tokens: 0, total_hard_quota_tokens: 0,
        allow_overdraft: false, enabled: true, rate_plan: 'default',
      });
      cfg.billing.client_keys = arr;
      await invoke('save_config', { cfg });
      await load();
      onCopy(newKey, '已生成并复制');
    } catch { /* 静默 */ }
  };

  const toggleGroup = async (i: number) => {
    const next = groups.map((g, idx) => idx === i ? { ...g, enabled: !g.enabled } : g);
    setGroups(next);
    try {
      const cfg: any = await invoke('load_config');
      cfg.node_groups = (cfg.node_groups ?? []).map((g: any, idx: number) => ({
        ...g, enabled: next[idx]?.enabled ?? g.enabled,
      }));
      await invoke('save_config', { cfg });
    } catch { /* 静默 */ }
  };

  /* 模型别名：改动后立即回写 gateway.yaml（保留已有的 cache_enable/ttl 等字段） */
  const persistAliases = async (next: any[]) => {
    setAliases(next);
    setAliasCount(next.length);
    try {
      const cfg: any = await invoke('load_config');
      const prev: any[] = cfg.model_aliases ?? [];
      // 复用已有别名上的其它字段（cache_enable/ttl 等），按 alias 匹配
      cfg.model_aliases = next.map((a) => {
        const old = prev.find(p => p.alias === a.alias);
        return {
          ...(old ?? {}),
          alias: a.alias.trim(),
          real_model: a.real_model.trim(),
          group: a.group.trim() || 'default',
          enabled: a.enabled,
        };
      }).filter((a: any) => a.alias);
      await invoke('save_config', { cfg });
    } catch { /* 静默 */ }
  };

  const updateAlias = (i: number, patch: any, persist = false) => {
    const next = aliases.map((a, idx) => idx === i ? { ...a, ...patch } : a);
    if (persist) persistAliases(next); else setAliases(next);
  };

  const addAlias = () => persistAliases([...aliases, { alias: '', real_model: '', group: 'default', enabled: true }]);
  const removeAlias = (i: number) => persistAliases(aliases.filter((_, idx) => idx !== i));

  /* 节点编辑：改写 node_groups 中某个节点的 endpoint / api_keys / protocol_hints / enabled */
  const persistGroups = async (next: GroupView[]) => {
    setGroups(next);
    try {
      const cfg: any = await invoke('load_config');
      const prevGroups: any[] = cfg.node_groups ?? [];
      cfg.node_groups = next.map((g) => {
        const old = prevGroups.find(p => p.id === g.id) ?? {};
        const oldNodes: any[] = old.nodes ?? [];
        const nodes = g.nodes.map((n) => {
          const on = oldNodes.find((x: any) => x.id === n.id) ?? {};
          return { ...on, id: n.id, endpoint: n.endpoint.trim(), api_keys: [n.api_key], protocol_hints: [n.protocol], enabled: n.enabled };
        });
        return { ...old, id: g.id, enabled: g.enabled, nodes };
      });
      await invoke('save_config', { cfg });
    } catch { /* 静默 */ }
  };
  const updateGroupNode = (gi: number, ni: number, patch: any, persist = false) => {
    const next = groups.map((g, idx) => idx === gi ? { ...g, nodes: g.nodes.map((n, j) => j === ni ? { ...n, ...patch } : n) } : g);
    if (persist) persistGroups(next); else setGroups(next);
  };
  const toggleGroupNode = (gi: number, ni: number) =>
    persistGroups(groups.map((g, idx) => idx === gi ? { ...g, nodes: g.nodes.map((n, j) => j === ni ? { ...n, enabled: !n.enabled } : n) } : g));

  return (
    <div className="space-y-5">
      {/* AUTOMODE：自动尝试可用模型 */}
      <PillCard padding="md" hoverable={false}>
        <div className="flex items-center justify-between gap-4">
          <div>
            <div className="font-semibold flex items-center gap-2">
              <Icon name="zap" size={15} />
              自动尝试可用模型（AUTOMODE）
              <PillBadge size="sm">AUTOMODE</PillBadge>
            </div>
            <p className="text-xs text-neutral-500 mt-1.5 leading-relaxed">
              开启后，下游请求 model=&quot;AUTOMODE&quot; 时网关自动在模型目录的全部「源 × 模型」间尝试并降级：某个源限流/挂起就自动换下一个。源越多越稳，加一堆免费限流 API 即可放心用。
            </p>
          </div>
          <div className="shrink-0">
            <PillSwitch size="sm" checked={automode} onChange={toggleAutomode} label="" />
          </div>
        </div>

        {/* 候选策略：负载均衡 / 单一顺序死扛 */}
        <div className="mt-3 pt-3 border-t border-neutral-100 dark:border-neutral-900 grid grid-cols-1 md:grid-cols-2 gap-3">
          <div className="p-field">
            <label className="p-label">候选策略</label>
            <select className="pill-input bg-white dark:bg-neutral-950 border-neutral-200 dark:border-neutral-800"
              value={strategy}
              onChange={(e) => changeStrategy(e.target.value)}
              disabled={!automode}>
              <option value="balance">负载均衡（按质量动态调度）</option>
              <option value="sticky">单一顺序死扛（固定顺序，能扛就一直用）</option>
            </select>
          </div>
          <div className="text-[11px] text-neutral-500 self-end pb-2 leading-relaxed">
            候选链最多 48 个源，依次重试直到成功；排序规则由下方开关控制，保存即热生效。
          </div>
        </div>

        {/* 排序开关组（多级排序键，从上到下依次生效） */}
        <div className="mt-3 pt-3 border-t border-neutral-100 dark:border-neutral-900 grid grid-cols-1 md:grid-cols-2 gap-x-6 gap-y-3">
          <div className="flex items-center justify-between gap-4">
            <div>
              <div className="text-sm font-medium">免费源优先</div>
              <p className="text-xs text-neutral-500 mt-0.5">自动识别模型名/端点带 free 的免费源，排前面先试</p>
            </div>
            <div className="shrink-0">
              <PillSwitch size="sm" checked={preferFree} onChange={togglePreferFree} label="" />
            </div>
          </div>
          <div className="flex items-center justify-between gap-4">
            <div>
              <div className="text-sm font-medium">非量化优先</div>
              <p className="text-xs text-neutral-500 mt-0.5">模型名含量化标记（q4/q6/int4/gptq/awq/gguf）的排后</p>
            </div>
            <div className="shrink-0">
              <PillSwitch size="sm" checked={preferNonQuant} onChange={togglePreferNonQuant} label="" />
            </div>
          </div>
          <div className="flex items-center justify-between gap-4">
            <div>
              <div className="text-sm font-medium">大模型优先</div>
              <p className="text-xs text-neutral-500 mt-0.5">按模型名解析参数量（70b &gt; 14b），大的排前</p>
            </div>
            <div className="shrink-0">
              <PillSwitch size="sm" checked={preferLarge} onChange={togglePreferLarge} label="" />
            </div>
          </div>
          <div className="flex items-center justify-between gap-4">
            <div>
              <div className="text-sm font-medium">小模型靠后</div>
              <p className="text-xs text-neutral-500 mt-0.5">标注参数量 ≤32B（8b/14b/32b）的排后</p>
            </div>
            <div className="shrink-0">
              <PillSwitch size="sm" checked={depSmall} onChange={toggleDepSmall} label="" />
            </div>
          </div>
        </div>
      </PillCard>

      {/* 概览卡 */}
      <div className="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-4 gap-4">
        <PillCard padding="md">
          <div className="flex items-start justify-between">
            <div>
              <div className="text-[11px] text-neutral-500 uppercase tracking-wide">{t('gateway.listen')}</div>
              <div className="text-lg font-bold mt-2 tabular-nums font-mono break-all">{listen || '—'}</div>
              <div className="mt-2 text-[11px] text-neutral-500">{t('gateway.listen_hint')}</div>
            </div>
            <div className="h-10 w-10 rounded-pill bg-neutral-100 dark:bg-neutral-900 flex items-center justify-center shrink-0">
              <Icon name="link" size={18} />
            </div>
          </div>
        </PillCard>

        <PillCard padding="md">
          <div className="flex items-start justify-between">
            <div>
              <div className="text-[11px] text-neutral-500 uppercase tracking-wide">{t('gateway.upstream_nodes')}</div>
              <div className="text-2xl font-bold mt-2 tabular-nums">{groups.reduce((s, g) => s + g.nodeCount, 0)}</div>
              <div className="mt-2 text-[11px] text-neutral-500">{t('gateway.upstream_groups', { count: groups.length })}</div>
            </div>
            <div className="h-10 w-10 rounded-pill bg-neutral-100 dark:bg-neutral-900 flex items-center justify-center shrink-0">
              <Icon name="server" size={18} />
            </div>
          </div>
        </PillCard>

        <PillCard padding="md">
          <div className="flex items-start justify-between">
            <div>
              <div className="text-[11px] text-neutral-500 uppercase tracking-wide">{t('gateway.client_keys')}</div>
              <div className="text-2xl font-bold mt-2 tabular-nums">{keys.length}</div>
              <div className="mt-2 text-[11px] text-neutral-500">{t('gateway.downstream_auth')}</div>
            </div>
            <div className="h-10 w-10 rounded-pill bg-neutral-100 dark:bg-neutral-900 flex items-center justify-center shrink-0">
              <Icon name="key" size={18} />
            </div>
          </div>
        </PillCard>

        <PillCard padding="md">
          <div className="flex items-start justify-between">
            <div>
              <div className="text-[11px] text-neutral-500 uppercase tracking-wide">{t('gateway.model_alias')}</div>
              <div className="text-2xl font-bold mt-2 tabular-nums">{aliasCount}</div>
              <div className="mt-2 text-[11px] text-neutral-500">{t('gateway.alias_maps', { count: aliasCount })}</div>
            </div>
            <div className="h-10 w-10 rounded-pill bg-neutral-100 dark:bg-neutral-900 flex items-center justify-center shrink-0">
              <Icon name="activity" size={18} />
            </div>
          </div>
        </PillCard>
      </div>

      {/* 模型别名（可编辑：改名 / 真实模型 / 分组 / 启停 / 新增 / 删除） */}
      <PillCard padding="none">
        <div className="flex items-center justify-between px-5 py-4 border-b border-neutral-200/70 dark:border-neutral-800/70">
          <div className="flex items-center gap-2">
            <div className="h-8 w-8 rounded-pill bg-neutral-100 dark:bg-neutral-900 flex items-center justify-center">
              <Icon name="activity" size={16} />
            </div>
            <div>
              <div className="font-semibold">{t('gateway.aliases_title')}</div>
              <div className="text-[11px] text-neutral-500">{t('gateway.aliases_hint')}</div>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <PillBadge variant="neutral" size="sm">{t('switch.total_count', { count: aliases.length })}</PillBadge>
            <PillButton size="sm" onClick={addAlias} leftIcon={<Icon name="plus" size={14} />}>{t('gateway.add_alias')}</PillButton>
          </div>
        </div>
        <div className="overflow-hidden rounded-b-softer">
          <div className="grid grid-cols-12 px-5 py-2.5 text-[11px] font-medium text-neutral-500
                          bg-neutral-50 dark:bg-neutral-900/60 border-b border-neutral-200/70 dark:border-neutral-800/70">
            <div className="col-span-3">{t('gateway.col_alias')}</div>
            <div className="col-span-3">{t('gateway.col_real_model')}</div>
            <div className="col-span-2">{t('gateway.col_group')}</div>
            <div className="col-span-1 text-right">{t('gateway.col_enabled')}</div>
            <div className="col-span-3 text-right">{t('common.ops')}</div>
          </div>
          {aliases.length === 0 ? (
            <div className="px-5 py-4 text-sm text-neutral-500">{t('gateway.no_alias')}</div>
          ) : aliases.map((a, i) => (
            <div key={i} className={`grid grid-cols-12 px-5 py-3 text-xs items-center gap-2 border-b last:border-b-0
                       border-neutral-200/50 dark:border-neutral-800/50 ${!a.enabled ? 'opacity-55 saturate-50' : ''}`}>
              <div className="col-span-3 min-w-0">
                <input value={a.alias} placeholder={t('gateway.alias_placeholder')}
                  onChange={e => updateAlias(i, { alias: e.target.value })}
                  onBlur={() => persistAliases(aliases)}
                  className="w-full font-mono text-xs bg-neutral-100 dark:bg-neutral-900 rounded-pill px-3 py-1.5 outline-none focus:ring-2 focus:ring-neutral-400/40" />
              </div>
              <div className="col-span-3 min-w-0">
                <input value={a.real_model} placeholder={t('gateway.real_model_placeholder')}
                  onChange={e => updateAlias(i, { real_model: e.target.value })}
                  onBlur={() => persistAliases(aliases)}
                  className="w-full font-mono text-xs bg-neutral-100 dark:bg-neutral-900 rounded-pill px-3 py-1.5 outline-none focus:ring-2 focus:ring-neutral-400/40" />
              </div>
              <div className="col-span-2 min-w-0">
                <select value={groupOptions.includes(a.group) ? a.group : 'auto'}
                  onChange={e => updateAlias(i, { group: e.target.value }, true)}
                  className="w-full font-mono text-xs bg-neutral-100 dark:bg-neutral-900 rounded-pill px-3 py-1.5 outline-none focus:ring-2 focus:ring-neutral-400/40">
                  <option value="auto">自动匹配</option>
                  {groups.map((g) => <option key={g.id} value={g.id}>{g.id}</option>)}
                </select>
              </div>
              <div className="col-span-1 flex justify-end">
                <button aria-label={t('gateway.toggle_enabled')} onClick={() => persistAliases(aliases.map((x, idx) => idx === i ? { ...x, enabled: !x.enabled } : x))}
                  className={`h-6 w-6 rounded-pill flex items-center justify-center ${a.enabled ? 'bg-neutral-900 text-white dark:bg-white dark:text-black' : 'bg-neutral-100 dark:bg-neutral-900'}`}>
                  <Icon name={a.enabled ? 'pause' : 'play'} size={12} />
                </button>
              </div>
              <div className="col-span-3 flex justify-end">
                <button aria-label={t('common.delete')} onClick={() => removeAlias(i)}
                  className="h-6 w-6 rounded-pill flex items-center justify-center text-neutral-500 hover:text-red-500 hover:bg-red-500/10">
                  <Icon name="trash-2" size={13} />
                </button>
              </div>
            </div>
          ))}
        </div>
      </PillCard>

      {/* Client Keys 列表 */}
      <PillCard padding="none">
        <div className="flex items-center justify-between px-5 py-4 border-b border-neutral-200/70 dark:border-neutral-800/70">
          <div className="flex items-center gap-2">
            <div className="h-8 w-8 rounded-pill bg-neutral-100 dark:bg-neutral-900 flex items-center justify-center">
              <Icon name="shield" size={16} />
            </div>
            <div>
              <div className="font-semibold">{t('gateway.keys_title')}</div>
              <div className="text-[11px] text-neutral-500">{t('gateway.keys_hint')}</div>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <PillButton size="sm" onClick={genKey} leftIcon={<Icon name="key" size={14} />}>
              自动生成
            </PillButton>
            <PillBadge variant="neutral" size="sm">{t('switch.total_count', { count: keys.length })}</PillBadge>
          </div>
        </div>
        {keys.length === 0 ? (
          <div className="px-5 py-4 text-sm text-neutral-500">
            {t('gateway.no_key')}
          </div>
        ) : (
          <div className="overflow-hidden rounded-b-softer">
            <div className="grid grid-cols-12 px-5 py-2.5 text-[11px] font-medium text-neutral-500
                            bg-neutral-50 dark:bg-neutral-900/60 border-b border-neutral-200/70 dark:border-neutral-800/70">
              <div className="col-span-3">{t('gateway.col_name')}</div>
              <div className="col-span-3 hidden lg:block">{t('gateway.col_key')}</div>
              <div className="col-span-1">{t('gateway.col_group')}</div>
              <div className="col-span-1 text-right">{t('gateway.col_rpm')}</div>
              <div className="col-span-2 text-right">{t('gateway.col_enabled')}</div>
            </div>
            {keys.map((k, i) => (
              <div key={i} className={`grid grid-cols-12 px-5 py-3 text-xs items-center border-b last:border-b-0
                         border-neutral-200/50 dark:border-neutral-800/50 ${!k.enabled ? 'opacity-60' : ''}`}>
                <div className="col-span-3 min-w-0 truncate font-medium">{k.name}</div>
                <div className="col-span-3 hidden lg:block font-mono text-xs text-neutral-500 truncate">{k.masked}</div>
                <div className="col-span-1"><PillBadge variant="muted" size="sm">{k.group}</PillBadge></div>
                <div className="col-span-1 text-right tabular-nums">{fmtInt(k.rpm)}</div>
                <div className="col-span-2 flex justify-end">
                  <button aria-label={t('gateway.toggle_enabled')} onClick={() => toggleKey(i)}
                    className={`h-6 w-6 rounded-pill flex items-center justify-center ${k.enabled ? 'bg-neutral-900 text-white dark:bg-white dark:text-black' : 'bg-neutral-100 dark:bg-neutral-900'}`}>
                    <Icon name={k.enabled ? 'pause' : 'play'} size={12} />
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </PillCard>

      {/* 上游节点组概览 */}
      <PillCard padding="none">
        <div className="flex items-center justify-between px-5 py-4 border-b border-neutral-200/70 dark:border-neutral-800/70">
          <div className="flex items-center gap-2">
            <div className="h-8 w-8 rounded-pill bg-neutral-100 dark:bg-neutral-900 flex items-center justify-center">
              <Icon name="network" size={16} />
            </div>
            <div>
              <div className="font-semibold">{t('gateway.groups_title')}</div>
              <div className="text-[11px] text-neutral-500">{t('gateway.groups_hint')}</div>
            </div>
          </div>
        </div>
        {groups.length === 0 ? (
          <div className="px-5 py-4 text-sm text-neutral-500">{t('gateway.no_groups')}</div>
        ) : (
          <div className="overflow-hidden rounded-b-softer">
            <div className="grid grid-cols-12 px-5 py-2.5 text-[11px] font-medium text-neutral-500
                            bg-neutral-50 dark:bg-neutral-900/60 border-b border-neutral-200/70 dark:border-neutral-800/70">
              <div className="col-span-4">{t('gateway.col_group')}</div>
              <div className="col-span-4 md:col-span-5">{t('gateway.col_protocol')}</div>
              <div className="col-span-2 text-right">{t('gateway.col_nodes')}</div>
              <div className="col-span-2 text-right">{t('gateway.col_enabled')}</div>
            </div>
            {groups.map((g, i) => (
              <div key={g.id} className={`border-b last:border-b-0 border-neutral-200/50 dark:border-neutral-800/50 ${!g.enabled ? 'opacity-55 saturate-50' : ''}`}>
                {/* 组汇总行 */}
                <div className="grid grid-cols-12 px-5 py-3 text-xs items-center">
                  <div className="col-span-4 font-mono truncate min-w-0">{g.id}</div>
                  <div className="col-span-4 md:col-span-5 flex flex-wrap gap-1 min-w-0">
                    {g.protocols.length === 0
                      ? <span className="text-neutral-400">—</span>
                      : g.protocols.map(p => <PillBadge key={p} variant="neutral" size="sm" className="font-mono">{p}</PillBadge>)}
                  </div>
                  <div className="col-span-2 text-right tabular-nums">{g.nodeCount}</div>
                  <div className="col-span-2 flex justify-end">
                    <button aria-label={t('gateway.toggle_enabled')} onClick={() => toggleGroup(i)}
                      className={`h-6 w-6 rounded-pill flex items-center justify-center ${g.enabled ? 'bg-neutral-900 text-white dark:bg-white dark:text-black' : 'bg-neutral-100 dark:bg-neutral-900'}`}>
                      <Icon name={g.enabled ? 'pause' : 'play'} size={12} />
                    </button>
                  </div>
                </div>
                {/* 节点编辑行：endpoint / api key / protocol / 启停 */}
                {g.nodes.map((n, j) => (
                  <div key={j} className="grid grid-cols-12 gap-2 px-5 pb-3 text-xs items-center">
                    <div className="col-span-12 md:col-span-5">
                      <input value={n.endpoint} placeholder="https://..."
                        onChange={e => updateGroupNode(i, j, { endpoint: e.target.value })}
                        onBlur={() => persistGroups(groups)}
                        className="w-full font-mono text-xs bg-neutral-100 dark:bg-neutral-900 rounded-pill px-3 py-1.5 outline-none focus:ring-2 focus:ring-neutral-400/40" />
                    </div>
                    <div className="col-span-6 md:col-span-4">
                      <input value={n.api_key} type="password" placeholder="API Key"
                        onChange={e => updateGroupNode(i, j, { api_key: e.target.value })}
                        onBlur={() => persistGroups(groups)}
                        className="w-full font-mono text-xs bg-neutral-100 dark:bg-neutral-900 rounded-pill px-3 py-1.5 outline-none focus:ring-2 focus:ring-neutral-400/40" />
                    </div>
                    <div className="col-span-4 md:col-span-2">
                      <input value={n.protocol} placeholder="protocol"
                        onChange={e => updateGroupNode(i, j, { protocol: e.target.value })}
                        onBlur={() => persistGroups(groups)}
                        className="w-full font-mono text-xs bg-neutral-100 dark:bg-neutral-900 rounded-pill px-3 py-1.5 outline-none focus:ring-2 focus:ring-neutral-400/40" />
                    </div>
                    <div className="col-span-2 md:col-span-1 flex justify-end">
                      <button aria-label={t('gateway.toggle_enabled')} onClick={() => toggleGroupNode(i, j)}
                        className={`h-6 w-6 rounded-pill flex items-center justify-center ${n.enabled ? 'bg-neutral-900 text-white dark:bg-white dark:text-black' : 'bg-neutral-100 dark:bg-neutral-900'}`}>
                        <Icon name={n.enabled ? 'pause' : 'play'} size={12} />
                      </button>
                    </div>
                  </div>
                ))}
              </div>
            ))}
          </div>
        )}
      </PillCard>

      {copyToast && (
        <div className="fixed bottom-6 left-1/2 -translate-x-1/2 z-50
                        rounded-pill bg-black/90 dark:bg-white/90 text-white dark:text-black
                        text-xs px-4 py-2 shadow-card animate-[fadeInUp_.25s_ease]">
          {copyToast}
        </div>
      )}
    </div>
  );
};

export default GatewaySection;
