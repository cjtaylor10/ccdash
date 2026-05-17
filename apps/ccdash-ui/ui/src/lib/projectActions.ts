import { open } from '@tauri-apps/plugin-dialog';
import { projects } from './stores';
import { projectsApi, tauri } from './tauri';

/** Open a folder picker and call `project.add` on the result. Updates the
 * `projects` store on success. Returns the message string of any error, or
 * null on success / cancel. */
export async function addProjectViaPicker(): Promise<string | null> {
  try {
    const picked = await open({ directory: true, multiple: false, title: 'Pick project directory' });
    if (!picked || typeof picked !== 'string') return null;
    await projectsApi.add(picked);
    const { projects: ps } = await tauri.projectList();
    projects.set(ps);
    return null;
  } catch (e) {
    return String(e);
  }
}
