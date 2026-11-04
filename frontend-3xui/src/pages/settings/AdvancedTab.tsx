import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { Alert, Input, InputNumber, Select, Space, Switch, Tag } from 'antd';
import { useQuery } from '@tanstack/react-query';

import { HttpUtil } from '@/utils';
import { onNumber } from '@/utils/onNumber';
import { SettingListItem } from '@/components/ui';
import { fetchXrayConfig } from '@/hooks/useXraySetting';
import { keys } from '@/api/queryKeys';
import type { AllSetting } from '@/models/setting';

interface AdvancedTabProps {
  allSetting: AllSetting;
  updateSetting: (patch: Partial<AllSetting>) => void;
}

interface CheckResult {
  tag: string;
  success: boolean;
  delay?: number;
  error?: string;
}

interface CheckStatus {
  checkedAt?: number;
  results?: CheckResult[];
}

interface OutboundLike {
  tag?: string;
  protocol?: string;
}

export default function AdvancedTab({ allSetting, updateSetting }: AdvancedTabProps) {
  const { t } = useTranslation();

  const configQuery = useQuery({
    queryKey: keys.xray.config(),
    queryFn: fetchXrayConfig,
    staleTime: Infinity,
  });

  const outboundOptions = useMemo(() => {
    const seen = new Map<string, string>();
    const add = (outbound: unknown) => {
      const item = outbound as OutboundLike | null;
      const tag = item?.tag?.trim();
      if (!tag || tag.startsWith('__') || seen.has(tag)) return;
      seen.set(tag, item?.protocol || '');
    };
    for (const outbound of configQuery.data?.xraySetting?.outbounds || []) add(outbound);
    for (const outbound of configQuery.data?.subscriptionOutbounds || []) add(outbound);
    return [...seen.entries()].map(([tag, protocol]) => ({
      value: tag,
      label: protocol ? `${tag} (${protocol})` : tag,
    }));
  }, [configQuery.data]);

  const selectedTags = useMemo(
    () =>
      allSetting.outboundCheckTags
        .split(',')
        .map((tag) => tag.trim())
        .filter(Boolean),
    [allSetting.outboundCheckTags],
  );

  const statusQuery = useQuery({
    queryKey: ['xray', 'outboundCheckStatus'],
    queryFn: async (): Promise<CheckStatus | null> => {
      const msg = await HttpUtil.get('/panel/api/xray/outboundCheckStatus', undefined, {
        silent: true,
      });
      return msg?.success ? (msg.obj as CheckStatus) : null;
    },
    enabled: allSetting.outboundCheckEnabled,
    refetchInterval: allSetting.outboundCheckEnabled ? 30000 : false,
  });

  const status = statusQuery.data;
  const checkedAt = status?.checkedAt
    ? new Date(status.checkedAt * 1000).toLocaleString()
    : t('pages.settings.outboundCheckNeverRun');

  return (
    <>
      <Alert
        type="info"
        showIcon
        className="mb-12"
        title={t('pages.settings.outboundCheckHint')}
        description={t('pages.settings.outboundCheckHintDesc')}
      />
      <SettingListItem
        paddings="small"
        title={t('pages.settings.outboundCheckEnabled')}
        description={t('pages.settings.outboundCheckEnabledDesc')}
      >
        <Switch
          checked={allSetting.outboundCheckEnabled}
          onChange={(value) => updateSetting({ outboundCheckEnabled: value })}
        />
      </SettingListItem>
      <SettingListItem
        paddings="small"
        title={t('pages.settings.outboundCheckTags')}
        description={t('pages.settings.outboundCheckTagsDesc')}
      >
        <Select
          mode="multiple"
          allowClear
          disabled={!allSetting.outboundCheckEnabled}
          loading={configQuery.isLoading}
          value={selectedTags}
          placeholder={t('pages.settings.outboundCheckTagsPlaceholder')}
          options={outboundOptions}
          style={{ width: '100%' }}
          onChange={(values: string[]) =>
            updateSetting({ outboundCheckTags: values.join(',') })
          }
        />
      </SettingListItem>
      <SettingListItem
        paddings="small"
        title={t('pages.settings.outboundCheckInterval')}
        description={t('pages.settings.outboundCheckIntervalDesc')}
      >
        <InputNumber
          value={allSetting.outboundCheckIntervalMinutes}
          min={1}
          max={1440}
          disabled={!allSetting.outboundCheckEnabled}
          addonAfter={t('pages.settings.minutes')}
          style={{ width: '100%' }}
          onChange={onNumber((value) => updateSetting({ outboundCheckIntervalMinutes: value }))}
        />
      </SettingListItem>
      <SettingListItem
        paddings="small"
        title={t('pages.settings.outboundCheckWebhook')}
        description={t('pages.settings.outboundCheckWebhookDesc')}
      >
        <Input
          value={allSetting.loginNotifyWebhook}
          placeholder="https://qyapi.weixin.qq.com/cgi-bin/webhook/send?key=..."
          onChange={(e) => updateSetting({ loginNotifyWebhook: e.target.value })}
        />
      </SettingListItem>
      {allSetting.outboundCheckEnabled && !allSetting.loginNotifyWebhook.trim() && (
        <Alert
          type="warning"
          showIcon
          className="mb-12"
          title={t('pages.settings.outboundCheckNoWebhook')}
        />
      )}
      <SettingListItem
        paddings="small"
        title={t('pages.settings.outboundCheckLastResult')}
        description={checkedAt}
      >
        {status?.results?.length ? (
          <Space size={[4, 4]} wrap>
            {status.results.map((result) => (
              <Tag key={result.tag} color={result.success ? 'green' : 'red'}>
                {result.tag}
                {result.success
                  ? result.delay != null
                    ? ` · ${result.delay} ms`
                    : ''
                  : result.error
                    ? ` · ${result.error}`
                    : ''}
              </Tag>
            ))}
          </Space>
        ) : (
          <span>{t('pages.settings.outboundCheckNeverRun')}</span>
        )}
      </SettingListItem>
    </>
  );
}
