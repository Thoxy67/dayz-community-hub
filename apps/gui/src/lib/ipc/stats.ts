/** Play statistics: the figures and the full history of play sessions. */
import { commands, run } from "./core";
import type { StatsRange } from "./types";

/** The player's offset from UTC in minutes, east positive. */
const utcOffset = () => -new Date().getTimezoneOffset();

export const playStats = (range: StatsRange) => run(commands.playStats(range, utcOffset()));
/** Sessions matching `search` that started in `from..to` (Unix seconds, either null). */
export const playSessions = (
  search: string,
  from: number | null,
  to: number | null,
  offset: number,
  limit: number,
) => run(commands.playSessions(search, from, to, offset, limit));
export const deleteSession = (start: number, name: string) =>
  run(commands.deleteSession(start, name));
