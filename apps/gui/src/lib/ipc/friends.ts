/** The account's Steam friends: who is online, who plays DayZ, and where. */
import { commands, run } from "./core";

export const steamFriends = () => run(commands.steamFriends());
