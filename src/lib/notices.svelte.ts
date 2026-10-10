// Toast notices shown in the corner of the window.

export type NoticeLevel = "info" | "warning" | "error";

export interface Notice {
  id: number;
  level: NoticeLevel;
  title: string;
  body?: string;
}

let nextId = 1;

export const notices = $state<Notice[]>([]);

export function dismiss(id: number) {
  const i = notices.findIndex((n) => n.id === id);
  if (i >= 0) notices.splice(i, 1);
}

/** Show a notice; info and warnings auto-dismiss, errors stay until closed. */
export function notify(level: NoticeLevel, title: string, body?: string) {
  const id = nextId++;
  notices.push({ id, level, title, body });
  if (level !== "error") setTimeout(() => dismiss(id), level === "info" ? 4000 : 8000);
}
