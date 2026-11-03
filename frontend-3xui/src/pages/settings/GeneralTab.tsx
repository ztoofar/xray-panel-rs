import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Input, InputNumber, Select, Switch, Tabs } from 'antd';
import {
  ClockCircleOutlined,
  DeploymentUnitOutlined,
  SettingOutlined,
} from '@ant-design/icons';
import type { AllSetting } from '@/models/setting';
import { LanguageManager } from '@/utils';
import { onNumber } from '@/utils/onNumber';
import { DefaultSettingTag, SettingListItem } from '@/components/ui';
import { useMediaQuery } from '@/hooks/useMediaQuery';
import { catTabLabel } from './catTabLabel';
import PanelUpdateSection from './PanelUpdateSection';
import { normalizePath, sanitizePath } from './uriPath';

interface GeneralTabProps {
  allSetting: AllSetting;
  updateSetting: (patch: Partial<AllSetting>) => void;
}

// Only settings this Rust panel actually applies are exposed here.
export default function GeneralTab({ allSetting, updateSetting }: GeneralTabProps) {
  const { t } = useTranslation();
  const { isMobile } = useMediaQuery();
  const [lang, setLang] = useState<string>(() => LanguageManager.getLanguage());

  function onLangChange(value: string) {
    setLang(value);
    LanguageManager.setLanguage(value);
  }

  const langOptions = LanguageManager.supportedLanguages.map(
    (item: { value: string; name: string; icon: string }) => ({
      value: item.value,
      label: (
        <>
          <span role="img" aria-label={item.name}>
            {item.icon}
          </span>
          &nbsp;&nbsp;
          <span>{item.name}</span>
        </>
      ),
    }),
  );

  return (
    <Tabs
      defaultActiveKey="1"
      items={[
        {
          key: '1',
          label: catTabLabel(
            <SettingOutlined aria-hidden />,
            t('pages.settings.panelSettings'),
            isMobile,
          ),
          children: (
            <>
              <SettingListItem
                paddings="small"
                title={t('pages.settings.panelListeningIP')}
                description={t('pages.settings.panelListeningIPDesc')}
              >
                <Input
                  value={allSetting.webListen}
                  onChange={(e) => updateSetting({ webListen: e.target.value })}
                />
              </SettingListItem>
              <SettingListItem
                paddings="small"
                title={t('pages.settings.panelPort')}
                description={t('pages.settings.panelPortDesc')}
                badge={<DefaultSettingTag settingKey="webPort" value={allSetting.webPort} />}
              >
                <InputNumber
                  value={allSetting.webPort}
                  min={1}
                  max={65535}
                  style={{ width: '100%' }}
                  onChange={onNumber((v) => updateSetting({ webPort: v }))}
                />
              </SettingListItem>
              <SettingListItem
                paddings="small"
                title={t('pages.settings.panelUrlPath')}
                description={t('pages.settings.panelUrlPathDesc')}
              >
                <Input
                  value={allSetting.webBasePath}
                  onChange={(e) => updateSetting({ webBasePath: sanitizePath(e.target.value) })}
                  onBlur={() =>
                    updateSetting({ webBasePath: normalizePath(allSetting.webBasePath) })
                  }
                />
              </SettingListItem>
              <SettingListItem
                paddings="small"
                title={t('pages.settings.nodeAddress')}
                description={t('pages.settings.nodeAddressDesc')}
              >
                <Input
                  value={allSetting.nodeAddress}
                  placeholder="node.example.com / 203.0.113.10"
                  onChange={(e) => updateSetting({ nodeAddress: e.target.value.trim() })}
                />
              </SettingListItem>
              <SettingListItem
                paddings="small"
                title={t('pages.settings.sessionMaxAge')}
                description={t('pages.settings.sessionMaxAgeDesc')}
              >
                <InputNumber
                  value={allSetting.sessionMaxAge}
                  min={1}
                  max={525600}
                  style={{ width: '100%' }}
                  onChange={onNumber((v) => updateSetting({ sessionMaxAge: v }))}
                />
              </SettingListItem>
              <SettingListItem
                paddings="small"
                title={t('pages.settings.pageSize')}
                description={t('pages.settings.pageSizeDesc')}
              >
                <InputNumber
                  value={allSetting.pageSize}
                  min={0}
                  max={1000}
                  style={{ width: '100%' }}
                  onChange={onNumber((v) => updateSetting({ pageSize: v }))}
                />
              </SettingListItem>
              <SettingListItem
                paddings="small"
                title={t('pages.settings.restartXrayOnClientDisable')}
                description={t('pages.settings.restartXrayOnClientDisableDesc')}
              >
                <Switch
                  checked={allSetting.restartXrayOnClientDisable}
                  onChange={(v) => updateSetting({ restartXrayOnClientDisable: v })}
                />
              </SettingListItem>
              <SettingListItem
                paddings="small"
                title={t('pages.settings.loginNotifyWebhook')}
                description={t('pages.settings.loginNotifyWebhookDesc')}
              >
                <Input
                  value={allSetting.loginNotifyWebhook}
                  placeholder="https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=..."
                  onChange={(e) => updateSetting({ loginNotifyWebhook: e.target.value })}
                />
              </SettingListItem>
              <SettingListItem paddings="small" title={t('pages.settings.language')}>
                <Select
                  value={lang}
                  onChange={onLangChange}
                  style={{ width: '100%' }}
                  options={langOptions}
                />
              </SettingListItem>
              <PanelUpdateSection />
            </>
          ),
        },
        {
          key: '2',
          label: catTabLabel(
            <DeploymentUnitOutlined aria-hidden />,
            t('pages.settings.runtimeEnv'),
            isMobile,
          ),
          children: (
            <>
              <SettingListItem
                paddings="small"
                title={t('pages.settings.xrayBin')}
                description={t('pages.settings.xrayBinDesc')}
              >
                <Input
                  value={allSetting.xrayBin}
                  placeholder="/usr/local/bin/xray"
                  onChange={(e) => updateSetting({ xrayBin: e.target.value })}
                />
              </SettingListItem>
              <SettingListItem
                paddings="small"
                title={t('pages.settings.xrayConfig')}
                description={t('pages.settings.xrayConfigDesc')}
              >
                <Input
                  value={allSetting.xrayConfig}
                  placeholder="/usr/local/etc/xray/config.json"
                  onChange={(e) => updateSetting({ xrayConfig: e.target.value })}
                />
              </SettingListItem>
              <SettingListItem
                paddings="small"
                title={t('pages.settings.xrayApiAddr')}
                description={t('pages.settings.xrayApiAddrDesc')}
              >
                <Input
                  value={allSetting.xrayApiAddr}
                  placeholder="127.0.0.1:10085"
                  onChange={(e) => updateSetting({ xrayApiAddr: e.target.value })}
                />
              </SettingListItem>
              <SettingListItem
                paddings="small"
                title={t('pages.settings.xrayLogFile')}
                description={t('pages.settings.xrayLogFileDesc')}
              >
                <Input
                  value={allSetting.xrayLogFile}
                  placeholder="xray.log（默认与数据库同目录）"
                  onChange={(e) => updateSetting({ xrayLogFile: e.target.value })}
                />
              </SettingListItem>
              <SettingListItem
                paddings="small"
                title={t('pages.settings.updateProxy')}
                description={t('pages.settings.updateProxyDesc')}
              >
                <Input
                  value={allSetting.updateProxy}
                  placeholder="socks5://127.0.0.1:1080 或 http://host:port"
                  onChange={(e) => updateSetting({ updateProxy: e.target.value })}
                />
              </SettingListItem>
              <SettingListItem
                paddings="small"
                title={t('pages.settings.secureCookie')}
                description={t('pages.settings.secureCookieDesc')}
              >
                <Switch
                  checked={allSetting.secureCookie}
                  onChange={(v) => updateSetting({ secureCookie: v })}
                />
              </SettingListItem>
              <SettingListItem paddings="small" title={t('pages.settings.runtimeEnvHint')}>
                <span />
              </SettingListItem>
            </>
          ),
        },
        {
          key: '3',
          label: catTabLabel(
            <ClockCircleOutlined aria-hidden />,
            t('pages.settings.dateAndTime'),
            isMobile,
          ),
          children: (
            <SettingListItem
              paddings="small"
              title={t('pages.settings.datepicker')}
              description={t('pages.settings.datepickerDescription')}
            >
              <Select
                value={allSetting.datepicker || 'gregorian'}
                onChange={(v) => updateSetting({ datepicker: v as 'gregorian' | 'jalalian' })}
                style={{ width: '100%' }}
                options={[
                  { value: 'gregorian', label: t('pages.settings.calendarGregorian') },
                  { value: 'jalalian', label: t('pages.settings.calendarJalalian') },
                ]}
              />
            </SettingListItem>
          ),
        },
      ]}
    />
  );
}
