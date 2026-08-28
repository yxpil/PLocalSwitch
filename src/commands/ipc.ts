/**
 * IPC 基础封装（Tauri / 纯浏览器 mock 降级，零彩色语义）
 */
import type { ApiResponse } from '@types';
import { useLogger } from '@logger/index';

const log = useLogger('ipc');

export function isTauriEnv(): boolean {
  if (typeof window === 'undefined') return false;
  return !!(window as any).__TAURI_INTERNALS__
    || /Tauri\//i.test(navigator.userAgent)
    || !!import.meta.env.TAURI_ENV_PLATFORM;
}

export async function invoke<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  const t0 = Date.now();
  try {
    if (isTauriEnv()) {
      const { invoke: raw } = await import('@tauri-apps/api/core');
      const resp = await raw(cmd, args) as ApiResponse<T> | T;
      log.debug(`[ok] ${cmd} (${Date.now() - t0}ms)`);
      return unwrap(resp);
    }
    const handler = mock[cmd];
    if (!handler) {
      log.warn(`mock: 无实现命令 ${cmd}，返回 null`);
      return null as unknown as T;
    }
    const resp = await handler(args) as ApiResponse<T> | T;
    log.debug(`[mock] ${cmd} (${Date.now() - t0}ms)`);
    return unwrap(resp);
  } catch (e) {
    log.error(`[fail] ${cmd}`, e instanceof Error ? e : new Error(String(e)));
    throw e;
  }
}

function unwrap<T>(resp: ApiResponse<T> | T): T {
  if (resp && typeof resp === 'object' && 'success' in (resp as object)) {
    const r = resp as ApiResponse<T>;
    if (!r.success) {
      const err = new Error(r.message || '后端业务失败');
      (err as any).code = (r as any).code || 'BIZ';
      throw err;
    }
    return r.data as T;
  }
  return resp as T;
}

// ============= 浏览器回退（无 Tauri 后端时）：不返回任何 demo 假数据 =============
// 说明：桌面端（Tauri）环境下永远走上方真实 IPC → Rust 命令，不会命中这里。
//       这里仅在纯浏览器预览时提供一个「空配置」骨架，让页面能展示空表格，绝不造假数据。
const mock: Record<string, (args: any) => any> = {};
function reg(cmd: string, fn: (a: any) => any) {
  mock[cmd] = async (a: any) => ({
    success: true, data: await fn(a), message: 'ok', timestamp: Date.now(),
  });
}

// 空网关配置骨架（全新安装的默认值，无任何预设节点/别名/密钥）
function emptyGatewayConfig() {
  return {
    version: "1.0",
    app: { name: "PLocalSwitch-Gateway", env: "dev", timezone: "Asia/Shanghai", log_level: "info",
      privacy: { store_payload_text: false, masking: true, mask_token_head_tail: [4, 4], mask_url_path_segment_limit: 2 } },
    http: { listen: "0.0.0.0:8787", request_body_max_bytes: 4194304, global_concurrency_limit: 512,
      per_client_key_concurrency_limit: 64, client_disconnect_aborts_upstream: true,
      timeouts: { connect_ms: 3000, read_ms: 60000, stream_read_ms: 600000 } },
    db: { backend: "sqlite", sqlite_path: "./data/pls_gateway.db", pool_max_open: 32, pool_max_idle: 8, migrate_on_start: true },
    metrics: { enabled: true, expose_at: "/metrics", process_collector: true, per_client_key_labels: true, per_node_labels: true, per_error_label: true },
    cors: { allow_origins: ["*"], allow_methods: ["GET", "POST", "OPTIONS", "DELETE", "PUT"], allow_headers: ["*"], allow_credentials: false },
    model_aliases: [],
    node_groups: [],
    flex_adapter: { sniff_attempts_per_node: 2, global_max_sub_attempts: 4, sniff_remember_ttl_seconds: 300, flexible_parse_alert_on_fallback: true, stream_lock_after_first_byte: true,
      capability: { probe_interval_seconds: 120, probe_prompt: "ping", probe_priority_nodes_only: true } },
    cache_pool: { implementation: "memory", in_memory: { max_entries_non_stream: 5000, max_entries_stream: 2000, max_total_memory_mb: 512, evict_interval_seconds: 60, hash_key_algo: "xxh3_128" }, redis: { url: "", username: "", password: "", db: 0, default_ttl_seconds: 3600 } },
    billing: { currency: "CNY", rates: [], client_keys: [], audit: { discrepancy_alarm_percent: 5.0, override_billing_when_discrepancy: false, override_prefer: "local" }, tokenizers: [] },
    node_quality: { min_samples: 10, scoring_weights: { success_rate: 0.5, latency_p99: 0.2, ttft: 0.1, error_counts: 0.1, token_discrepancy: 0.05, sse_abnormal_rate: 0.05 },
      labels: { excellent: "90..100", good: "75..89", normal: "60..74", poor: "40..59", fault: "0..39" },
      autotrim: { enabled: true, temporary_ban_seconds_when_fault: 300, demote_weight_when_poor: 0.5 } },
    policy: { retry_on: { network_connect_refused: true, dns_fail: true, connect_timeout: true, read_timeout: true, tls_error: true, http_429: true, http_5xx: true, auth_401_403: false, bad_param_4xx: false, sse_premature_close: true, json_parse_fail: true },
      analysis_history_window_seconds: 600 },
    masking: { enabled: true, sensitive_headers: ["authorization", "x-api-key"], sensitive_body_fields: ["api_key", "secret", "authorization", "password", "x-api-key"], token_show_head: 4, token_show_tail: 4, url_preserve_path_segments: 2 },
  };
}

// 浏览器回退：load_config/save_config 用一个空配置 map 模拟（仅做页面能动的演示，无任何预设假数据）
let cfgBrowser: any = emptyGatewayConfig();
reg('load_config', () => cfgBrowser);
reg('save_config', (a: { cfg: any }) => { cfgBrowser = { ...a.cfg }; return cfgBrowser; });
reg('reset_config', () => { cfgBrowser = emptyGatewayConfig(); return cfgBrowser; });
