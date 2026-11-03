import { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Input, Space, message } from 'antd';

import { HttpUtil } from '@/utils';
import { SettingListItem } from '@/components/ui';
import './SecurityTab.css';

interface ApiMsg {
  success?: boolean;
  msg?: string;
}

// Only admin credentials: the Rust panel implements no TOTP or API tokens.
export default function SecurityTab() {
  const { t } = useTranslation();
  const [messageApi, messageContextHolder] = message.useMessage();
  const [user, setUser] = useState({
    oldUsername: '',
    oldPassword: '',
    newUsername: '',
    newPassword: '',
  });
  const [updating, setUpdating] = useState(false);

  function updateUserField<K extends keyof typeof user>(key: K, value: string) {
    setUser((prev) => ({ ...prev, [key]: value }));
  }

  const sendUpdateUser = useCallback(async () => {
    setUpdating(true);
    try {
      const msg = (await HttpUtil.post('/panel/api/setting/updateUser', user)) as ApiMsg;
      if (msg?.success) {
        await HttpUtil.post('/logout');
        window.location.replace(window.X_UI_BASE_PATH || '/');
      } else if (msg?.msg) {
        messageApi.error(msg.msg);
      }
    } finally {
      setUpdating(false);
    }
  }, [user, messageApi]);

  return (
    <>
      {messageContextHolder}
      <SettingListItem paddings="small" title={t('pages.settings.oldUsername')}>
        <Input
          value={user.oldUsername}
          autoComplete="username"
          onChange={(e) => updateUserField('oldUsername', e.target.value)}
        />
      </SettingListItem>
      <SettingListItem paddings="small" title={t('pages.settings.currentPassword')}>
        <Input.Password
          value={user.oldPassword}
          autoComplete="current-password"
          onChange={(e) => updateUserField('oldPassword', e.target.value)}
        />
      </SettingListItem>
      <SettingListItem paddings="small" title={t('pages.settings.newUsername')}>
        <Input
          value={user.newUsername}
          onChange={(e) => updateUserField('newUsername', e.target.value)}
        />
      </SettingListItem>
      <SettingListItem paddings="small" title={t('pages.settings.newPassword')}>
        <Input.Password
          value={user.newPassword}
          autoComplete="new-password"
          onChange={(e) => updateUserField('newPassword', e.target.value)}
        />
      </SettingListItem>
      <div className="security-actions">
        <Space style={{ padding: '0 20px' }}>
          <Button type="primary" loading={updating} onClick={sendUpdateUser}>
            {t('confirm')}
          </Button>
        </Space>
      </div>
    </>
  );
}
