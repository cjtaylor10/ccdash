# Phase 9: Embedded browser preview — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development or superpowers:executing-plans.

**Goal:** A 4th top-level tab "Browser" that hosts a localhost preview pane. Auto-detects dev-server URLs from running ports AND from terminal output, surfaces them as one-click navigation entries.

**Architecture decisions (user-confirmed via AskUserQuestion):**
1. Top-level Browser tab next to Sessions/Ports/Plans (not per-terminal).
2. Detect URLs from BOTH the existing Ports tab data AND terminal output regex.
3. Address bar always visible.
4. Controls: Back, Forward, Reload, Open-in-external-browser.

**Implementation choice — iframe over nested WebviewWindow:**
- Tauri 2 supports `WebviewWindow::add_child`, but for the local-preview-of-a-localhost-URL use case an `<iframe>` is much simpler:
  - No additional capability config / sandbox boilerplate
  - Works in plain Svelte DOM
  - Cross-origin iframe (file:// → localhost) renders fine (only DOM access is blocked, which we don't need)
  - Custom navigation chrome around the iframe lives in the Svelte side
- "Open in external browser" via `tauri-plugin-shell::open(url)` for users who want full DevTools.

**Tech Stack:** Tauri 2 plugin-shell for external URL launch, plain `<iframe>`, Svelte 5 store for detected URLs, simple in-memory navigation history.

---

## File map

### Rust
- Modify: `apps/ccdash-ui/Cargo.toml` — add `tauri-plugin-shell = "2"`.
- Modify: `apps/ccdash-ui/src/main.rs` — register plugin.
- Modify: `apps/ccdash-ui/capabilities/default.json` — grant `shell:allow-open`.
- Modify: `apps/ccdash-ui/ui/package.json` — add `@tauri-apps/plugin-shell`.

### Frontend
- Modify: `apps/ccdash-ui/ui/src/lib/stores.ts` — add `activeTab` union to include `'browser'`; new `detectedUrls: Set<string>` store.
- Modify: `apps/ccdash-ui/ui/src/lib/tauri.ts` — `openExternal(url)` helper.
- Create: `apps/ccdash-ui/ui/src/lib/urlDetect.ts` — regex helper + the store-update logic. Pure module so it's unit-testable.
- Modify: `apps/ccdash-ui/ui/src/lib/components/Terminal.svelte` — decode each output chunk and run urlDetect on it.
- Create: `apps/ccdash-ui/ui/src/lib/components/BrowserView.svelte` — the tab content. Address bar, back/forward/reload buttons, "Open in external browser" button, iframe pane, detected-URLs sidebar list.
- Modify: `apps/ccdash-ui/ui/src/App.svelte` — add the Browser tab button + the BrowserView mount; subscribe Ports updates to feed `detectedUrls`.

---

## Task 1: Plugin install + capability

(Mirrors Phase 6 Task 1 for the dialog plugin.)

- Cargo.toml: `tauri-plugin-shell = "2"`.
- main.rs: `.plugin(tauri_plugin_shell::init())` after the dialog plugin init.
- capabilities/default.json: append `"shell:allow-open"`.
- package.json: add `@tauri-apps/plugin-shell` and run `pnpm install`.
- Verify with `cargo check -p ccdash-ui` + `pnpm build`.

## Task 2: URL detection module (with tests)

Create `apps/ccdash-ui/ui/src/lib/urlDetect.ts`:

```typescript
const LOCAL_URL_RE = /https?:\/\/(?:localhost|127\.0\.0\.1|0\.0\.0\.0|\[::1\])(:\d{1,5})?(?:\/[^\s\x1b]*)?/gi;

export function extractLocalUrls(text: string): string[] {
  const seen = new Set<string>();
  for (const m of text.matchAll(LOCAL_URL_RE)) {
    let url = m[0];
    // Strip trailing punctuation common in CLI output.
    url = url.replace(/[.,;:!?)\]}>]+$/, '');
    seen.add(url);
  }
  return [...seen];
}
```

(Tests would live in `apps/ccdash-ui/ui/src/lib/urlDetect.test.ts` if we had a Vitest setup — we don't yet. Skipping unit test for v0.5; pure regex is review-only.)

## Task 3: Store + detection wiring

- Add `detectedUrls = writable<Set<string>>(new Set())` to stores.ts.
- In `Terminal.svelte`'s output listener, decode the bytes and call `extractLocalUrls`, merging into the store:

```typescript
const text = new TextDecoder().decode(bytes);
const urls = extractLocalUrls(text);
if (urls.length > 0) {
  detectedUrls.update((s) => {
    const next = new Set(s);
    for (const u of urls) next.add(u);
    return next;
  });
}
```

- In App.svelte's onMount, after refreshing ports, also feed detectedUrls from `$ports.running` (synthesize `http://localhost:PORT` from each TCP listener).

## Task 4: BrowserView component

Layout:
```
+--- chrome bar ---+
| ← → ↻  [address bar]  Go   Open ↗ |
+--- detected list (collapsed left rail) | iframe pane ---+
| http://localhost:3000 (loanplatform)   |                  |
| http://localhost:5173 (vite default)   |    <iframe>      |
+----------------------------------------+------------------+
```

- Address bar: text input bound to `current` URL state.
- Back/Forward: small in-memory history stack.
- Reload: bump a `reloadCounter`; use it in `{#key}` block around the `<iframe>` so it remounts.
- "Open ↗" button: invoke shell.open(currentUrl).
- Left rail: list of `[...$detectedUrls]` sorted; clicking sets `current`.

## Task 5: Wire Browser tab in App.svelte

- Extend the `activeTab` union to include `'browser'`.
- Add a "Browser" button to the tab strip.
- Conditionally render `<BrowserView />`.
- A tiny dot badge on the Browser tab when `$detectedUrls.size > 0` and `$activeTab !== 'browser'`.

## Task 6: Workspace gate + release v0.5.0

(Same as Phase 6/7/8: version bump, release.sh, formula sha, tap push, gh release, brew verify, exec log, tags.)
