/** Truncate a long branch name with middle ellipsis. */
export function truncateBranch(name: string, max = 24): string {
  if (name.length <= max) return name;
  const head = Math.max(1, Math.floor((max - 1) * 0.55));
  const tail = Math.max(1, max - 1 - head);
  return `${name.slice(0, head)}…${name.slice(-tail)}`;
}
