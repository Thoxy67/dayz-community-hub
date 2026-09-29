/**
 * The one question the app can ask at a time: "are you sure". Actions await
 * `confirm(...)`; the shell draws whatever is pending.
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

class Dialogs {
  pending = $state<Pending | null>(null);

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
}

export const dialogs = new Dialogs();
export const confirm = (req: ConfirmRequest) => dialogs.confirm(req);
