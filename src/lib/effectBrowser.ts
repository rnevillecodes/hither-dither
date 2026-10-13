// Pure rules for the Add Effect browser: which effects to offer, search, and compatibility.
import type { PluginEntry, ScanProgress } from "./api";

export interface BrowserEntry {
  classId: string;
  name: string;
  vendor: string;
  /** It can process the file's channels. */
  usable: boolean;
  /** Why it can't, when it can't. */
  reason: string | null;
}

export interface VendorGroup {
  vendor: string;
  entries: BrowserEntry[];
}

/** The file an effect would be added to. */
export interface FileLayout {
  speaker_arrangement: number;
  layout: string;
}

const byName = (a: string, b: string) => a.localeCompare(b, undefined, { sensitivity: "base" });

/** Whether every word of `query` appears in the effect's name or vendor, ignoring case. */
export function matches(plugin: Pick<PluginEntry, "name" | "vendor">, query: string): boolean {
  const text = `${plugin.name} ${plugin.vendor}`.toLowerCase();
  return query
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean)
    .every((word) => text.includes(word));
}

/** Whether the effect accepts the file's channel layout on its main input and output. */
export function compatible(plugin: Pick<PluginEntry, "layouts">, file: FileLayout): boolean {
  return plugin.layouts.some((l) => l.arrangement === file.speaker_arrangement);
}

/**
 * The effects to offer for `file`, matching `query`, grouped by vendor (both in name order).
 * Only ready effects are offered: failed, incompatible, disabled and effect-less plugins
 * are left out. Effects that can't process the file's channels are kept, with the reason.
 */
export function browse(plugins: readonly PluginEntry[], query: string, file: FileLayout): VendorGroup[] {
  const groups = new Map<string, BrowserEntry[]>();
  for (const p of plugins) {
    if (p.status !== "ready" || !p.classId || !matches(p, query)) continue;
    const usable = compatible(p, file);
    const vendor = p.vendor.trim() || "Unknown vendor";
    const entry = {
      classId: p.classId,
      name: p.name,
      vendor,
      usable,
      reason: usable ? null : `Doesn't support ${file.layout} audio`,
    };
    groups.set(vendor, [...(groups.get(vendor) ?? []), entry]);
  }
  return [...groups]
    .sort(([a], [b]) => byName(a, b))
    .map(([vendor, entries]) => ({ vendor, entries: entries.sort((a, b) => byName(a.name, b.name)) }));
}

/** "Scanning plugins… n of m" while a scan runs, else null. */
export function scanText(progress: ScanProgress | null): string | null {
  return progress && progress.done < progress.total ? `Scanning plugins… ${progress.done} of ${progress.total}` : null;
}
