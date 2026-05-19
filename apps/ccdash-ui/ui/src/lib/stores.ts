import { derived, writable } from 'svelte/store';
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

/** True iff `cwd` equals `path` or sits beneath it. Avoids the
 *  `/a/b` vs `/a/bb` false positive that a naive startsWith would have. */
export function pathContains(cwd: string, path: string): boolean {
  if (!cwd || !path) return false;
  if (cwd === path) return true;
  const prefix = path.endsWith('/') ? path : path + '/';
  return cwd.startsWith(prefix);
}

/** Map of `tmux_session_id` → owning project id (or null if none can be
 *  inferred). Prefers the daemon-stamped `project_id`, falling back to a
 *  longest-prefix match of the session's `cwd` against each project's path
 *  and worktree paths — so sessions started outside ccdash (with no
 *  `project_id` stamp) still attribute correctly.
 *
 *  Shared between Sidebar (for grouping sessions under their project) and
 *  BrowserView (for the project-scoped sub-tab filter), so both views see
 *  the same attribution. */
export const resolvedProjectByTmuxId = derived(
  [sessions, projects],
  ([$sessions, $projects]) => {
    const map = new Map<string, string | null>();
    for (const s of $sessions) {
      if (s.project_id) {
        map.set(s.tmux_session_id, s.project_id);
        continue;
      }
      let best: { id: string; len: number } | null = null;
      for (const p of $projects) {
        const candidates = [p.path, ...p.worktrees.map((w) => w.path)];
        for (const c of candidates) {
          if (pathContains(s.cwd, c) && (!best || c.length > best.len)) {
            best = { id: p.id, len: c.length };
          }
        }
      }
      map.set(s.tmux_session_id, best?.id ?? null);
    }
    return map;
  },
);

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

// === Resizable layout (sidebar width, terminal panel height) ===

function readNum(key: string, fallback: number): number {
  try {
    const v = localStorage.getItem(key);
    if (v === null) return fallback;
    const n = Number(v);
    return Number.isFinite(n) ? n : fallback;
  } catch { return fallback; }
}

/** Sidebar width in pixels. Clamped at the consumer site. */
export const sidebarWidth = writable<number>(readNum('ccdash.sidebarWidth', 232));
sidebarWidth.subscribe((v) => {
  try { localStorage.setItem('ccdash.sidebarWidth', String(v)); } catch {}
});

/** Sidebar fully collapsed (off-canvas with a floating expand button). */
function readSidebarCollapsed(): boolean {
  try { return localStorage.getItem('ccdash.sidebarCollapsed') === '1'; } catch { return false; }
}
export const sidebarCollapsed = writable<boolean>(readSidebarCollapsed());
sidebarCollapsed.subscribe((v) => {
  try { localStorage.setItem('ccdash.sidebarCollapsed', v ? '1' : '0'); } catch {}
});

/** Terminal panel height in pixels. Clamped at the consumer site. */
export const terminalPanelHeight = writable<number>(readNum('ccdash.terminalPanelHeight', 340));
terminalPanelHeight.subscribe((v) => {
  try { localStorage.setItem('ccdash.terminalPanelHeight', String(v)); } catch {}
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

// === Panes (upper content area) ===

const PANE_TYPES = ['browser', 'plans', 'sessions', 'ports'] as const;
export type PaneType = typeof PANE_TYPES[number];

/** A pane in the upper content area. `type === null` means the user clicked
 *  `+ Pane` but hasn't yet picked a content type — the pane renders an
 *  empty placeholder with the type-picker dropdown open. */
export type Pane = {
  id: string;
  type: PaneType | null;
};

function makePaneId(): string {
  // crypto.randomUUID exists in all modern browsers and Tauri's webview.
  // The cast handles tsconfig DOM lib versions that haven't added the type.
  return (globalThis.crypto as Crypto).randomUUID();
}

/** First-launch default: Sessions on the left, Browser on the right.
 *  Two panes (not one) so new users see their sessions list immediately
 *  AND have a Browser ready — opening to a single Browser meant new users
 *  had no obvious way to attach a session. */
function defaultPanes(): Pane[] {
  return [
    { id: makePaneId(), type: 'sessions' },
    { id: makePaneId(), type: 'browser' },
  ];
}

function readPanes(): Pane[] {
  try {
    const raw = localStorage.getItem('ccdash.panes');
    if (!raw) return defaultPanes();
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed) || parsed.length === 0) {
      return defaultPanes();
    }
    const valid: Pane[] = [];
    for (const p of parsed) {
      if (
        p &&
        typeof p === 'object' &&
        typeof p.id === 'string' &&
        (p.type === null || (PANE_TYPES as readonly string[]).includes(p.type))
      ) {
        valid.push({ id: p.id, type: p.type });
      }
    }
    if (valid.length === 0) return defaultPanes();
    return valid;
  } catch {
    return defaultPanes();
  }
}

