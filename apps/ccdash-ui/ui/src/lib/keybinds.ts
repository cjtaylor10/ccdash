/** Global keybind dispatcher. Returns a cleanup function. */
import { getCurrentWindow } from '@tauri-apps/api/window';
import { windows as windowsApi } from './tauri';

export interface KeybindHandlers {
  openCommandPalette: () => void;
  openLaunchDialog: () => void;
}

function isMod(e: KeyboardEvent): boolean {
  return e.metaKey || e.ctrlKey;
}

function activeElementIsInput(): boolean {
  const a = document.activeElement;
  if (!a) return false;
  const tag = a.tagName;
  return tag === 'INPUT' || tag === 'TEXTAREA' || (a as HTMLElement).isContentEditable;
}

export function installKeybinds(handlers: KeybindHandlers): () => void {
  async function onKeydown(e: KeyboardEvent) {
    if (!isMod(e)) return;
    const key = e.key.toLowerCase();

    if (key === 'n' && !e.shiftKey && !e.altKey) {
      e.preventDefault();
      try { await windowsApi.openNew(); } catch {}
      return;
    }
    if (key === 'w' && !e.shiftKey && !e.altKey) {
      e.preventDefault();
      try { await getCurrentWindow().close(); } catch {}
      return;
    }
    if (key === 'k' && !e.shiftKey && !e.altKey) {
      // Cmd+K opens palette regardless of focus; users expect this from VS Code / Linear.
      e.preventDefault();
      handlers.openCommandPalette();
      return;
    }
    if (key === 'l' && !e.shiftKey && !e.altKey && !activeElementIsInput()) {
      // Cmd+L opens launch dialog (mnemonic: Launch).
      e.preventDefault();
      handlers.openLaunchDialog();
      return;
    }
  }
  window.addEventListener('keydown', onKeydown);
  return () => window.removeEventListener('keydown', onKeydown);
}
