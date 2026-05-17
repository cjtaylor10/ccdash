import { tauri } from './tauri';
import { connected, connectError, reconnecting, nextRetryAt } from './stores';

const BASE_DELAY_MS = 5_000;
const MAX_DELAY_MS = 30_000;

let attempt = 0;
let timer: number | null = null;
let stopped = false;
let resolveTick: (() => void) | null = null;

function delayMs(): number {
  return Math.min(BASE_DELAY_MS * Math.pow(2, attempt), MAX_DELAY_MS);
}

async function tryConnect(refreshAll: () => Promise<void>): Promise<boolean> {
  try {
    await tauri.connect();
    await refreshAll();
    connected.set(true);
    connectError.set(null);
    reconnecting.set(false);
    nextRetryAt.set(null);
    attempt = 0;
    return true;
  } catch (e) {
    connectError.set(String(e));
    return false;
  }
}

export async function startReconnectLoop(refreshAll: () => Promise<void>) {
  stopped = false;
  reconnecting.set(true);
  connected.set(false);
  while (!stopped) {
    if (await tryConnect(refreshAll)) return;
    attempt++;
    const wait = delayMs();
    nextRetryAt.set(Date.now() + wait);
    await new Promise<void>((resolve) => {
      resolveTick = resolve;
      timer = window.setTimeout(() => {
        timer = null;
        resolveTick = null;
        resolve();
      }, wait);
    });
  }
}

/** Cancel the current backoff wait and try immediately. */
export function retryNow() {
  if (timer !== null) {
    clearTimeout(timer);
    timer = null;
  }
  if (resolveTick) {
    const r = resolveTick;
    resolveTick = null;
    r();
  }
}

export function stopReconnectLoop() {
  stopped = true;
  if (timer !== null) {
    clearTimeout(timer);
    timer = null;
  }
  reconnecting.set(false);
}
