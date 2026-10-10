/** What the list shortcuts do to a server that the lists share. */
import { words } from "$lib/i18n";
import { copyText } from "$lib/ipc/native";
import { errorText, say } from "$lib/stores/say";

/** C: the address to paste into the game or a chat, "ip:port". */
export async function copyAddress(ip: string, gamePort: number) {
  const address = `${ip}:${gamePort}`;
  try {
    await copyText(address);
    say.ok(`${words("common").copied} · ${address}`);
  } catch (e) {
    say.err(errorText(e));
  }
}
