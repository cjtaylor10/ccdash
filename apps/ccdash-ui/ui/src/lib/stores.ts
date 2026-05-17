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

export const activeTab = writable<'sessions' | 'ports' | 'plans'>('sessions');

export type TerminalPaneState = {
  command: string[];
  mode: 'live';
};
export const terminalPane = writable<TerminalPaneState | null>(null);

/** When set to a window label, this window mirrors that one's UI state. */
export const mirrorTarget = writable<string | null>(null);
