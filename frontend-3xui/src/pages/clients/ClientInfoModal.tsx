import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Modal, Tag } from 'antd';

import { HttpUtil, IntlUtil, SizeFormatter } from '@/utils';
import { formatInboundLabel } from '@/lib/inbounds/label';
import { useDatepicker } from '@/hooks/useDatepicker';
import type { ClientRecord, InboundOption } from '@/hooks/useClients';
import type { HostRecord } from '@/schemas/api/host';
import './ClientInfoModal.css';

const INBOUND_PROTOCOL_COLORS: Record<string, string> = {
  vless: 'blue',
  vmess: 'geekblue',
  trojan: 'volcano',
  shadowsocks: 'magenta',
};

interface ClientInfoModalProps {
  open: boolean;
  client: ClientRecord | null;
  inboundsById: Record<number, InboundOption>;
  tunnelAllowedIPs?: Record<number, string>;
  isOnline: boolean;
  hosts?: HostRecord[];
  onOpenChange: (open: boolean) => void;
}

// Mirrors the simplified create form: email, inbounds, traffic, expiry, UUID,
// status, plus the record timestamps.
export default function ClientInfoModal({
  open,
  client,
  inboundsById,
  isOnline,
  onOpenChange,
}: ClientInfoModalProps) {
  const { t } = useTranslation();
  const { datepicker } = useDatepicker();
  const dateLabel = (ts?: number) => (!ts || ts <= 0 ? '-' : IntlUtil.formatDate(ts, datepicker));

  const [lastOnlineMap, setLastOnlineMap] = useState<Record<string, number>>({});
  const [onlineIps, setOnlineIps] = useState<string[]>([]);
  const [locations, setLocations] = useState<
    Record<string, { loading?: boolean; text?: string; error?: string }>
  >({});

  useEffect(() => {
    if (!open || !isOnline || !client?.email) {
      setOnlineIps([]);
      return;
    }
    let cancelled = false;
    (async () => {
      const msg = (await HttpUtil.post<Record<string, string[]>>(
        '/panel/api/clients/onlineIps',
        undefined,
        { silent: true },
      )) as { success?: boolean; obj?: Record<string, string[]> };
      if (!cancelled && msg?.success && msg.obj) {
        setOnlineIps(msg.obj[client.email] || []);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open, isOnline, client?.email]);

  async function lookupLocation(ip: string) {
    setLocations((prev) => ({ ...prev, [ip]: { loading: true } }));
    const msg = (await HttpUtil.post<Record<string, unknown>>(
      '/panel/api/clients/ipLocation',
      { ip },
      { silent: true },
    )) as {
      success?: boolean;
      msg?: string;
      obj?: { country?: string; region?: string; city?: string; isp?: string };
    };
    if (msg?.success && msg.obj) {
      const object = msg.obj;
      const text =
        [object.country, object.region, object.city, object.isp]
          .filter((part): part is string => typeof part === 'string' && part.length > 0)
          .join(' · ') || '-';
      setLocations((prev) => ({ ...prev, [ip]: { text } }));
    } else {
      setLocations((prev) => ({
        ...prev,
        [ip]: { error: msg?.msg || t('somethingWentWrong') },
      }));
    }
  }

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    (async () => {
      const msg = (await HttpUtil.post<Record<string, number>>(
        '/panel/api/clients/lastOnline',
        undefined,
        { silent: true },
      )) as { success?: boolean; obj?: Record<string, number> };
      if (!cancelled && msg?.success && msg.obj && typeof msg.obj === 'object') {
        setLastOnlineMap(msg.obj);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open]);

  const traffic = client?.traffic;
  const used = (traffic?.up || 0) + (traffic?.down || 0);
  const totalBytes = client?.totalGB || 0;
  const remaining = totalBytes > 0 ? Math.max(0, totalBytes - used) : -1;
  const expiryTime = client?.expiryTime ?? 0;
  const expiryLabel = !expiryTime
    ? '∞'
    : expiryTime < 0
      ? `${t('pages.clients.delayedStart')}: ${Math.round(expiryTime / -86400000)}d`
      : IntlUtil.formatDate(expiryTime, datepicker);

  return (
    <Modal
      open={open}
      title={
        client
          ? `${t('pages.clients.clientInfo')} — ${client.email}`
          : t('pages.clients.clientInfo')
      }
      footer={null}
      width={560}
      onCancel={() => onOpenChange(false)}
    >
      {client && (
        <table className="info-table block">
          <tbody>
            <tr>
              <td>{t('pages.clients.online')}</td>
              <td>
                {client.enable && isOnline ? (
                  <Tag color="green">{t('pages.clients.online')}</Tag>
                ) : (
                  <Tag>{t('pages.clients.offline')}</Tag>
                )}
              </td>
            </tr>
            {isOnline && (
              <tr>
                <td>{t('pages.clients.currentIp')}</td>
                <td>
                  {onlineIps.length === 0 ? (
                    <span className="hint">{t('pages.clients.ipLocationNoIp')}</span>
                  ) : (
                    <div className="ip-list">
                      {onlineIps.map((ip) => {
                        const location = locations[ip];
                        return (
                          <div className="ip-item" key={ip}>
                            <Tag>{ip}</Tag>
                            <Button
                              size="small"
                              type="link"
                              loading={location?.loading}
                              onClick={() => lookupLocation(ip)}
                            >
                              {t('pages.clients.ipLocation')}
                            </Button>
                            {location?.text && <span className="hint">{location.text}</span>}
                            {location?.error && (
                              <span className="hint ip-location-error">{location.error}</span>
                            )}
                          </div>
                        );
                      })}
                    </div>
                  )}
                </td>
              </tr>
            )}
            <tr>
              <td>{t('status')}</td>
              <td>
                <Tag color={client.enable ? 'green' : 'default'}>
                  {client.enable ? t('enabled') : t('disabled')}
                </Tag>
              </td>
            </tr>
            <tr>
              <td>{t('pages.clients.email')}</td>
              <td>
                <Tag color="green">{client.email}</Tag>
              </td>
            </tr>
            <tr>
              <td>{t('pages.clients.attachedInbounds')}</td>
              <td>
                {(client.inboundIds || []).length === 0 ? (
                  <span className="hint">—</span>
                ) : (
                  <div className="chips">
                    {(client.inboundIds || []).map((id) => {
                      const inbound = inboundsById[id];
                      const protocol = (inbound?.protocol || '').toLowerCase();
                      return (
                        <Tag key={id} color={INBOUND_PROTOCOL_COLORS[protocol] ?? 'default'}>
                          {formatInboundLabel(inbound?.tag, inbound?.remark)}
                        </Tag>
                      );
                    })}
                  </div>
                )}
              </td>
            </tr>
            <tr>
              <td>{t('pages.inbounds.traffic')}</td>
              <td>
                <Tag>
                  ↑ {SizeFormatter.sizeFormat(traffic?.up || 0)} / ↓{' '}
                  {SizeFormatter.sizeFormat(traffic?.down || 0)}
                </Tag>
                <span className="hint">
                  {SizeFormatter.sizeFormat(used)} /{' '}
                  {totalBytes > 0 ? SizeFormatter.sizeFormat(totalBytes) : '∞'}
                </span>
              </td>
            </tr>
            <tr>
              <td>{t('remained')}</td>
              <td>
                {remaining < 0 ? (
                  <Tag color="purple">∞</Tag>
                ) : (
                  <Tag color={remaining > 0 ? '' : 'red'}>
                    {SizeFormatter.sizeFormat(remaining)}
                  </Tag>
                )}
              </td>
            </tr>
            <tr>
              <td>{t('pages.inbounds.expireDate')}</td>
              <td>
                <Tag color={expiryTime === 0 ? 'purple' : undefined}>{expiryLabel}</Tag>
                {expiryTime > 0 && (
                  <span className="hint">{IntlUtil.formatRelativeTime(expiryTime)}</span>
                )}
              </td>
            </tr>
            <tr>
              <td>{t('pages.clients.uuid')}</td>
              <td>
                <Tag className="info-large-tag">{client.uuid || '-'}</Tag>
              </td>
            </tr>
            <tr>
              <td>{t('lastOnline')}</td>
              <td>
                {isOnline && <Tag color="green">{t('pages.clients.online')}</Tag>}
                <Tag>{dateLabel(lastOnlineMap[client.email])}</Tag>
              </td>
            </tr>
            <tr>
              <td>{t('pages.inbounds.createdAt')}</td>
              <td>
                <Tag>{dateLabel(client.createdAt)}</Tag>
              </td>
            </tr>
            <tr>
              <td>{t('pages.inbounds.updatedAt')}</td>
              <td>
                <Tag>{dateLabel(client.updatedAt)}</Tag>
              </td>
            </tr>
          </tbody>
        </table>
      )}
    </Modal>
  );
}
