import { useEffect, useMemo, useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Form, Input, InputNumber, Modal, Select, Switch, message } from 'antd';
import dayjs from 'dayjs';
import type { Dayjs } from 'dayjs';

import { RandomUtil } from '@/utils';
import { formatInboundLabel } from '@/lib/inbounds/label';
import { DateTimePicker } from '@/components/form';
import type { ClientRecord, InboundOption } from '@/hooks/useClients';
import './ClientFormModal.css';

const MULTI_CLIENT_PROTOCOLS = new Set([
  'shadowsocks',
  'vless',
  'vmess',
  'trojan',
  'hysteria',
  'wireguard',
  'amneziawg',
  'mtproto',
  'tuic',
  'masque',
]);

interface ApiMsg<T = unknown> {
  success?: boolean;
  msg?: string;
  obj?: T;
}

type Mode = 'add' | 'edit';

interface SaveMetaEdit {
  isEdit: true;
  email: string;
  attach: number[];
  detach: number[];
}

interface SaveMetaCreate {
  isEdit: false;
  email: string;
}

interface SaveCreatePayload {
  client: Record<string, unknown>;
  inboundIds: number[];
}

interface ClientFormModalProps {
  open: boolean;
  mode: Mode;
  client: ClientRecord | null;
  inbounds: InboundOption[];
  attachedIds?: number[];
  tunnelAllowedIPs?: Record<number, string>;
  tgBotEnable?: boolean;
  groups?: string[];
  save: (
    payload: Record<string, unknown> | SaveCreatePayload,
    meta: SaveMetaEdit | SaveMetaCreate,
  ) => Promise<ApiMsg | null>;
  resetTraffic?: (client: ClientRecord) => Promise<ApiMsg | null>;
  onOpenChange: (open: boolean) => void;
}

function bytesToGB(bytes: number): number {
  if (!bytes || bytes <= 0) return 0;
  return Math.round((bytes / (1024 * 1024 * 1024)) * 100) / 100;
}

function gbToBytes(gb: number): number {
  if (!gb || gb <= 0) return 0;
  return Math.round(gb * 1024 * 1024 * 1024);
}

