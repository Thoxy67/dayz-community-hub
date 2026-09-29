/**
 * The launcher's own updates. On Windows it downloads and installs in place;
 * elsewhere the backend reports that the package manager does it, and this
 * store only says a new version exists.
 */
import { checkForUpdate, installUpdate, type DownloadEvent, type UpdateInfo } from "$lib/ipc/system";
import { Channel, errorText } from "$lib/ipc/core";

export type UpdateState = "idle" | "checking" | "up_to_date" | "available" | "downloading" | "done" | "error";

class Updater {
  state = $state<UpdateState>("idle");
  info = $state<UpdateInfo | null>(null);
  error = $state("");
  received = $state(0);
  total = $state(0);
  percent = $derived(this.total > 0 ? Math.round((this.received / this.total) * 100) : 0);

  async check() {
    this.state = "checking";
    this.error = "";
    try {
      this.info = await checkForUpdate();
      this.state = this.info ? "available" : "up_to_date";
    } catch (e) {
      this.error = errorText(e);
      this.state = "error";
    }
  }

  async install() {
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
}

export const updater = new Updater();
