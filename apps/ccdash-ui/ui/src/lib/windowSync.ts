import { get } from 'svelte/store';
import { windows as windowsApi } from './tauri';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { activeTab, mirrorTarget, selectedProjectId } from './stores';

type MirroredState = {
  selectedProjectId: string | null;
  activeTab: 'sessions' | 'ports' | 'plans';
};

let publishHandle: number | null = null;
let unlistenMirror: UnlistenFn | null = null;

export function startPublishing() {
  const myLabel = windowsApi.currentLabel();
  publishHandle = window.setInterval(() => {
    const state: MirroredState = {
      selectedProjectId: get(selectedProjectId),
      activeTab: get(activeTab),
    };
    windowsApi.publishState(myLabel, state).catch(() => {});
  }, 250);
}

export function stopPublishing() {
  if (publishHandle !== null) {
    clearInterval(publishHandle);
    publishHandle = null;
  }
}

export async function startMirroring(target: string) {
  if (unlistenMirror) unlistenMirror();
  unlistenMirror = await windowsApi.listenState<MirroredState>(target, (state) => {
    if (state.selectedProjectId !== undefined) selectedProjectId.set(state.selectedProjectId);
    if (state.activeTab !== undefined) activeTab.set(state.activeTab);
  });
  mirrorTarget.set(target);
}

export function stopMirroring() {
  if (unlistenMirror) {
    unlistenMirror();
    unlistenMirror = null;
  }
  mirrorTarget.set(null);
}
