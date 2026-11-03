import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Modal, Space, Tag, Typography, message } from 'antd';
import { CloudDownloadOutlined, ReloadOutlined } from '@ant-design/icons';

import { HttpUtil } from '@/utils';
import { SettingListItem } from '@/components/ui';

interface ApiMsg<T> {
  success?: boolean;
  msg?: string;
  obj?: T;
}

interface PanelUpdateInfo {
  current?: string;
  latest?: string;
  tag?: string;
  updateAvailable?: boolean;
  releaseUrl?: string;
  publishedAt?: string;
  restarting?: boolean;
  restartRequired?: boolean;
}

export default function PanelUpdateSection() {
  const { t } = useTranslation();
  const [messageApi, messageContextHolder] = message.useMessage();
  const [modal, modalContextHolder] = Modal.useModal();
  const [info, setInfo] = useState<PanelUpdateInfo | null>(null);
  const [checking, setChecking] = useState(false);
  const [updating, setUpdating] = useState(false);

  const check = useCallback(async () => {
    setChecking(true);
    try {
      const msg = (await HttpUtil.get(
        '/panel/api/server/panelUpdateCheck',
      )) as ApiMsg<PanelUpdateInfo>;
      if (msg?.success && msg.obj) setInfo(msg.obj);
      else messageApi.error(msg?.msg || t('somethingWentWrong'));
    } catch {
      messageApi.error(t('pages.settings.panelUpdateCheckFailed'));
    } finally {
      setChecking(false);
    }
  }, [messageApi, t]);

  useEffect(() => {
    // oxlint-disable-next-line react/set-state-in-effect
    void check();
  }, [check]);

  const runUpdate = () => {
    modal.confirm({
      title: t('pages.settings.panelUpdateConfirmTitle'),
      content: t('pages.settings.panelUpdateConfirmDesc'),
      okText: t('pages.settings.panelUpdateNow'),
      cancelText: t('cancel'),
      onOk: async () => {
        setUpdating(true);
        try {
          const msg = (await HttpUtil.post(
            '/panel/api/server/panelUpdate',
          )) as ApiMsg<PanelUpdateInfo>;
          if (!msg?.success) {
            messageApi.error(msg?.msg || t('somethingWentWrong'));
            return;
          }
          if (msg.obj?.restarting) {
            messageApi.success(t('pages.settings.panelUpdateRestarting'));
            window.setTimeout(() => window.location.reload(), 8000);
          } else {
            messageApi.warning(t('pages.settings.panelUpdateRestartRequired'));
          }
        } catch {
          messageApi.error(t('pages.settings.panelUpdateFailed'));
        } finally {
          setUpdating(false);
        }
      },
    });
  };

  return (
    <>
      {messageContextHolder}
      {modalContextHolder}
      <SettingListItem
        paddings="small"
        title={t('pages.settings.panelUpdate')}
        description={t('pages.settings.panelUpdateDesc')}
      >
        <Space orientation="vertical" size="small" style={{ width: '100%' }}>
          <Space wrap>
            <Tag>
              {t('pages.settings.panelUpdateCurrent')}: {info?.current || '—'}
            </Tag>
            {info &&
              (info.updateAvailable ? (
                <Tag color="orange">
                  {t('pages.settings.panelUpdateLatest')}: {info.latest}
                </Tag>
              ) : (
                <Tag color="green">{t('pages.settings.panelUpToDate')}</Tag>
              ))}
            <Button
              size="small"
              icon={<ReloadOutlined aria-hidden />}
              loading={checking}
              onClick={() => void check()}
            >
              {t('pages.settings.checkUpdate')}
            </Button>
            {info?.updateAvailable && (
              <Button
                size="small"
                type="primary"
                icon={<CloudDownloadOutlined aria-hidden />}
                loading={updating}
                onClick={runUpdate}
              >
                {t('pages.settings.panelUpdateNow')}
              </Button>
            )}
          </Space>
          {info?.releaseUrl && (
            <Typography.Link href={info.releaseUrl} target="_blank" rel="noreferrer">
              {t('pages.settings.panelReleaseNotes')}
            </Typography.Link>
          )}
        </Space>
      </SettingListItem>
    </>
  );
}