export const panes = writable<Pane[]>(readPanes());
panes.subscribe((v) => {
  try {
    localStorage.setItem('ccdash.panes', JSON.stringify(v));
  } catch {}
});

function readPaneLayoutDirection(): 'row' | 'column' {
  try {
    const v = localStorage.getItem('ccdash.paneLayoutDirection');
    return v === 'column' ? 'column' : 'row';
  } catch {
    return 'row';
  }
}

export const paneLayoutDirection = writable<'row' | 'column'>(
  readPaneLayoutDirection(),
);
paneLayoutDirection.subscribe((v) => {
  try {
    localStorage.setItem('ccdash.paneLayoutDirection', v);
  } catch {}
});

/** Width (row mode) or height (column mode) of each pane keyed by pane id.
 *  Missing entries → pane gets equal share of remaining space. NOT persisted
 *  in V1 — sizes reset on reload, layout shape does not. */
export const paneSizeById = writable<Map<string, number>>(new Map());

/** Which session sub-tab a given Browser pane is showing. Not persisted —
 *  recomputed from in-scope sessions on load. */
export const browserPaneSubtabByPaneId = writable<Map<string, string>>(
  new Map(),
);

/** Helper used by the toolbar `+ Pane` button. Appends an empty pane to the
 *  end of the layout. */
export function addPane(): void {
  panes.update((arr) => [...arr, { id: makePaneId(), type: null }]);
}

/** Helper used by each pane's `✕` button. No-op when called on the last
 *  remaining pane (the workspace always has at least one pane). */
export function removePane(id: string): void {
  panes.update((arr) => (arr.length <= 1 ? arr : arr.filter((p) => p.id !== id)));
  browserPaneSubtabByPaneId.update((m) => {
    if (!m.has(id)) return m;
    const next = new Map(m);
    next.delete(id);
    return next;
  });
  paneSizeById.update((m) => {
    if (!m.has(id)) return m;
    const next = new Map(m);
    next.delete(id);
    return next;
  });
}

export function setPaneType(id: string, type: PaneType): void {
  panes.update((arr) => arr.map((p) => (p.id === id ? { ...p, type } : p)));
}

// === Phase 7: reconnect state ===
export const reconnecting = writable<boolean>(false);
export const nextRetryAt = writable<number | null>(null);

// === Top-level view (workspace vs. prompts) ===

function readActiveView(): 'workspace' | 'prompts' {
  try {
    const v = localStorage.getItem('ccdash.activeView');
    return v === 'prompts' ? 'prompts' : 'workspace';
  } catch {
    return 'workspace';
  }
}

export const activeView = writable<'workspace' | 'prompts'>(readActiveView());
activeView.subscribe((v) => {
  try {
    localStorage.setItem('ccdash.activeView', v);
  } catch {}
});

// === Prompts library ===

export type Prompt = {
  id: string;
  title: string;
  body: string;
  createdAt: number;
  updatedAt: number;
};

function readPrompts(): Prompt[] {
  try {
    const raw = localStorage.getItem('ccdash.prompts');
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    const out: Prompt[] = [];
    for (const p of parsed) {
      if (
        p &&
        typeof p === 'object' &&
        typeof p.id === 'string' &&
        typeof p.title === 'string' &&
        typeof p.body === 'string' &&
        Number.isFinite(p.createdAt) &&
        Number.isFinite(p.updatedAt)
      ) {
        out.push({
          id: p.id,
          title: p.title,
          body: p.body,
          createdAt: p.createdAt,
          updatedAt: p.updatedAt,
        });
      }
    }
    return out;
  } catch {
    return [];
  }
}

export const prompts = writable<Prompt[]>(readPrompts());
prompts.subscribe((v) => {
  try {
    localStorage.setItem('ccdash.prompts', JSON.stringify(v));
  } catch {}
});

export function addPrompt(): string {
  const id = (globalThis.crypto as Crypto).randomUUID();
  const now = Date.now();
  prompts.update((arr) => [
    { id, title: '', body: '', createdAt: now, updatedAt: now },
    ...arr,
  ]);
  return id;
}

export function updatePrompt(
  id: string,
  patch: Partial<Pick<Prompt, 'title' | 'body'>>,
): void {
  prompts.update((arr) =>
    arr.map((p) => (p.id === id ? { ...p, ...patch, updatedAt: Date.now() } : p)),
  );
}

export function deletePrompt(id: string): void {
  prompts.update((arr) => arr.filter((p) => p.id !== id));
}
