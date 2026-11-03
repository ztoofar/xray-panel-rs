import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Collapse, Empty, Modal, Spin, Tag } from 'antd';

import { HttpUtil } from '@/utils';
import { LinkTags, linkMetaText, parseLinkParts } from '@/lib/xray/link-label';
import { QrPanel } from '@/pages/inbounds/qr';
import type { ClientRecord } from '@/hooks/useClients';

interface ClientQrModalProps {
  open: boolean;
  client: ClientRecord | null;
  onOpenChange: (open: boolean) => void;
}

interface ApiMsg<T = unknown> {
  success?: boolean;
  obj?: T;
}

// QrPanel encodes at error level L; version 40 holds 2953 UTF-8 bytes.
const STANDARD_QR_MAX_BYTES = 2953;
const UTF8_ENCODER = new TextEncoder();

function canEncodeStandardQr(link: string) {
  return link.length > 0 && UTF8_ENCODER.encode(link).byteLength <= STANDARD_QR_MAX_BYTES;
}

export default function ClientQrModal({ open, client, onOpenChange }: ClientQrModalProps) {
  const { t } = useTranslation();
  const [links, setLinks] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!open || !client?.subId) return;
    let cancelled = false;
    // oxlint-disable-next-line react/set-state-in-effect
    setLoading(true);
    (async () => {
      try {
        const msg = (await HttpUtil.get(
          `/panel/api/clients/subLinks/${encodeURIComponent(client.subId!)}`,
        )) as ApiMsg<string[]>;
        if (!cancelled) {
          setLinks(msg?.success && Array.isArray(msg.obj) ? msg.obj : []);
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open, client?.subId]);

  const items = useMemo(() => {
    return links.map((link, index) => {
      const parts = parseLinkParts(link);
      const meta = parts ? linkMetaText(parts) : '';
      const label = parts ? (
        <span style={{ display: 'inline-flex', alignItems: 'center', gap: 6, flexWrap: 'wrap' }}>
          <LinkTags parts={parts} />
          {meta && <span style={{ opacity: 0.6, fontSize: 12 }}>({meta})</span>}
        </span>
      ) : (
        <span>
          {t('pages.clients.link')} {index + 1}
        </span>
      );
      return {
        key: `link-${index}`,
        label,
        children: (
          <QrPanel
            value={link}
            remark={parts?.remark || `${client?.email || ''} #${index + 1}`}
            showQr={canEncodeStandardQr(link)}
          />
        ),
      };
    });
  }, [links, client?.email, t]);

  return (
    <Modal
      open={open}
      title={client ? `${client.email} · ${t('pages.clients.qrCode')}` : t('pages.clients.qrCode')}
      footer={null}
      width={640}
      onCancel={() => onOpenChange(false)}
      destroyOnHidden
    >
      {loading ? (
        <Spin />
      ) : items.length === 0 ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description={t('pages.clients.noLinks')}
        />
      ) : (
        <Collapse
          accordion
          defaultActiveKey={items[0] ? [items[0].key] : []}
          items={items}
        />
      )}
      {client?.subId && (
        <Tag color="blue" style={{ marginTop: 12 }}>
          {t('pages.clients.subId')}: {client.subId}
        </Tag>
      )}
    </Modal>
  );
}
