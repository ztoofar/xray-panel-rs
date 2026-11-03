import { useMemo, useState } from 'react';

import { TimeFormatter } from '@/utils';
import type { Status } from '@/models/status';

const OVERVIEW_WINDOW = 72;

const SERIES_KEYS = [
  'cpu',
  'mem',
  'swap',
  'diskUsage',
  'netUp',
  'netDown',
  'tcpCount',
  'udpCount',
] as const;

export type OverviewSeriesKey = (typeof SERIES_KEYS)[number];

export interface OverviewHistory {
  series: Record<OverviewSeriesKey, number[]>;
  labels: string[];
}

interface HistoryWindow {
  series: Record<OverviewSeriesKey, number[]>;
  times: number[];
}

function emptySeries(): Record<OverviewSeriesKey, number[]> {
  return Object.fromEntries(SERIES_KEYS.map((key) => [key, [] as number[]])) as Record<
    OverviewSeriesKey,
    number[]
  >;
}

function emptyWindow(): HistoryWindow {
  return { series: emptySeries(), times: [] };
}

function sampleOf(status: Status): Record<OverviewSeriesKey, number> {
  return {
    cpu: status.cpu.percent,
    mem: status.mem.percent,
    swap: status.swap.percent,
    diskUsage: status.disk.percent,
    netUp: status.netIO.up,
    netDown: status.netIO.down,
    tcpCount: status.tcpCount,
    udpCount: status.udpCount,
  };
}

function tailWindow<T>(values: T[]): T[] {
  return values.slice(-OVERVIEW_WINDOW);
}

export function mean(values: number[]): number {
  if (values.length === 0) return 0;
  let total = 0;
  for (const v of values) total += v;
  return total / values.length;
}

export function peak(values: number[]): number {
  let max = 0;
  for (const v of values) if (v > max) max = v;
  return max;
}

export function useOverviewHistory(status: Status, hasData: boolean): OverviewHistory {
  const [trend, setTrend] = useState<HistoryWindow>(emptyWindow);

  // Each polled status is appended during render; an effect would show the
  // chart one sample behind the numbers beside it.
  const [sampledStatus, setSampledStatus] = useState<Status | null>(null);
  if (hasData && status !== sampledStatus) {
    setSampledStatus(status);
    setTrend((prev) => {
      const scopeChanged = sampledStatus && sampledStatus.resourceScope !== status.resourceScope;
      const point = sampleOf(status);
      const next = emptyWindow();
      next.times = tailWindow(prev.times.concat(Math.floor(Date.now() / 1000)));
      for (const key of SERIES_KEYS) {
        const scoped = key === 'cpu' || key === 'swap';
        const previous = scoped && scopeChanged ? [] : prev.series[key];
        next.series[key] =
          !scoped || status.metricAvailable(key)
            ? tailWindow(previous.concat(point[key]))
            : previous;
      }
      return next;
    });
  }

  const labels = useMemo(() => trend.times.map(TimeFormatter.formatClock), [trend.times]);

  return useMemo(() => ({ series: trend.series, labels }), [trend.series, labels]);
}
