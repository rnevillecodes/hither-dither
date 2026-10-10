// Display formatting for metadata and statistics.

/** h:mm:ss.mmm */
export function formatDuration(secs: number): string {
  const totalMs = Math.round(secs * 1000);
  const ms = totalMs % 1000;
  const totalS = Math.floor(totalMs / 1000);
  const s = totalS % 60;
  const m = Math.floor(totalS / 60) % 60;
  const h = Math.floor(totalS / 3600);
  return `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}.${String(ms).padStart(3, "0")}`;
}

/** Seconds with millisecond precision, e.g. "1.500 s". */
export function formatSeconds(secs: number): string {
  return `${secs.toFixed(3)} s`;
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KiB", "MiB", "GiB", "TiB"];
  let v = bytes / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v.toFixed(v < 10 ? 2 : 1)} ${units[i]} (${bytes.toLocaleString("en-US")} bytes)`;
}

export function formatInt(n: number): string {
  return n.toLocaleString("en-US");
}

export function formatRate(hz: number): string {
  return `${formatInt(hz)} Hz`;
}

/** dB value with one decimal; null means -inf. Avoids "-0.0". */
export function formatDb(db: number | null, unit = "dBFS"): string {
  if (db === null || db === -Infinity) return `-inf ${unit}`;
  const v = Math.abs(db) < 0.05 ? 0 : db;
  return `${v.toFixed(1)} ${unit}`;
}

export function formatLinear(v: number): string {
  return v.toFixed(4);
}

export function formatPercent(p: number): string {
  const v = Math.abs(p) < 0.005 ? 0 : p;
  return `${v.toFixed(2)}%`;
}

export function formatBitDepth(bits: number, validBits: number, encoding: string): string {
  const base = `${bits}-bit`;
  return validBits !== bits && encoding === "PCM" ? `${base} (${validBits} valid)` : base;
}
