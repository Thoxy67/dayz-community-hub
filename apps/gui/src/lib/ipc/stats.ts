/** Play statistics: the figures and the full history of play sessions. */
import { commands, run } from "./core";
import type { StatsRange } from "./types";

/** The player's offset from UTC in minutes, east positive. */
const utcOffset = () => -new Date().getTimezoneOffset();

export const playStats = (range: StatsRange) => run(commands.playStats(range, utcOffset()));
export const playSessions = (search: string, offset: number, limit: number) =>
  run(commands.playSessions(search, offset, limit));
export const deleteSession = (start: number, name: string) =>
  run(commands.deleteSession(start, name));
