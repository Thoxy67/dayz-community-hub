/**
 * Things that happened, said briefly and then gone.
 *
 * For the result of something the reader just did and could not otherwise
 * see: a file was written, a theme was copied, an import was refused. Not for
 * anything that matters in a minute's time, which belongs in the console.
 */
export type Toast = {
  id: number;
  tone: "ok" | "warn" | "err" | "neutral";
  text: string;
  /** A button on the toast: "Show file", "Undo". */
  action?: { label: string; run: () => void };
};

class Toasts {
  list = $state.raw<Toast[]>([]);
  #next = 1;
  #timers = new Map<number, ReturnType<typeof setTimeout>>();

  /** Say something. Stays four seconds, or seven when it has a button. */
  say(text: string, tone: Toast["tone"] = "neutral", action?: Toast["action"]): number {
    const id = this.#next++;
    // The newest four: a burst of exports should not paper over the window.
    this.list = [...this.list.slice(-3), { id, tone, text, ...(action ? { action } : {}) }];
    this.#timers.set(
      id,
      setTimeout(() => this.dismiss(id), action ? 7000 : 4000),
    );
    return id;
  }

  ok = (text: string, action?: Toast["action"]) => this.say(text, "ok", action);
  warn = (text: string, action?: Toast["action"]) => this.say(text, "warn", action);
  err = (text: string, action?: Toast["action"]) => this.say(text, "err", action);

  dismiss(id: number) {
    clearTimeout(this.#timers.get(id));
    this.#timers.delete(id);
    this.list = this.list.filter((t) => t.id !== id);
  }
}

export const toasts = new Toasts();
