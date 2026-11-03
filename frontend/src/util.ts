export function formatBytes(value: number): string {
  if (!Number.isFinite(value) || value <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB", "PB"];
  let index = 0;
  let size = value;
  while (size >= 1024 && index < units.length - 1) {
    size /= 1024;
    index += 1;
  }
  return `${size.toFixed(index === 0 ? 0 : 2)} ${units[index]}`;
}

export function formatUptime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return "0 分钟";
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const parts: string[] = [];
  if (days > 0) parts.push(`${days} 天`);
  if (hours > 0) parts.push(`${hours} 小时`);
  if (minutes > 0 || parts.length === 0) parts.push(`${minutes} 分钟`);
  return parts.join(" ");
}

export function formatTime(unixSeconds?: number | null): string {
  if (!unixSeconds) return "永久";
  return new Date(unixSeconds * 1000).toLocaleString("zh-CN", {
    hour12: false,
  });
}

export function formatShortTime(unixSeconds?: number | null): string {
  if (!unixSeconds) return "—";
  return new Date(unixSeconds * 1000).toLocaleTimeString("zh-CN", {
    hour12: false,
  });
}

export function toUnix(date: unknown): number {
  if (!date) return 0;
  if (typeof date === "object" && date !== null && "unix" in date) {
    return Number((date as { unix: () => number }).unix());
  }
  return 0;
}

export function parseJson<T>(text: string, field: string): T {
  try {
    return JSON.parse(text) as T;
  } catch (error) {
    throw new Error(`${field} 不是合法 JSON: ${(error as Error).message}`);
  }
}

export async function copyText(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    // Clipboard API is unavailable on plain HTTP; fall back to a temp textarea.
    const area = document.createElement("textarea");
    area.value = text;
    area.style.position = "fixed";
    area.style.opacity = "0";
    document.body.appendChild(area);
    area.select();
    document.execCommand("copy");
    area.remove();
  }
}
