/**
 * BattlEye, which most servers require: its service on Windows, the Proton
 * runtime on Linux. Read by the status bar and the settings, repaired from
 * either.
 */
import type { BattlEyeStatus } from "$lib/ipc/bindings";
import { battleyeStatus, repairBattleye } from "$lib/ipc/launch";
import { words } from "$lib/i18n";
import { say, errorText } from "./say";

class BattlEye {
  status = $state<BattlEyeStatus | null>(null);
  repairing = $state(false);

  async refresh() {
    try {
      this.status = await battleyeStatus();
    } catch {
      this.status = null;
    }
  }

  /** Repair it; `clear` first deletes the game's data outside its folder. */
  async repair(clear = false) {
    const w = words("shell");
    this.repairing = true;
    try {
      const r = await repairBattleye(clear);
      this.status = r;
      if (r.state === "running" || r.state === "ready") say.ok(w.sbBeRepaired);
      else if (!r.service) say.info(w.sbBeInstalling);
      else say.warn(w.sbBeStillMissing);
    } catch (e) {
      say.err(w.sbBeRepairFailed({ error: errorText(e) }));
    } finally {
      this.repairing = false;
    }
  }
}

export const battleye = new BattlEye();
