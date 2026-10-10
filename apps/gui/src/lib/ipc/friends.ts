/** The account's Steam friends: who is online, who plays DayZ, and where. */
import { commands, run } from "./core";

/**
 * `client`: the running Steam client may be asked when the Web API cannot
 * answer. It shows DayZ running for a moment, so only while the Friends view
 * is open.
 */
export const steamFriends = (client: boolean) => run(commands.steamFriends(client));
