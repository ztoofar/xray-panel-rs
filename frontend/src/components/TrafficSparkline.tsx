import type { TrafficPoint } from "../types";

interface Props {
  points: TrafficPoint[];
  height?: number;
}

export default function TrafficSparkline({ points, height = 160 }: Props) {
  if (points.length < 2) {
    return <div className="empty-chart">暂无流量历史</div>;
  }

  const width = 640;
  const padding = 8;
  const max = Math.max(
    ...points.map((point) => point.up + point.down),
    1,
  );

  const polyline = (pick: (point: TrafficPoint) => number) =>
    points
      .map((point, index) => {
        const x =
          padding + (index / (points.length - 1)) * (width - padding * 2);
        const y =
          height - padding - (pick(point) / max) * (height - padding * 2);
        return `${x.toFixed(1)},${y.toFixed(1)}`;
      })
      .join(" ");

  return (
    <svg
      className="sparkline"
      style={{ height }}
      viewBox={`0 0 ${width} ${height}`}
      preserveAspectRatio="none"
    >
      <polyline
        points={polyline((point) => point.up)}
        fill="none"
        stroke="#1677ff"
        strokeWidth="2"
        vectorEffect="non-scaling-stroke"
      />
      <polyline
        points={polyline((point) => point.down)}
        fill="none"
        stroke="#52c41a"
        strokeWidth="2"
        vectorEffect="non-scaling-stroke"
      />
    </svg>
  );
}
