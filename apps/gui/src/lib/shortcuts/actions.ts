/**
 * What the app-wide shortcuts do, by id. The shell provides them once
 * (`shell/keys.ts`); the keyboard handler, the controller's Menu/View/L3
 * and the command palette all run them from here, so a shortcut does the
 * same thing whichever way it is reached.
 */
const handlers = new Map<string, () => void>();

/** Say what shortcut `id` does. Returns the undo. */
export function provide(id: string, run: () => void): () => void {
  handlers.set(id, run);
  return () => {
    if (handlers.get(id) === run) handlers.delete(id);
  };
}

/** Do what shortcut `id` does; false when nothing provides it. */
export function runShortcut(id: string): boolean {
  const h = handlers.get(id);
  if (!h) return false;
  h();
  return true;
}
