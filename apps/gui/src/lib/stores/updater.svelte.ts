/**
 * The launcher's own updates. A Windows zip and an AppImage replace
 * themselves (downloaded, checked against the signing key, then a restart);
 * a distribution package or a dev build is only told a new version exists,
 * with the reason it cannot install it here.
 */
import {
  checkForUpdate,
  installUpdate,
  updateSupport,
  type DownloadEvent,
  type UpdateInfo,
  type UpdateSupport,
} from "$lib/ipc/system";
import { restartApp } from "$lib/ipc/profile";
import { Channel, errorText } from "$lib/ipc/core";

export type UpdateState = "idle" | "checking" | "up_to_date" | "available" | "downloading" | "done" | "error";

class Updater {
  state = $state<UpdateState>("idle");
  info = $state<UpdateInfo | null>(null);
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
      this.support ??= await updateSupport().catch(() => null);
      this.info = await checkForUpdate();
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
