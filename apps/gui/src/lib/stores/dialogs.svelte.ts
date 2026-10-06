/**
 * The questions the app can ask, one of each at a time: "are you sure", and
 * the password a server wants. Actions await `confirm(...)` or
 * `askPassword(...)`; the shell draws whatever is pending.
 */
export type ConfirmRequest = {
  title: string;
  message: string;
  /** Words on the button that acts: "Delete 3 mods", not "OK". */
  confirmLabel?: string;
  cancelLabel?: string;
  danger?: boolean;
};

type Pending = ConfirmRequest & { resolve: (yes: boolean) => void };

export type PasswordAnswer = { password: string; save: boolean };

type PendingPassword = { server: string; resolve: (a: PasswordAnswer | null) => void };

class Dialogs {
  pending = $state<Pending | null>(null);
  password = $state<PendingPassword | null>(null);
  /** The command palette (Ctrl+K). */
  palette = $state(false);
  /** The sheet of keyboard shortcuts (?). */
  shortcuts = $state(false);

  confirm(req: ConfirmRequest): Promise<boolean> {
    // A second question replaces the first, which is answered "no".
    this.pending?.resolve(false);
    return new Promise((resolve) => (this.pending = { ...req, resolve }));
  }

  answer(yes: boolean) {
    const p = this.pending;
    this.pending = null;
    p?.resolve(yes);
  }

  /** The password to join `server` with; `null` when the player gives up. */
  askPassword(server: string): Promise<PasswordAnswer | null> {
    this.password?.resolve(null);
    return new Promise((resolve) => (this.password = { server, resolve }));
  }

  answerPassword(a: PasswordAnswer | null) {
    const p = this.password;
    this.password = null;
    p?.resolve(a);
  }

  /** Something is being asked: keyboard shortcuts behind it stay quiet. */
  get open() {
    return this.pending !== null || this.password !== null || this.palette || this.shortcuts;
  }
}

export const dialogs = new Dialogs();
export const confirm = (req: ConfirmRequest) => dialogs.confirm(req);
export const askPassword = (server: string) => dialogs.askPassword(server);
