import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { HttpUtil } from '@/utils';
import { parseMsg } from '@/utils/zodValidate';
import { DefaultsPayloadSchema, type DefaultsPayload } from '@/schemas/defaults';
import { keys } from '@/api/queryKeys';

const DEFAULT_SECS = 5;
const MIN_SECS = 2;
const MAX_SECS = 300;

function normalizeSecs(value: unknown): number {
  const raw = Number(value ?? DEFAULT_SECS);
  return Number.isFinite(raw) ? Math.min(MAX_SECS, Math.max(MIN_SECS, raw)) : DEFAULT_SECS;
}

async function fetchDefaults(): Promise<DefaultsPayload> {
  const msg = await HttpUtil.post('/panel/api/setting/defaultSettings', undefined, {
    silent: true,
  });
  if (!msg?.success) throw new Error(msg?.msg || 'Failed to fetch defaults');
  const validated = parseMsg(msg, DefaultsPayloadSchema, 'setting/defaultSettings');
  return validated.obj ?? {};
}

// The backend traffic job and every live table share one cadence. A shorter
// choice shows fresher counters; a longer one reduces gRPC/API load.
export function useRefreshInterval(): number {
  const query = useQuery({
    queryKey: keys.settings.defaults(),
    queryFn: fetchDefaults,
    staleTime: Infinity,
  });
  return normalizeSecs(query.data?.trafficPollSecs) * 1000;
}

// Used by the inbounds toolbar: updates the shared setting and refreshes the
// cached defaults immediately so every polling hook picks up the new cadence.
export function useRefreshIntervalSetting() {
  const queryClient = useQueryClient();
  const query = useQuery({
    queryKey: keys.settings.defaults(),
    queryFn: fetchDefaults,
    staleTime: Infinity,
  });
  const mutation = useMutation({
    mutationFn: async (secs: number) => {
      const msg = await HttpUtil.post('/panel/api/setting/update', { trafficPollSecs: secs });
      if (!msg?.success) {
        throw new Error(msg?.msg || 'Failed to save refresh interval');
      }
    },
    onMutate: (secs: number) => {
      queryClient.setQueryData<DefaultsPayload>(keys.settings.defaults(), (old) =>
        old ? { ...old, trafficPollSecs: secs } : old,
      );
    },
    onError: () => {
      queryClient.invalidateQueries({ queryKey: keys.settings.defaults() });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: keys.settings.defaults() });
      queryClient.invalidateQueries({ queryKey: keys.settings.all() });
    },
  });
  return {
    seconds: normalizeSecs(query.data?.trafficPollSecs),
    setSeconds: mutation.mutate,
    saving: mutation.isPending,
  };
}
