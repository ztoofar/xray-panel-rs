import { useTranslation } from 'react-i18next';
import { Alert, Input } from 'antd';

import { SettingListItem } from '@/components/ui';
import type { AllSetting } from '@/models/setting';

interface IpDataCloudTabProps {
  allSetting: AllSetting;
  updateSetting: (patch: Partial<AllSetting>) => void;
}

export default function IpDataCloudTab({
  allSetting,
  updateSetting,
}: IpDataCloudTabProps) {
  const { t } = useTranslation();
  const configured = allSetting.ipDataCloudKey.trim() !== '';

  return (
    <>
      <Alert
        type="info"
        showIcon
        className="mb-12"
        title={t('pages.settings.ipDataCloudHint')}
        description={
          <>
            {t('pages.settings.ipDataCloudHintDesc')}{' '}
            <a href="https://www.ipdatacloud.com/" target="_blank" rel="noreferrer">
              ipdatacloud.com
            </a>
          </>
        }
      />
      <SettingListItem
        paddings="small"
        title={t('pages.settings.ipDataCloudKey')}
        description={t('pages.settings.ipDataCloudKeyDesc')}
      >
        <Input.Password
          value={allSetting.ipDataCloudKey}
          placeholder={t('pages.settings.ipDataCloudKey')}
          autoComplete="off"
          onChange={(event) => updateSetting({ ipDataCloudKey: event.target.value })}
        />
      </SettingListItem>
      {!configured && (
        <Alert
          type="warning"
          showIcon
          className="mb-12"
          title={t('pages.settings.ipDataCloudNoKey')}
        />
      )}
    </>
  );
}
