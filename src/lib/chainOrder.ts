// Pure rules for the Effects panel: its rows, selecting effects, and moving effects and
// groups by dragging or with the keyboard.
import type { ChainInfo, EffectInfo, EffectTarget, GroupInfo } from "./api";

/** One row of the Effects panel. `top` is the position of the row's top-level item. */
export type ChainRow =
  | { kind: "header"; group: GroupInfo; top: number; collapsed: boolean }
  /** `group` and `member` (its position in the group) are set for an effect in a group. */
  | { kind: "effect"; effect: EffectInfo; top: number; group: GroupInfo | null; member: number };

/** Where a dragged row would go: an effect's `moveEffect` target, or a group's `moveGroup` index. */
export type Drop = { kind: "effect"; to: EffectTarget } | { kind: "group"; index: number };

/** The rows of `chain` in order, leaving out the effects of groups in `collapsed`. */
export function chainRows(chain: ChainInfo, collapsed: readonly number[]): ChainRow[] {
  const groupOf = new Map<number, GroupInfo>();
  for (const g of chain.groups) for (const s of g.slotIds) groupOf.set(s, g);
  const rows: ChainRow[] = [];
  let top = -1;
  for (const effect of chain.effects) {
    const group = groupOf.get(effect.slotId) ?? null;
    const member = group ? group.slotIds.indexOf(effect.slotId) : -1;
    if (member <= 0) top += 1;
    if (member === 0) rows.push({ kind: "header", group: group!, top, collapsed: collapsed.includes(group!.groupId) });
    if (group && collapsed.includes(group.groupId)) continue;
    rows.push({ kind: "effect", effect, top, group, member });
  }
  return rows;
}

/**
 * The gap a dragged row would drop into (0..count): the number of rows whose middle is
 * above `y`. `mids` are the rows' vertical middles, in order.
 */
export function dropGap(mids: readonly number[], y: number): number {
  return mids.filter((m) => m < y).length;
}

/** Whether `row` is an effect in a group whose rows are showing. */
const isMember = (row: ChainRow | undefined): row is ChainRow & { kind: "effect"; group: GroupInfo } =>
  row?.kind === "effect" && row.group !== null;

/**
 * Where dropping `dragged` into gap `gap` of `rows` puts it, or null if it would stay where
 * it is or can't go there. An effect dropped between a group's header or effects joins the
 * group; just below a group's last effect it joins the group if `indented` (the pointer is
 * in the indented area), and goes after the group otherwise. A group only drops between
 * top-level items. Indexes count after the dragged item is taken out, as the backend does.
 */
export function dropTarget(rows: readonly ChainRow[], dragged: ChainRow, gap: number, indented: boolean): Drop | null {
  const g = Math.max(0, Math.min(rows.length, gap));
  const above = rows[g - 1];
  const below = rows[g];
  const topBefore = above ? above.top + 1 : 0;

  if (dragged.kind === "header") {
    if (isMember(below)) return null;
    const t = dragged.top;
    const index = t < topBefore ? topBefore - 1 : topBefore;
    return index === t ? null : { kind: "group", index };
  }

  // Inside a group: between its header and effects, or just below its last effect, indented.
  let inside: { group: GroupInfo; at: number } | null = null;
  if (isMember(below) && below.member > 0) inside = { group: below.group, at: below.member };
  else if (isMember(below) && below.member === 0) inside = { group: below.group, at: 0 };
  else if (isMember(above) && indented) inside = { group: above.group, at: above.member + 1 };

  if (inside) {
    const { group } = inside;
    const from = dragged.group?.groupId === group.groupId ? dragged.member : -1;
    const index = from >= 0 && from < inside.at ? inside.at - 1 : inside.at;
    return index === from ? null : { kind: "effect", to: { group: group.groupId, index } };
  }
  const t = dragged.group ? -1 : dragged.top;
  const index = t >= 0 && t < topBefore ? topBefore - 1 : topBefore;
  return index === t ? null : { kind: "effect", to: { group: null, index } };
}

/**
 * Where Alt+Up (-1) or Alt+Down (1) moves the item of `row`, or null at an end. An effect
 * moves within its group, and leaves it from the group's first or last place. A top-level
 * effect next to an expanded group enters it at that end, and passes a collapsed group.
 * A group moves past the item before or after it.
 */
export function stepTarget(rows: readonly ChainRow[], row: ChainRow, step: -1 | 1): Drop | null {
  const tops = rows.length ? rows[rows.length - 1].top + 1 : 0;
  if (row.kind === "header") {
    const index = row.top + step;
    return index < 0 || index >= tops ? null : { kind: "group", index };
  }
  if (row.group) {
    const index = row.member + step;
    if (index >= 0 && index < row.group.slotIds.length) return { kind: "effect", to: { group: row.group.groupId, index } };
    // Out of the group: just before it, or just after it.
    return { kind: "effect", to: { group: null, index: step < 0 ? row.top : row.top + 1 } };
  }
  const t = row.top + step;
  if (t < 0 || t >= tops) return null;
  const neighbour = rows.find((r) => r.top === t);
  if (neighbour?.kind === "header" && !neighbour.collapsed) {
    const g = neighbour.group;
    return { kind: "effect", to: { group: g.groupId, index: step < 0 ? g.slotIds.length : 0 } };
  }
  return { kind: "effect", to: { group: null, index: t } };
}

/** How a click on an effect changes the selection. */
export type ClickMode = "only" | "toggle" | "range";

/**
 * The selection after clicking effect `slot` with `mode`, and the effect to select ranges
 * from: one effect, one more or fewer, or every effect in `order` (the chain's slot ids in
 * processing order) from `anchor` to `slot`.
 */
export function clickSelection(
  order: readonly number[],
  selected: readonly number[],
  anchor: number | null,
  slot: number,
  mode: ClickMode,
): { selected: number[]; anchor: number } {
  if (mode === "toggle") {
    const next = selected.includes(slot) ? selected.filter((s) => s !== slot) : [...selected, slot];
    return { selected: next, anchor: slot };
  }
  const from = anchor === null ? -1 : order.indexOf(anchor);
  const to = order.indexOf(slot);
  if (mode === "range" && from >= 0 && to >= 0) {
    const [a, b] = from < to ? [from, to] : [to, from];
    return { selected: order.slice(a, b + 1), anchor: anchor! };
  }
  return { selected: [slot], anchor: slot };
}

/** The first group of `chain` holding a selected effect, for Ungroup. */
export function groupOfSelection(chain: ChainInfo, selected: readonly number[]): number | null {
  return chain.groups.find((g) => g.slotIds.some((s) => selected.includes(s)))?.groupId ?? null;
}
