import { useEffect } from 'react';
import { getSharedWebSocketClient } from '@/api/websocket';

type Handler = (payload: unknown) => void;

const REALTIME_ENABLED = false;

export function useWebSocket(handlers: Record<string, Handler>) {
  useEffect(() => {
    if (!REALTIME_ENABLED) return;
    const client = getSharedWebSocketClient();
    const entries = Object.entries(handlers);
    for (const [event, fn] of entries) client.on(event, fn);
    client.connect();
    return () => {
      for (const [event, fn] of entries) client.off(event, fn);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
}