// Deliberately minimal: only the fields the panel's create/update API needs.
// Unshared fields keep neutral defaults so edits never clear stored values.
export default function ClientFormModal({
  open,
  mode,
  client,
  inbounds,
  attachedIds = [],
  save,
  onOpenChange,
}: ClientFormModalProps) {
  const { t } = useTranslation();
  const uid = useId();
  const [messageApi, messageContextHolder] = message.useMessage();
  const isEdit = mode === 'edit';

  const [email, setEmail] = useState('');
  const [uuid, setUuid] = useState('');
  const [totalGB, setTotalGB] = useState(0);
  const [enable, setEnable] = useState(true);
  const [inboundIds, setInboundIds] = useState<number[]>([]);
  const [expiryDate, setExpiryDate] = useState(0);
  const [expiryTouched, setExpiryTouched] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    if (mode === 'edit' && client) {
      setEmail(client.email || '');
      setUuid(client.uuid || '');
      setTotalGB(bytesToGB(client.totalGB || 0));
      setEnable(!!client.enable);
      setInboundIds([...attachedIds]);
      setExpiryDate(Math.max(0, Number(client.expiryTime) || 0));
      setExpiryTouched(false);
    } else {
      setEmail(RandomUtil.randomLowerAndNum(10));
      setUuid(RandomUtil.randomUUID());
      setTotalGB(0);
      setEnable(true);
      setInboundIds([]);
      setExpiryDate(0);
      setExpiryTouched(false);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, mode]);

  const inboundOptions = useMemo(
    () =>
      (inbounds || [])
        .filter((ib) => MULTI_CLIENT_PROTOCOLS.has(ib.protocol || ''))
        .filter((ib) => ib.enable || attachedIds.includes(ib.id))
        .map((ib) => ({
          label: formatInboundLabel(ib.tag, ib.remark),
          value: ib.id,
          title: formatInboundLabel(ib.tag, ib.remark),
        })),
    [inbounds, attachedIds],
  );

  const expiryDayjs = useMemo<Dayjs | null>(
    () => (expiryDate > 0 ? dayjs(expiryDate) : null),
    [expiryDate],
  );

  function close() {
    onOpenChange(false);
  }

  async function onSubmit() {
    const trimmed = email.trim();
    if (!trimmed) {
      messageApi.error(t('pages.clients.email'));
      return;
    }
    if (inboundIds.length === 0) {
      messageApi.error(t('pages.clients.selectInbound'));
      return;
    }
    const originalExpiry = isEdit && client ? Number(client.expiryTime) || 0 : 0;
    const expiration = expiryTouched ? expiryDate : originalExpiry;
    const payload: Record<string, unknown> = {
      email: trimmed,
      uuid,
      id: uuid,
      subId: '',
      password: '',
      auth: '',
      flow: '',
      security: 'auto',
      clientHostRuleIds: [],
      reverseTag: '',
      totalGB: gbToBytes(totalGB),
      expiryTime: expiration,
      delayedStart: false,
      delayedDays: 0,
      reset: 0,
      resetDay: 0,
      resetWeekday: 0,
      resetMax: 0,
      trafficReset: 'never',
      trafficResetDay: 1,
      limitIp: 0,
      limitHwid: 0,
      tgId: 0,
      group: '',
      comment: '',
      enable,
    };
    setSaving(true);
    try {
      let msg: ApiMsg | null;
      if (isEdit && client) {
        const original = new Set(attachedIds);
        const next = new Set(inboundIds);
        msg = await save(payload, {
          isEdit: true,
          email: client.email,
          attach: [...next].filter((id) => !original.has(id)),
          detach: [...original].filter((id) => !next.has(id)),
        });
      } else {
        msg = await save(
          { client: payload, inboundIds },
          { isEdit: false, email: trimmed },
        );
      }
      if (msg?.success) close();
      else if (msg?.msg) messageApi.error(msg.msg);
    } finally {
      setSaving(false);
    }
  }

  return (
    <>
      {messageContextHolder}
      <Modal
        open={open}
        title={isEdit ? t('pages.clients.editClient') : t('pages.clients.addClient')}
        destroyOnHidden
        width={560}
        onCancel={close}
        footer={
          <div style={{ display: 'flex', justifyContent: 'flex-end', gap: 8 }}>
            <Button onClick={close}>{t('cancel')}</Button>
            <Button type="primary" loading={saving} onClick={onSubmit}>
              {isEdit ? t('save') : t('create')}
            </Button>
          </div>
        }
      >
        <Form layout="vertical">
          <Form.Item label={t('pages.clients.email')} required htmlFor={`${uid}-email`}>
            <Input
              id={`${uid}-email`}
              value={email}
              placeholder={t('pages.clients.email')}
              onChange={(e) => setEmail(e.target.value)}
            />
          </Form.Item>
          <Form.Item
            label={t('pages.clients.attachedInbounds')}
            required
            htmlFor={`${uid}-inbounds`}
          >
            <Select
              id={`${uid}-inbounds`}
              mode="multiple"
              value={inboundIds}
              onChange={(v) => setInboundIds(v)}
              options={inboundOptions}
              placeholder={t('pages.clients.selectInbound')}
              maxTagCount="responsive"
              placement="topLeft"
              listHeight={220}
              showSearch={{
                filterOption: (input, option) =>
                  ((option?.label as string) || '')
                    .toLowerCase()
                    .includes(input.toLowerCase()),
              }}
            />
          </Form.Item>
          <Form.Item label={t('pages.clients.totalGB')} htmlFor={`${uid}-total`}>
            <InputNumber
              id={`${uid}-total`}
              min={0}
              step={1}
              style={{ width: '100%' }}
              value={totalGB}
              onChange={(v) => setTotalGB(Number(v) || 0)}
            />
          </Form.Item>
          <Form.Item label={t('pages.clients.uuid')} htmlFor={`${uid}-uuid`}>
            <Input
              id={`${uid}-uuid`}
              value={uuid}
              onChange={(e) => setUuid(e.target.value)}
            />
          </Form.Item>
          <Form.Item label={t('pages.clients.expiryTime')} htmlFor={`${uid}-expiry`}>
            <DateTimePicker
              id={`${uid}-expiry`}
              value={expiryDayjs}
              onChange={(d) => {
                setExpiryTouched(true);
                setExpiryDate(d ? d.valueOf() : 0);
              }}
            />
          </Form.Item>
          <Form.Item label={t('enable')} htmlFor={`${uid}-enable`}>
            <Switch
              id={`${uid}-enable`}
              checked={enable}
              onChange={(v) => setEnable(v)}
            />
          </Form.Item>
        </Form>
      </Modal>
    </>
  );
}
