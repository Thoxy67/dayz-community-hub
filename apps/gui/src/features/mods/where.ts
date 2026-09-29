/**
 * Whose copy of a mod the launcher uses, in the four cases a player tells
 * apart: the launcher's own download, a Steam subscription (Steam keeps it
 * current), a copy in a Steam library the account is not subscribed to
 * (nobody updates it), or a Steam library copy while Steam cannot be asked.
 */
import type { InstalledModDto, SteamSubscriptionDto } from "$lib/ipc/types";

export type Where = "launcher" | "subscribed" | "unsubscribed" | "steam";

export function whereOf(
  mod: Pick<InstalledModDto, "source">,
  item: SteamSubscriptionDto | null | undefined,
  steamAnswered: boolean,
): Where {
  if (mod.source === "launcher") return "launcher";
  if (!steamAnswered) return "steam";
  return item?.subscribed ? "subscribed" : "unsubscribed";
}
