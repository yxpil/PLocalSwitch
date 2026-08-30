import { invoke } from './ipc';
import type { AppConfig, AppInfo, SystemInfo } from '@types';

export const getAppInfo = (): Promise<AppInfo>     => invoke<AppInfo>('get_app_info');
export const ping       = (msg?: string)           => invoke<string>('ping', msg !== undefined ? { msg } : {});
export const getSystemInfo = (): Promise<SystemInfo> => invoke<SystemInfo>('get_system_info');

export const loadConfig  = (): Promise<AppConfig>  => invoke<AppConfig>('load_config');
export const saveConfig  = (cfg: AppConfig)        => invoke<AppConfig>('save_config', { cfg });

// 网关服务启停控制（桌面面板总控）
export const gatewayStatus = (): Promise<any>   => invoke<any>('gateway_status');
export const gatewayStart  = (): Promise<boolean> => invoke<boolean>('gateway_start');
export const gatewayStop   = (): Promise<boolean> => invoke<boolean>('gateway_stop');
export const gatewayRestart = (): Promise<boolean> => invoke<boolean>('restart_graceful');
/** 网关崩溃自动重启开关：enable 省略 = 查询当前状态 */
export const gatewayAutoRestart = (enable?: boolean): Promise<boolean> =>
  invoke<boolean>('gateway_auto_restart', enable !== undefined ? { enable } : {});

export * from './storage';
