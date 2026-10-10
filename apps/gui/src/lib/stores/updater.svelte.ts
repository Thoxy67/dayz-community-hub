/**
 * The launcher's own updates. A Windows zip and an AppImage replace
 * themselves (downloaded, checked against the signing key, then a restart);
 * a distribution package or a dev build is only told a new version exists,
 * with the reason it cannot install it here.
 */
import {
  checkForUpdate,
  installUpdate,
  updateDownloadState,
  updateSupport,
  type DownloadEvent,
  type UpdateInfo,
  type UpdateSupport,
} from "$lib/ipc/system";
import { restartApp } from "$lib/ipc/profile";
import { Channel, errorText } from "$lib/ipc/core";

/** How often a download in flight is asked about after a reload. */
const POLL_MS = 250;

export type UpdateState =
  "idle" | "checking" | "up_to_date" | "available" | "downloading" | "done" | "error";

class Updater {
  state = $state<UpdateState>("idle");
  info = $state.raw<UpdateInfo | null>(null);
  error = $state("");
  received = $state(0);
  total = $state(0);
  percent = $derived(this.total > 0 ? Math.round((this.received / this.total) * 100) : 0);
  /** Whether this copy can install an update itself; null until asked. */
  support = $state<UpdateSupport | null>(null);

  async check() {
    this.state = "checking";
    this.error = "";
    try {
      // Both questions at once: neither waits on the other.
      const support = this.support
        ? null
        : updateSupport()
            .then((s) => (this.support = s))
            .catch(() => {});
      this.info = await checkForUpdate();
      await support;
      this.state = this.info ? "available" : "up_to_date";
    } catch (e) {
      this.error = errorText(e);
      this.state = "error";
    }
  }

  async install() {
    if (this.support && !this.support.supported) return;
    this.state = "downloading";
    this.received = 0;
    this.total = 0;
    this.error = "";
    const ch = new Channel<DownloadEvent>();
    ch.onmessage = (ev) => {
      if (ev.event === "Started") this.total = ev.data.contentLength ?? 0;
      else if (ev.event === "Progress") this.received += ev.data.chunkLength;
      else this.state = "done";
    };
    try {
      await installUpdate(ch);
    } catch (e) {
      this.error = errorText(e);
      this.state = "error";
    }
  }

  /**
   * At startup. A download the backend runs (the window reloaded during it)
   * is shown and followed, not started again: its channel died with the old
   * page, so its progress is asked for until it ends. Otherwise, a check.
   */
  async resume() {
    try {
      const d = await updateDownloadState();
      if (d.running || d.done) {
        // No check meanwhile: on Windows it clears the last update's
        // leftovers, which a download in flight may be writing.
        this.#followDownload(d);
        return;
      }
    } catch {
      // Asked of an older backend, or failed: a plain check.
    }
    await this.check();
  }

  #followDownload(d: Awaited<ReturnType<typeof updateDownloadState>>) {
    this.received = d.received;
    this.total = d.total ?? 0;
    if (d.running) {
      this.state = "downloading";
      setTimeout(
        () =>
          void updateDownloadState()
            .then((n) => this.#followDownload(n))
            .catch(() => {}),
        POLL_MS,
      );
    } else if (d.done) {
      this.state = "done";
    } else if (d.error) {
      this.error = d.error;
      this.state = "error";
    }
  }

  /** Start the new version: the backend relaunches the replaced executable or AppImage. */
  async restart() {
    try {
      await restartApp();
    } catch (e) {
      this.error = errorText(e);
      this.state = "error";
    }
  }
}

export const updater = new Updater();
