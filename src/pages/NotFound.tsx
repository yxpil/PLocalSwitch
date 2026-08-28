import React from 'react';
import { useTranslation } from 'react-i18next';
import { NavLink, useLocation } from 'react-router-dom';
import PillCard from '@components/ui/PillCard';
import PillButton from '@components/ui/PillButton';
import Icon from '@icons/index';

const NotFound: React.FC = () => {
  const { t } = useTranslation();
  const loc = useLocation();
  return (
    <div className="min-h-[60vh] flex items-center justify-center py-10">
      <PillCard hoverable padding="lg" className="max-w-xl w-full text-center">
        <div className="mx-auto h-20 w-20 rounded-softer bg-neutral-900 dark:bg-white shadow-pill
                        flex items-center justify-center mb-5">
          <Icon name="search-x" size={36} className="text-white dark:text-black"/>
        </div>
        <div className="text-6xl font-black tracking-tight mb-2">404</div>
        <div className="font-semibold">{t('common.not_found')}</div>
        <code className="block mt-2 text-xs font-mono text-neutral-500 break-all">{loc.pathname}</code>
        <div className="mt-6 flex justify-center gap-2 flex-wrap">
          <NavLink to="/">
            <PillButton leftIcon={<Icon name="home" size={16}/>}>{t('common.back_home')}</PillButton>
          </NavLink>
          <NavLink to="/models">
            <PillButton variant="soft" leftIcon={<Icon name="sliders-horizontal" size={16}/>}>{t('nav.models')}</PillButton>
          </NavLink>
          <NavLink to="/settings">
            <PillButton variant="ghost" leftIcon={<Icon name="settings" size={16}/>}>{t('nav.settings')}</PillButton>
          </NavLink>
        </div>
      </PillCard>
    </div>
  );
};

export default NotFound;
