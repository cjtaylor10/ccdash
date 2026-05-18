import { writable } from 'svelte/store';
import type { Plan, PortBinding, Project, DeclaredPort, Session } from './tauri';

export const connected = writable<boolean>(false);
export const connectError = writable<string | null>(null);

export const projects = writable<Project[]>([]);
export const selectedProjectId = writable<string | null>(null);
export const sessions = writable<Session[]>([]);
export const ports = writable<{ running: PortBinding[]; declared: DeclaredPort[] }>({
  running: [],
  declared: [],
});
export const plans = writable<Plan[]>([]);

export const activeTab = writable<'sessions' | 'ports' | 'plans' | 'browser'>('sessions');

/** Loopback URLs detected from terminal output, grouped by the tmux session
 *  id that emitted them. A `null` key collects URLs derived from the
 *  machine-wide port scan (no session affiliation). BrowserView merges the
 *  active session's set with the `null` set when displaying suggestions. */
export const detectedUrlsBySession = writable<Map<string | null, Set<string>>>(
  new Map([[null, new Set()]]),
);

export type TerminalPaneState = {
  /** tmux session id this pane is attached to (e.g. "$0"). */
  sessionId: string;
  /** Command vector used to mount the xterm — always
   *  ["tmux", "attach-session", "-t", sessionId]. Kept verbatim so the
   *  Terminal component can echo it in its header and for parity with the
   *  existing pty-bridge contract. */
  command: string[];
};

/** Sessions the user has clicked Attach on. Order = insertion order.
 *  Each gets a persistent xterm + pty in App.svelte; switching between
 *  them just toggles visibility (no remount, no scrollback loss). */
export const attachedSessions = writable<TerminalPaneState[]>([]);

/** The currently-visible attached session, or null if no terminal is open. */
export const activeTerminalSessionId = writable<string | null>(null);

/** Whether the terminal panel is collapsed (header visible, body hidden).
 *  Persisted to localStorage so it's sticky across launches. */
function readCollapsed(): boolean {
  try { return localStorage.getItem('ccdash.terminalCollapsed') === '1'; } catch { return false; }
}
export const terminalCollapsed = writable<boolean>(readCollapsed());
terminalCollapsed.subscribe((v) => {
  try { localStorage.setItem('ccdash.terminalCollapsed', v ? '1' : '0'); } catch {}
});

/** Per-session browser state — current URL, history stack, reload counter.
 *  Lets each Claude session keep its own browser viewport instead of
 *  fighting over one shared iframe. */
export type BrowserState = {
  history: string[];
  index: number;
  address: string;
  reloadCounter: number;
};
export const browserStateBySession = writable<Map<string | null, BrowserState>>(
  new Map([[null, { history: [], index: -1, address: '', reloadCounter: 0 }]]),
);

/** When set to a window label, this window mirrors that one's UI state. */
export const mirrorTarget = writable<string | null>(null);

// === Phase 7: reconnect state ===
export const reconnecting = writable<boolean>(false);
export const nextRetryAt = writable<number | null>(null);
