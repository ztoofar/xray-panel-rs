import { useEffect } from 'react';
import { getSharedWebSocketClient } from '@/api/websocket';

type Handler = (payload: unknown) => void;

// The Rust backend now serves the 3x-ui `/ws` endpoint and pushes the same
// `traffic` frames; live speed no longer waits for a REST polling phase.
const REALTIME_ENABLED = true;

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
