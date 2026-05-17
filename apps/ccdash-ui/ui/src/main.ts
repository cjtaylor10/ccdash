import { mount } from 'svelte';
import { invoke } from '@tauri-apps/api/core';
import App from './App.svelte';
import './lib/theme.css';

// Sanity log + global error capture before mounting.
invoke('log_from_frontend', { level: 'info', message: 'main.ts starting' })
  .catch((e) => console.error('log failed', e));

window.addEventListener('error', (e) => {
  invoke('log_from_frontend', {
    level: 'error',
    message: `window.error: ${e.message} @ ${e.filename}:${e.lineno}:${e.colno}`,
  }).catch(() => {});
});
window.addEventListener('unhandledrejection', (e) => {
  invoke('log_from_frontend', {
    level: 'error',
    message: `unhandledrejection: ${String(e.reason)}`,
  }).catch(() => {});
});

const target = document.getElementById('app');
if (!target) {
  invoke('log_from_frontend', {
    level: 'error',
    message: 'no #app element found',
  }).catch(() => {});
} else {
  invoke('log_from_frontend', {
    level: 'info',
    message: 'mounting App into #app',
  }).catch(() => {});
  mount(App, { target });
}
