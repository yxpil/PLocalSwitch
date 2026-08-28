import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import Icon from '@icons/index';
import { invoke } from '@commands/index';
import { accessHost } from '../../utils/net';

type IconName = Parameters<typeof Icon>[0]['name'];

/**
 * 4 张网关概览卡（真实配置数据，无随机假数据）
 *  上游节点 / 模型别名 / Client Key / 监听地址
 */
const StatsWidget: React.FC = () => {
  const { t } = useTranslation();
  const [upstream, setUpstream] = useState(0);
  const [alias, setAlias] = useState(0);
  const [keys, setKeys] = useState(0);
  const [listen, setListen] = useState('');

  useEffect(() => {
    (async () => {
      try {
        const cfg: any = await invoke('load_config');
        setUpstream((cfg?.node_groups ?? []).reduce((s: number, g: any) => s + (g.nodes?.length ?? 0), 0));
        setAlias((cfg?.model_aliases ?? []).length);
        setKeys((cfg?.billing?.client_keys ?? []).length);
        setListen(accessHost(cfg?.http?.listen));
      } catch { /* 无后端时忽略 */ }
    })();
  }, []);

  const boxes: { label: string; icon: IconName; value: string; sub?: string }[] = [
    { label: t('home.upstream_nodes'), icon: 'server',  value: String(upstream), sub: t('home.stat_upstream_sub') },
    { label: t('gateway.model_alias'), icon: 'layers',  value: String(alias),    sub: t('home.stat_alias_sub') },
    { label: 'Client Key', icon: 'key',    value: String(keys),     sub: t('gateway.downstream_auth') },
    { label: t('gateway.listen'),  icon: 'link',    value: listen || '—',    sub: t('home.stat_listen_sub') },
  ];

  return (
    <>
      {boxes.map((b, i) => (
        <div key={i}
          className="rounded-softer p-5 bg-white dark:bg-neutral-950 border border-neutral-200/70 dark:border-neutral-800/70
                     shadow-soft hover:shadow-card transition-shadow duration-300 ease-PILL">
          <div className="flex items-start justify-between mb-3">
            <div className="text-xs font-medium text-neutral-500 dark:text-neutral-400">{b.label}</div>
            <div className="h-9 w-9 rounded-pill bg-neutral-100 dark:bg-neutral-900 flex items-center justify-center">
              <Icon name={b.icon} size={16} />
            </div>
          </div>
          <div className="flex items-baseline gap-2">
            <div className="text-2xl font-black tracking-tight tabular-nums truncate">{b.value}</div>
          </div>
          {b.sub && <div className="text-[11px] text-neutral-500 mt-1 tabular-nums">{b.sub}</div>}
        </div>
      ))}
    </>
  );
};

export default StatsWidget;
