import { invoke } from '@tauri-apps/api/core';

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

/** True if the daemon's error message indicates a port conflict. The detailed
 * conflict payload is in error.data on the daemon side, but the current
 * Tauri command bridge only forwards error.message — so callers can only
 * detect the condition, not extract the force_token. Full remediation is a
 * Phase 7 task. */
export function isPortConflictMessage(msg: string): boolean {
  return /port conflict/i.test(msg);
}

export const projectsApi = {
  add: (path: string, name?: string) =>
    invoke<Project>('project_add', { path, name }),
  remove: (id: string) => invoke<null>('project_remove', { id }),
};

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
