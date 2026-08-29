import React, { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import PillCard from '@components/ui/PillCard';
import PillBadge from '@components/ui/PillBadge';
import PillButton from '@components/ui/PillButton';
import PillInput from '@components/ui/PillInput';
import PillModal from '@components/ui/PillModal';
import Icon from '@icons/index';
import { invoke } from '@commands/index';
import { accessHost } from '../utils/net';

/**
 * 网关配置（真实数据，无 demo）：
 *  - 顶部：下游 Client Key 管理（新增/修改/启停/删除）—— 下游连网关的鉴权 key
 *  - 下方：上游网关管理（填 endpoint + key + 协议），可编辑/启停/删除 —— 转发目标
 *  数据全部经 backend gateway.yaml 读写，字段与后端结构对齐。
 */

type UpstreamNode = {
  id: string;
  endpoint: string;
  api_keys?: string[];
  protocol_hints: string[];   // 与后端 UpstreamNode.protocol_hints 对齐
  enabled: boolean;
  weight: number;
};

type UpstreamGroup = {
  id: string;
  nodes: UpstreamNode[];
  enabled: boolean;
};

type ClientKey = {
  key: string;
  name: string;
  group?: string;
  rpm?: number;
  tpm?: number;
  enabled: boolean;
};

type ModelAlias = {
  alias: string;
  real_model: string;
  group: string;
  enabled: boolean;
};

const PROTOCOLS = [
  { value: 'openai',          label: 'OpenAI Chat Completions' },
  { value: 'anthropic',       label: 'Anthropic Messages' },
  { value: 'openai_response', label: 'OpenAI Responses' },
  { value: 'gemini',          label: 'Gemini generateContent' },
  { value: 'ollama',          label: 'Ollama Native' },
];

const MANUAL_GROUP = 'manual';
const emptyNode = (): UpstreamNode => ({
  id: `up-${Date.now().toString(36)}`,
  endpoint: '', api_keys: [], protocol_hints: [], enabled: true, weight: 1.0,
});
const emptyKey = (): ClientKey => ({ key: '', name: '', group: 'default', rpm: 60, tpm: 100000, enabled: true });

const Switch: React.FC = () => {
  const { t } = useTranslation();
  // 真实数据：从后端 gateway.yaml 读取
  const [groups, setGroups] = useState<UpstreamGroup[]>([]);
  const [keys, setKeys] = useState<ClientKey[]>([]);
  const [aliases, setAliases] = useState<ModelAlias[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [gatewayAddr, setGatewayAddr] = useState('');
  const [gatewayKey, setGatewayKey] = useState('');

  // 添加上游表单
  const [showForm, setShowForm] = useState(false);
  const [editingNode, setEditingNode] = useState<UpstreamNode | null>(null);
  const [fEndpoint, setFEndpoint] = useState('');
  const [fKey, setFKey] = useState('');
  const [fProtocol, setFProtocol] = useState('openai_chat');
  // 上游节点连通性测试（保存前）
  const [testState, setTestState] = useState<'idle' | 'testing' | 'ok' | 'fail'>('idle');
  const [testMsg, setTestMsg] = useState('');

  // 下游 Key 管理
  const [showKeyForm, setShowKeyForm] = useState(false);
  const [editingKey, setEditingKey] = useState<ClientKey | null>(null);
  const [kName, setKName] = useState('');
  const [kKey, setKKey] = useState('');
  const [kGroup, setKGroup] = useState('default');

  const loadConfig = async () => {
    try {
      setLoading(true); setError(null);
      const cfg: any = await invoke('load_config');
      if (cfg && Array.isArray(cfg.node_groups)) setGroups(cfg.node_groups);
      if (cfg && Array.isArray(cfg.billing?.client_keys)) setKeys(cfg.billing.client_keys);
      if (cfg && Array.isArray(cfg.model_aliases)) setAliases(cfg.model_aliases.map((a: any) => ({
        alias: a.alias ?? '', real_model: a.real_model ?? '', group: a.group ?? 'manual', enabled: a.enabled !== false,
      })));
      if (cfg && cfg.http && cfg.http.listen) setGatewayAddr(accessHost(cfg.http.listen));
      const k = cfg?.billing?.client_keys?.[0];
      if (k && k.key) setGatewayKey(k.key);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally { setLoading(false); }
  };

  useEffect(() => { loadConfig(); }, []);

  const persist = async (cfg: any) => {
    await invoke('save_config', { cfg });
  };

  // ---------- 上游节点 ----------
  const openAddForm = () => {
    setEditingNode(null);
    setFEndpoint(''); setFKey(''); setFProtocol('openai_chat');
    setTestState('idle'); setTestMsg('');
    setShowForm(true);
  };
  const openEditForm = (n: UpstreamNode) => {
    setEditingNode(n);
    setFEndpoint(n.endpoint || '');
    setFKey(n.api_keys?.[0] || '');
    setFProtocol(n.protocol_hints?.[0] || 'openai_chat');
    setTestState('idle'); setTestMsg('');
    setShowForm(true);
  };

  // 保存前测试上游连通性/鉴权
  const runTest = async () => {
    if (!fEndpoint.trim()) { setTestState('fail'); setTestMsg('请先填写上游 URL'); return; }
    setTestState('testing'); setTestMsg('测试中…');
    try {
      const r: any = await invoke('test_node', { endpoint: fEndpoint, apiKey: fKey, protocol: fProtocol });
      if (r && r.ok) { setTestState('ok'); setTestMsg(r.message || '✓ 测试通过'); }
      else { setTestState('fail'); setTestMsg(r?.message || '测试未通过'); }
    } catch (e: any) {
      setTestState('fail'); setTestMsg(e?.message || String(e));
    }
  };

  const saveNode = async () => {
    try {
      setError(null);
      const node: UpstreamNode = editingNode
        ? { ...editingNode, endpoint: fEndpoint.trim(), api_keys: fKey.trim() ? [fKey.trim()] : [], protocol_hints: [fProtocol] }
        : { ...emptyNode(), endpoint: fEndpoint.trim(), api_keys: fKey.trim() ? [fKey.trim()] : [], protocol_hints: [fProtocol] };

      let nextGroups = [...groups];
      if (editingNode) {
        nextGroups = nextGroups.map(g => g.id !== MANUAL_GROUP ? g : {
          ...g, nodes: g.nodes.map(n => n.id === editingNode.id ? node : n),
        });
      } else {
        const cur = nextGroups.find(g => g.id === MANUAL_GROUP);
        if (cur) {
          nextGroups = nextGroups.map(g => g.id === MANUAL_GROUP ? { ...g, nodes: [...g.nodes, node] } : g);
        } else {
          nextGroups = [...nextGroups, { id: MANUAL_GROUP, nodes: [node], enabled: true }];
        }
      }

      const cfg: any = await invoke('load_config');
      cfg.node_groups = nextGroups;
      await persist(cfg);
      setGroups(nextGroups);
      setShowForm(false);
      setFEndpoint(''); setFKey('');
    } catch (e) { setError(e instanceof Error ? e.message : String(e)); }
  };

  const toggleNode = async (gid: string, nid: string) => {
    const next = groups.map(g => g.id !== gid ? g : {
      ...g, nodes: g.nodes.map(n => n.id === nid ? { ...n, enabled: !n.enabled } : n),
    });
    setGroups(next);
    try {
      const cfg: any = await invoke('load_config');
      cfg.node_groups = next;
      await persist(cfg);
    } catch { /* 静默 */ }
  };

  const removeNode = async (gid: string, nid: string) => {
    const next = groups.map(g => g.id !== gid ? g : {
      ...g, nodes: g.nodes.filter(n => n.id !== nid),
    });
    setGroups(next);
    try {
      const cfg: any = await invoke('load_config');
      cfg.node_groups = next;
      await persist(cfg);
    } catch { /* 静默 */ }
  };

  // ---------- 下游 Client Key ----------
  const openAddKey = () => {
    setEditingKey(null);
    // 自动生成 key：打开「新增 Client Key」即填好 key 和名称，无需手填
    const hex = Array.from(crypto.getRandomValues(new Uint8Array(24)))
      .map((b) => b.toString(16).padStart(2, '0')).join('');
    setKName(`key${keys.length + 1}`); setKKey(`sk-${hex}`); setKGroup('default');
    setShowKeyForm(true);
  };
  const openEditKey = (k: ClientKey) => {
    setEditingKey(k);
    setKName(k.name || ''); setKKey(k.key || ''); setKGroup(k.group || 'default');
    setShowKeyForm(true);
  };
  const saveKey = async () => {
    try {
      setError(null);
      // 编辑时保留原 key 的全部字段（计费/配额等），仅覆盖表单编辑的三项；新建时补默认值
      const base: any = editingKey ?? {};
      const built: any = {
        rpm: 60, tpm: 100000, concurrency: 0,
        balance_cny: 0, daily_hard_quota_tokens: 0, total_hard_quota_tokens: 0,
        allow_overdraft: false, enabled: true, rate_plan: 'default',
        ...base,
        key: kKey.trim(), name: kName.trim() || 'key', group: kGroup.trim() || 'default',
      };
      let nextKeys: ClientKey[];
      if (editingKey) {
        nextKeys = keys.map(k => k.key === editingKey.key ? built : k);
      } else {
        nextKeys = [...keys, built];
      }
      const cfg: any = await invoke('load_config');
      cfg.billing.client_keys = nextKeys;
      await persist(cfg);
      setKeys(nextKeys);
      setShowKeyForm(false);
      setKName(''); setKKey(''); setKGroup('default');
    } catch (e) { setError(e instanceof Error ? e.message : String(e)); }
  };
  const toggleKey = async (k: ClientKey) => {
    const next = keys.map(x => x.key === k.key ? { ...x, enabled: !x.enabled } : x);
    setKeys(next);
    try {
      const cfg: any = await invoke('load_config');
      cfg.billing.client_keys = next;
      await persist(cfg);
    } catch { /* 静默 */ }
  };
  const removeKey = async (k: ClientKey) => {
    const next = keys.filter(x => x.key !== k.key);
    setKeys(next);
    try {
      const cfg: any = await invoke('load_config');
      cfg.billing.client_keys = next;
      await persist(cfg);
    } catch { /* 静默 */ }
  };

  // ---------- 模型映射（别名 → 真实模型 → 分组） ----------
  const persistAliases = async (next: ModelAlias[]) => {
    setAliases(next);
    try {
      setError(null);
      const cfg: any = await invoke('load_config');
      const prev: any[] = cfg.model_aliases ?? [];
      cfg.model_aliases = next.map((a) => {
        const old = prev.find(p => p.alias === a.alias);
        return { ...(old ?? {}), alias: a.alias.trim(), real_model: a.real_model.trim(), group: a.group.trim() || 'manual', enabled: a.enabled };
      }).filter((a: any) => a.alias);
      await persist(cfg);
    } catch (e) { setError(e instanceof Error ? e.message : String(e)); }
  };
  const updateAlias = (i: number, patch: any, persistNow = false) => {
    const next = aliases.map((a, idx) => idx === i ? { ...a, ...patch } : a);
    if (persistNow) persistAliases(next); else setAliases(next);
  };
  const addAlias = () => persistAliases([...aliases, { alias: '', real_model: '', group: 'manual', enabled: true }]);
  const removeAlias = (i: number) => persistAliases(aliases.filter((_, idx) => idx !== i));
  const toggleAlias = (i: number) => persistAliases(aliases.map((x, idx) => idx === i ? { ...x, enabled: !x.enabled } : x));

  const totalNodes = groups.reduce((s, g) => s + g.nodes.length, 0);

  return (
    <div className="space-y-6">
      {/* 网关连接信息（下游入口） */}
      <PillCard padding="md">
        <div className="flex items-center gap-3 flex-wrap">
          <div className="h-9 w-9 rounded-pill bg-neutral-100 dark:bg-neutral-900 flex items-center justify-center">
            <Icon name="network" size={16} />
          </div>
          <div className="flex-1 min-w-0">
            <div className="font-semibold">{t('switch.gateway_connect')}</div>
            <div className="text-[11px] text-neutral-500">
              {t('switch.gateway_connect_hint')}
            </div>
          </div>
          <div className="flex flex-wrap gap-2">
            <PillButton size="sm" variant="soft" leftIcon={<Icon name="plus" size={14} />} onClick={openAddForm}>{t('switch.add_upstream')}</PillButton>
            <PillButton size="sm" variant="ghost" leftIcon={<Icon name="plus" size={14} />} onClick={openAddKey}>{t('switch.add_client_key')}</PillButton>
            <PillButton size="sm" variant="ghost" leftIcon={<Icon name="refresh-cw" size={14} />} onClick={loadConfig}>{t('switch.refresh')}</PillButton>
          </div>
        </div>
        <div className="mt-4 grid grid-cols-1 md:grid-cols-2 gap-3">
          <div className="rounded-softer border border-neutral-200/70 dark:border-neutral-800/70 p-4">
            <div className="text-[11px] text-neutral-500 mb-1">{t('switch.base_url')}</div>
            <div className="font-mono text-sm break-all">{gatewayAddr || 'http://127.0.0.1:8787'}</div>
          </div>
          <div className="rounded-softer border border-neutral-200/70 dark:border-neutral-800/70 p-4">
            <div className="text-[11px] text-neutral-500 mb-1">{t('switch.auth_key')}</div>
            <div className="font-mono text-sm break-all">{gatewayKey || t('switch.not_configured')}</div>
          </div>
        </div>
      </PillCard>

      {error && (
        <div className="rounded-softer border border-dashed border-neutral-300 dark:border-neutral-700 p-4 text-sm text-neutral-600">
          {t('switch.ops_failed')}: {error}
        </div>
      )}

      {/* 下游 Client Key 管理 */}
      <PillCard padding="none">
        <div className="flex items-center justify-between px-5 py-4 border-b border-neutral-200/70 dark:border-neutral-800/70">
          <div className="flex items-center gap-2">
            <div className="h-8 w-8 rounded-pill bg-neutral-100 dark:bg-neutral-900 flex items-center justify-center">
              <Icon name="key" size={16} />
            </div>
            <div>
              <div className="font-semibold">{t('switch.client_key_mgmt')}</div>
              <div className="text-[11px] text-neutral-500">{t('switch.client_key_mgmt_hint')}</div>
            </div>
          </div>
          <PillBadge variant="neutral" size="sm">{t('switch.total_count', { count: keys.length })}</PillBadge>
        </div>

        {keys.length === 0 ? (
          <div className="px-5 py-4 text-sm text-neutral-500">{t('switch.no_key')}</div>
        ) : (
          <div className="overflow-hidden rounded-b-softer">
            <div className="grid grid-cols-12 px-5 py-2.5 text-[11px] font-medium text-neutral-500
                            bg-neutral-50 dark:bg-neutral-900/60 border-b border-neutral-200/70 dark:border-neutral-800/70">
              <div className="col-span-4">{t('switch.col_name')}</div>
              <div className="col-span-4 hidden md:block">{t('switch.col_key')}</div>
              <div className="col-span-1">{t('switch.col_group')}</div>
              <div className="col-span-3 text-right">{t('switch.col_ops')}</div>
            </div>
            {keys.map((k) => (
              <div key={k.key} className={`grid grid-cols-12 px-5 py-3 text-xs items-center border-b last:border-b-0
                         border-neutral-200/50 dark:border-neutral-800/50 ${!k.enabled ? 'opacity-60' : ''}`}>
                <div className="col-span-4 min-w-0 truncate font-medium">{k.name}</div>
                <div className="col-span-4 hidden md:block font-mono text-xs text-neutral-500 truncate">
                  {k.key.length > 12 ? k.key.slice(0, 4) + '\u00b7\u00b7\u00b7' + k.key.slice(-4) : k.key}
                </div>
                <div className="col-span-1"><PillBadge variant="muted" size="sm">{k.group || 'default'}</PillBadge></div>
                <div className="col-span-3 flex justify-end gap-1">
                  <button title={t('common.edit')} aria-label={t('common.edit')} onClick={() => openEditKey(k)}
                    className="h-7 w-7 rounded-pill bg-neutral-100 hover:bg-neutral-200 dark:bg-neutral-900 dark:hover:bg-neutral-800 flex items-center justify-center">
                    <Icon name="edit-3" size={13} />
                  </button>
                  <button title={k.enabled ? t('common.paused') : t('common.enabled')} aria-label="toggle" onClick={() => toggleKey(k)}
                    className="h-7 w-7 rounded-pill bg-neutral-100 hover:bg-neutral-200 dark:bg-neutral-900 dark:hover:bg-neutral-800 flex items-center justify-center">
                    <Icon name={k.enabled ? 'pause' : 'play'} size={13} />
                  </button>
                  <button title={t('common.delete')} aria-label={t('common.delete')} onClick={() => removeKey(k)}
                    className="h-7 w-7 rounded-pill bg-neutral-100 hover:bg-neutral-200 dark:bg-neutral-900 dark:hover:bg-neutral-800 flex items-center justify-center">
                    <Icon name="trash-2" size={13} />
                  </button>
                </div>
              </div>
            ))}
          </div>
        )}
      </PillCard>

      {/* 模型映射（别名 → 真实模型 → 分组），可任意增删改 */}
      <PillCard padding="none">
        <div className="flex items-center justify-between px-5 py-4 border-b border-neutral-200/70 dark:border-neutral-800/70">
          <div className="flex items-center gap-2">
            <div className="h-8 w-8 rounded-pill bg-neutral-100 dark:bg-neutral-900 flex items-center justify-center">
              <Icon name="activity" size={16} />
            </div>
            <div>
              <div className="font-semibold">{t('switch.aliases_title')}</div>
              <div className="text-[11px] text-neutral-500">{t('switch.aliases_hint')}</div>
            </div>
          </div>
          <div className="flex items-center gap-2">
            <PillBadge variant="neutral" size="sm">{t('switch.total_count', { count: aliases.length })}</PillBadge>
            <PillButton size="sm" leftIcon={<Icon name="plus" size={14} />} onClick={addAlias}>{t('switch.add_alias')}</PillButton>
          </div>
        </div>
        <div className="overflow-hidden rounded-b-softer">
          <div className="grid grid-cols-12 px-5 py-2.5 text-[11px] font-medium text-neutral-500
                          bg-neutral-50 dark:bg-neutral-900/60 border-b border-neutral-200/70 dark:border-neutral-800/70">
            <div className="col-span-4">{t('switch.col_alias')}</div>
            <div className="col-span-4">{t('switch.col_real_model')}</div>
            <div className="col-span-1 text-right">{t('switch.col_enabled')}</div>
            <div className="col-span-3 text-right">{t('switch.col_ops')}</div>
          </div>
          {aliases.length === 0 ? (
            <div className="px-5 py-4 text-sm text-neutral-500">{t('switch.no_alias')}</div>
          ) : aliases.map((a, i) => (
            <div key={i} className={`grid grid-cols-12 px-5 py-3 text-xs items-center gap-2 border-b last:border-b-0
                       border-neutral-200/50 dark:border-neutral-800/50 ${!a.enabled ? 'opacity-55 saturate-50' : ''}`}>
              <div className="col-span-4 min-w-0">
                <input value={a.alias} placeholder={t('switch.alias_placeholder')}
                  onChange={e => updateAlias(i, { alias: e.target.value })}
                  onBlur={() => persistAliases(aliases)}
                  className="w-full font-mono text-xs bg-neutral-100 dark:bg-neutral-900 rounded-pill px-3 py-1.5 outline-none focus:ring-2 focus:ring-neutral-400/40" />
              </div>
              <div className="col-span-4 min-w-0">
                <input value={a.real_model} placeholder={t('switch.real_model_placeholder')}
                  onChange={e => updateAlias(i, { real_model: e.target.value })}
                  onBlur={() => persistAliases(aliases)}
                  className="w-full font-mono text-xs bg-neutral-100 dark:bg-neutral-900 rounded-pill px-3 py-1.5 outline-none focus:ring-2 focus:ring-neutral-400/40" />
              </div>
              <div className="col-span-1 flex justify-end">
                <button aria-label={t('switch.col_enabled')} onClick={() => toggleAlias(i)}
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

      {/* 上游列表 */}
      <div>
        <div className="flex items-center justify-between mb-3">
          <div className="text-sm text-neutral-500">{t('switch.groups_count', { groups: groups.length, nodes: totalNodes })}</div>
        </div>
        {loading ? (
          <div className="animate-pulse h-40 rounded-softer bg-neutral-100 dark:bg-neutral-900 border border-dashed border-neutral-200 dark:border-neutral-800" />
        ) : groups.length === 0 ? (
          <PillCard padding="md">
            <div className="text-sm text-neutral-500">{t('switch.no_upstream')}</div>
          </PillCard>
        ) : (
          <div className="space-y-3">
            {groups.map(g => g.nodes.length === 0 ? null : (
              <div key={g.id} className="space-y-2">
                <div className="flex items-center gap-2 text-xs text-neutral-500">
                  <Icon name="network" size={12} />
                  <span className="font-mono">{g.id}</span>
                  <PillBadge variant="neutral" size="sm">{t('switch.nodes', { count: g.nodes.length })}</PillBadge>
                </div>
                {g.nodes.map(n => (
                  <PillCard key={n.id} padding="sm" className={n.enabled ? '' : 'opacity-55 saturate-50'}>
                    <div className="flex items-start gap-3">
                      <div className="h-10 w-10 shrink-0 rounded-pill bg-neutral-100 dark:bg-neutral-900 flex items-center justify-center">
                        <Icon name="server" size={16} />
                      </div>
                      <div className="flex-1 min-w-0">
                        <div className="flex items-center gap-2 flex-wrap">
                          <span className="font-mono font-bold text-sm">{n.id}</span>
                          {!n.enabled && <PillBadge variant="fail" size="sm">{t('common.paused')}</PillBadge>}
                          <PillBadge variant="neutral" size="sm">{n.protocol_hints?.[0] || '—'}</PillBadge>
                        </div>
                        <div className="mt-1 text-xs text-neutral-500 truncate">
                          <Icon name="link" size={10} className="inline mr-1 align-middle" />
                          {n.endpoint}
                        </div>
                        {n.api_keys && n.api_keys.length > 0 && (
                          <div className="mt-1 text-[11px] text-neutral-400 font-mono truncate">
                            key: {n.api_keys[0].length > 12 ? n.api_keys[0].slice(0, 4) + '\u00b7\u00b7\u00b7' + n.api_keys[0].slice(-4) : n.api_keys[0]}
                          </div>
                        )}
                      </div>
                      <div className="flex gap-1 shrink-0">
                        <button title={t('common.edit')} aria-label={t('common.edit')} onClick={() => openEditForm(n)}
                          className="h-7 w-7 rounded-pill bg-neutral-100 hover:bg-neutral-200 dark:bg-neutral-900 dark:hover:bg-neutral-800 flex items-center justify-center">
                          <Icon name="edit-3" size={13} />
                        </button>
                        <button title={n.enabled ? t('common.paused') : t('common.enabled')} aria-label="toggle" onClick={() => toggleNode(g.id, n.id)}
                          className="h-7 w-7 rounded-pill bg-neutral-100 hover:bg-neutral-200 dark:bg-neutral-900 dark:hover:bg-neutral-800 flex items-center justify-center">
                          <Icon name={n.enabled ? 'pause' : 'play'} size={13} />
                        </button>
                        <button title={t('common.delete')} aria-label={t('common.delete')} onClick={() => removeNode(g.id, n.id)}
                          className="h-7 w-7 rounded-pill bg-neutral-100 hover:bg-neutral-200 dark:bg-neutral-900 dark:hover:bg-neutral-800 flex items-center justify-center">
                          <Icon name="trash-2" size={13} />
                        </button>
                      </div>
                    </div>
                  </PillCard>
                ))}
              </div>
            ))}
          </div>
        )}
      </div>

      {/* ── 添加上游 / 编辑：居中悬浮窗 ── */}
      <PillModal
        open={showForm}
        onClose={() => setShowForm(false)}
        size="lg"
        title={editingNode ? t('switch.edit_upstream') : t('switch.add_upstream_gateway')}
        footer={({ close }) => (
          <>
            <PillButton variant="soft" leftIcon={<Icon name="check" size={14} />} onClick={saveNode} disabled={testState !== 'ok'}>
              {editingNode ? t('common.save_changes') : t('switch.save_upstream')}
            </PillButton>
            <PillButton variant="ghost" onClick={close}>{t('common.cancel')}</PillButton>
          </>
        )}
      >
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <PillInput label={t('switch.upstream_url')} placeholder="https://api.openai.com/v1" value={fEndpoint}
            onChange={(e) => { setFEndpoint(e.target.value); setTestState('idle'); setTestMsg(''); }} />
          <PillInput label={t('switch.api_key')} placeholder="sk-..." type="password" value={fKey}
            onChange={(e) => { setFKey(e.target.value); setTestState('idle'); setTestMsg(''); }} />
          <div className="p-field">
            <label className="p-label">{t('switch.upstream_protocol')}</label>
            <select className="pill-input bg-white dark:bg-neutral-950 border-neutral-200 dark:border-neutral-800"
              value={fProtocol} onChange={(e) => { setFProtocol(e.target.value); setTestState('idle'); setTestMsg(''); }}>
              {PROTOCOLS.map(p => <option key={p.value} value={p.value}>{p.label}</option>)}
            </select>
          </div>
        </div>
        {/* 保存前先测试：通过后才能保存 */}
        <div className="mt-4 flex flex-wrap items-center gap-2">
          <PillButton size="sm" variant="soft" leftIcon={<Icon name="zap" size={14} />} onClick={runTest} disabled={testState === 'testing'}>
            测试连通性
          </PillButton>
          {testMsg && (
            <span className={`text-xs ${testState === 'ok' ? 'text-emerald-600 dark:text-emerald-400' : testState === 'fail' ? 'text-red-600 dark:text-red-400' : 'text-neutral-500'}`}>
              {testMsg}
            </span>
          )}
          {testState !== 'ok' && testState !== 'idle' && (
            <span className="text-xs text-neutral-400">测试通过后才能保存</span>
          )}
        </div>
      </PillModal>

      {/* ── 新增 / 编辑 下游 Client Key：居中悬浮窗 ── */}
      <PillModal
        open={showKeyForm}
        onClose={() => setShowKeyForm(false)}
        title={editingKey ? t('switch.edit_key') : t('switch.new_key')}
        footer={({ close }) => (
          <>
            <PillButton variant="soft" leftIcon={<Icon name="check" size={14} />} onClick={saveKey}>
              {editingKey ? t('common.save_changes') : t('switch.add')}
            </PillButton>
            <PillButton variant="ghost" onClick={close}>{t('common.cancel')}</PillButton>
          </>
        )}
      >
        <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
          <PillInput label={t('switch.name')} placeholder={t('switch.name_placeholder')} value={kName} onChange={(e) => setKName(e.target.value)} />
          <PillInput label={t('switch.key')} placeholder={t('switch.key_placeholder')} value={kKey} onChange={(e) => setKKey(e.target.value)} />
          <PillInput label={t('switch.group')} placeholder={t('switch.group_placeholder')} value={kGroup} onChange={(e) => setKGroup(e.target.value)} />
        </div>
      </PillModal>
    </div>
  );
};

export default Switch;
