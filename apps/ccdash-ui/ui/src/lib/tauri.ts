import { invoke } from '@tauri-apps/api/core';

/** Structured error returned by every RPC-proxying Tauri command.
 * Tauri's invoke() rejects with the unwrapped error object (not wrapped in
 * Error). The shape is `{ message, data? }` matching the Rust `UiRpcError`. */
export interface UiRpcError {
  message: string;
  data?: unknown;
}

/** Type guard for the structured RPC error shape. */
export function isUiRpcError(e: unknown): e is UiRpcError {
  return (
    typeof e === 'object' &&
    e !== null &&
    'message' in e &&
    typeof (e as { message: unknown }).message === 'string'
  );
}

/** Read `e.data` as PortConflictData if the shape matches. */
export function asPortConflict(e: unknown): PortConflictData | null {
  if (!isUiRpcError(e) || e.data == null) return null;
  const d = e.data as { conflicts?: unknown; force_token?: unknown };
  if (!Array.isArray(d.conflicts) || typeof d.force_token !== 'string') return null;
  return { conflicts: d.conflicts as PortConflict[], force_token: d.force_token };
}

export interface Project {
  id: string;
  name: string;
  path: string;
  worktrees: Worktree[];
  state: 'ok' | 'missing';
}

export interface Worktree {
  path: string;
  branch: string;
  is_primary: boolean;
}

export interface Session {
  tmux_session_id: string;
  name: string;
  project_id: string | null;
  worktree: string | null;
  cwd: string;
  pid: number;
  state: 'running' | 'exited';
  first_seen: number;
}

export interface PortBinding {
  port: number;
  protocol: string;
  pid: number | null;
  command: string | null;
  project_id: string | null;
}

export interface DeclaredPort {
  project_id: string;
  port: number;
  source: string;
}

export interface Plan {
  path: string;
  title: string;
  phases: PlanPhase[];
}

export interface PlanPhase {
  name: string;
  tasks: PlanTask[];
}

export interface PlanTask {
  title: string;
  done: boolean;
}

export const tauri = {
  connect: () => invoke<string>('connect_and_handshake'),
  projectList: () => invoke<{ projects: Project[] }>('project_list'),
  sessionList: () => invoke<{ sessions: Session[] }>('session_list'),
  portsList: () => invoke<{ running: PortBinding[]; declared: DeclaredPort[] }>('ports_list'),
  plansGet: (projectId: string) => invoke<{ plans: Plan[] }>('plans_get', { projectId }),
};

export const terminal = {
  open: (command: string[], rows: number, cols: number) =>
    invoke<string>('terminal_open', { command, rows, cols }),
  write: (id: string, data: Uint8Array) =>
    invoke<void>('terminal_write', { id, data: Array.from(data) }),
  resize: (id: string, rows: number, cols: number) =>
    invoke<void>('terminal_resize', { id, rows, cols }),
  close: (id: string) => invoke<void>('terminal_close', { id }),
};

import { listen as tauriListen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

export const windows = {
  openNew: () => invoke<void>('open_new_window'),
  list: () => invoke<string[]>('list_windows'),
  publishState: (from: string, state: unknown) =>
    invoke<void>('publish_window_state', { from, state }),
  listenState: <T = unknown>(
    from: string,
    handler: (state: T) => void,
  ): Promise<UnlistenFn> =>
    tauriListen<T>(`window-state-broadcast::${from}`, (e) => handler(e.payload)),
  currentLabel: () => getCurrentWindow().label,
};

// === Phase 6: project + session management ===

export interface PortConflict {
  port: number;
  holder: string;
}

export interface PortConflictData {
  conflicts: PortConflict[];
  force_token: string;
}

export const projectsApi = {
  add: (path: string, name?: string) =>
    invoke<Project>('project_add', { path, name }),
  remove: (id: string) => invoke<null>('project_remove', { id }),
};

// === Phase 8: first-run / onboarding ===

export interface DiscoveredRepo {
  path: string;
  suggested_name: string;
}

export const daemonApi = {
  firstRunStatus: () => invoke<{ pending: boolean }>('first_run_status'),
  firstRunComplete: () => invoke<unknown>('first_run_complete'),
  scanPaths: (roots: string[]) =>
    invoke<{ discovered: DiscoveredRepo[] }>('scan_paths', { roots }),
};

// === Phase 9: browser preview helpers ===

/** Open `url` in the system's default browser via tauri-plugin-shell. */
export async function openExternal(url: string): Promise<void> {
  const { open } = await import('@tauri-apps/plugin-shell');
  await open(url);
}

export interface LaunchOpts {
  projectId: string;
  worktree?: string;
  command?: string;
  forceToken?: string;
}

export const sessionsApi = {
  launch: (opts: LaunchOpts) =>
    invoke<{ session: Session }>('session_launch', {
      projectId: opts.projectId,
      worktree: opts.worktree,
      command: opts.command,
      forceToken: opts.forceToken,
    }),
  kill: (tmuxSessionId: string) =>
    invoke<null>('session_kill', { tmuxSessionId }),
};
