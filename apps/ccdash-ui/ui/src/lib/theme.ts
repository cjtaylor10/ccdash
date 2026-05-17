import { writable } from 'svelte/store';

export type Theme = 'light' | 'dark' | 'system';

const KEY = 'ccdash.theme';

function read(): Theme {
  try {
    const t = localStorage.getItem(KEY);
    if (t === 'light' || t === 'dark' || t === 'system') return t;
  } catch {}
  return 'system';
}

function resolved(t: Theme): 'light' | 'dark' {
  if (t !== 'system') return t;
  const m = window.matchMedia?.('(prefers-color-scheme: light)');
  return m && m.matches ? 'light' : 'dark';
}

function apply(t: Theme) {
  document.documentElement.dataset.theme = resolved(t);
}

export const theme = writable<Theme>(read());

theme.subscribe((t) => {
  try {
    localStorage.setItem(KEY, t);
  } catch {}
  apply(t);
});

/** Listen to the OS theme change while the user has 'system' selected. */
export function watchSystem() {
  const m = window.matchMedia?.('(prefers-color-scheme: light)');
  if (!m) return;
  const onChange = () => {
    let t: Theme = 'system';
    theme.subscribe((v) => (t = v))();
    if (t === 'system') apply(t);
  };
  m.addEventListener('change', onChange);
}
