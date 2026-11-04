import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Checkbox, Form, Modal, Select, Space } from 'antd';
import { DownloadOutlined, SyncOutlined } from '@ant-design/icons';

import { HttpUtil, FileManager } from '@/utils';
import { activateOnKey } from '@/utils/a11y';
import { useMediaQuery } from '@/hooks/useMediaQuery';
import './LogModal.css';

interface LogModalProps {
  open: boolean;
  onClose: () => void;
}

// One pane, three sources: the panel's own tracing output, or one of the two
// Xray log files configured on the Xray page.
type LogView = 'panel' | 'error' | 'access';
type XrayFile = 'error' | 'access';

interface XrayLogPayload {
  kind: XrayFile;
  configured: boolean;
  path: string | null;
  exists: boolean;
  fallback: boolean;
  lines: string[];
}

interface PanelLogFile {
  name: string;
  date: string;
  size: number;
}

const AUTO_UPDATE_INTERVAL = 5000;
const NO_LINES: string[] = [];

export default function LogModal({ open, onClose }: LogModalProps) {
  const { t } = useTranslation();
  const { isMobile } = useMediaQuery();
  const [view, setView] = useState<LogView>('panel');
  const [rows, setRows] = useState('100');
  const [autoUpdate, setAutoUpdate] = useState(false);
  const [loading, setLoading] = useState(false);
  const [panelLines, setPanelLines] = useState<string[]>([]);
  const [panelFile, setPanelFile] = useState('');
  const [panelFiles, setPanelFiles] = useState<PanelLogFile[]>([]);
  const [xrayLog, setXrayLog] = useState<XrayLogPayload | null>(null);
  const source = view === 'panel' ? 'panel' : 'xray';
  const xrayFile: XrayFile = view === 'access' ? 'access' : 'error';

  const runRefresh = useCallback(async () => {
    try {
      if (view === 'panel') {
        const msg = await HttpUtil.post<string[]>(`/panel/api/server/logs/${rows}`, {
          source: 'panel',
          ...(panelFile ? { file: panelFile } : {}),
        });
        if (msg?.success) setPanelLines(msg.obj || []);
      } else {
        const msg = await HttpUtil.post<XrayLogPayload>(`/panel/api/server/logs/${rows}`, {
          source: 'xray',
          file: view,
        });
        if (msg?.success) setXrayLog(msg.obj || null);
      }
    } finally {
      setLoading(false);
    }
  }, [view, rows, panelFile]);

  const fetchPanelFiles = useCallback(async () => {
    const msg = await HttpUtil.get<PanelLogFile[]>(
      '/panel/api/server/panelLogFiles',
      undefined,
      { silent: true },
    );
    if (msg?.success) setPanelFiles(msg.obj || []);
  }, []);

  useEffect(() => {
    if (open && view === 'panel') void fetchPanelFiles();
  }, [open, view, fetchPanelFiles]);

  const refresh = useCallback(() => {
    setLoading(true);
    void runRefresh();
  }, [runRefresh]);

  const refreshRef = useRef(refresh);
  useEffect(() => {
    refreshRef.current = refresh;
  });

  // The spinner is raised during render so the fetch effect stays side-effect
  // free until its response lands.
  const refreshKey = open ? `${view}\u0000${rows}\u0000${panelFile}` : null;
  const [loadingKey, setLoadingKey] = useState<string | null>(null);
  if (refreshKey !== loadingKey) {
    setLoadingKey(refreshKey);
    if (refreshKey) setLoading(true);
  }

  useEffect(() => {
    if (open) void runRefresh();
  }, [open, runRefresh]);

  useEffect(() => {
    if (!open || !autoUpdate || panelFile) return;
    const id = setInterval(() => refreshRef.current(), AUTO_UPDATE_INTERVAL);
    return () => clearInterval(id);
  }, [open, autoUpdate, panelFile]);

  const lines = source === 'panel' ? panelLines : (xrayLog?.lines ?? NO_LINES);
  // Newest first: the log is polled, so the top of the pane should show the
  // line that just arrived instead of making the user scroll to the bottom.
  const visibleLines = useMemo(() => [...lines].reverse(), [lines]);

  const hint = useMemo(() => {
    if (source === 'panel' && panelFile) return t('pages.index.panelLogHistoryDesc');
    if (source !== 'xray' || !xrayLog) return '';
    if (xrayLog.fallback) return t('pages.index.logProcessFallback');
    if (!xrayLog.configured) {
      const name = xrayFile === 'error' ? t('pages.xray.errorLog') : t('pages.xray.accessLog');
      return t('pages.index.logNotConfigured', { name });
    }
    if (!xrayLog.exists) {
      return `${xrayLog.path ?? ''} — ${t('pages.index.logFileMissing')}`;
    }
    return xrayLog.path ?? '';
  }, [source, panelFile, xrayLog, xrayFile, t]);

  function download() {
    const name = source === 'panel' ? 'panel.log' : `${xrayFile}.log`;
    FileManager.downloadTextFile(lines.join('\n'), name);
  }

  const titleNode = (
    <>
      {t('pages.index.logs')}
      <SyncOutlined
        spin={loading}
        className="reload-icon"
        role="button"
        tabIndex={0}
        aria-label={t('refresh')}
        onClick={refresh}
        onKeyDown={activateOnKey(refresh)}
      />
    </>
  );

  return (
    <Modal
      open={open}
      footer={null}
      width={isMobile ? '100vw' : 800}
      style={isMobile ? { top: 0, paddingBottom: 0, maxWidth: '100vw' } : undefined}
      className={isMobile ? 'logmodal-mobile' : undefined}
      onCancel={onClose}
      title={titleNode}
    >
      <Form layout="inline" className="log-toolbar">
        <Form.Item>
          <Select
            aria-label={t('pages.index.logs')}
            value={view}
            size="small"
            style={{ width: 190 }}
            onChange={(value) => setView(value as LogView)}
            options={[
              { value: 'panel', label: t('pages.index.panelLogs') },
              { value: 'error', label: `${t('pages.index.xrayLogs')} · ${t('pages.xray.errorLog')}` },
              { value: 'access', label: `${t('pages.index.xrayLogs')} · ${t('pages.xray.accessLog')}` },
            ]}
          />
        </Form.Item>
        {source === 'panel' && (
          <Form.Item>
            <Select
              aria-label={t('pages.index.panelLogHistory')}
              value={panelFile}
              size="small"
              style={{ width: 150 }}
              onChange={(value) => {
                setPanelFile(value);
                if (value) setAutoUpdate(false);
              }}
              options={[
                { value: '', label: t('pages.index.panelLogRealtime') },
                ...panelFiles.map((file) => ({ value: file.name, label: file.date })),
              ]}
            />
          </Form.Item>
        )}
        <Form.Item>
          <Space.Compact>
            <Select
              aria-label={t('pages.index.logLines')}
              value={rows}
              size="small"
              style={{ width: 100 }}
              onChange={setRows}
              options={[
                { value: '20', label: '20' },
                { value: '50', label: '50' },
                { value: '100', label: '100' },
                { value: '500', label: '500' },
                { value: '1000', label: '1000' },
              ]}
            />
          </Space.Compact>
        </Form.Item>
        <Form.Item>
          <Checkbox checked={autoUpdate} onChange={(e) => setAutoUpdate(e.target.checked)}>
            {t('pages.index.autoUpdate')}
          </Checkbox>
        </Form.Item>
        <Form.Item className="download-item">
          <Button
            type="primary"
            onClick={download}
            icon={<DownloadOutlined aria-hidden />}
            aria-label={t('download')}
          />
        </Form.Item>
      </Form>

      {hint && <div className="log-hint">{hint}</div>}

      <div className={`log-container ${isMobile ? 'log-container-mobile' : ''}`}>
        {visibleLines.length === 0 ? (
          <div className="log-empty">No Record...</div>
        ) : (
          visibleLines.map((line, idx) => (
            <div key={idx} className="log-line log-line-raw">
              {line}
            </div>
          ))
        )}
      </div>
    </Modal>
  );
}
