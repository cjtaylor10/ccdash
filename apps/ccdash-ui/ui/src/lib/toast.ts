import { writable } from 'svelte/store';

export type ToastKind = 'ok' | 'err';
export interface Toast {
  id: number;
  msg: string;
  kind: ToastKind;
}

export const toast = writable<Toast | null>(null);

let seq = 0;
let timer: number | null = null;

export function showToast(msg: string, kind: ToastKind = 'ok', ms = 2200) {
  const id = ++seq;
  toast.set({ id, msg, kind });
  if (timer) clearTimeout(timer);
  timer = window.setTimeout(() => {
    toast.update((t) => (t && t.id === id ? null : t));
  }, ms);
}
