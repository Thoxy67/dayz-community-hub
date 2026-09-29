/**
 * What SteamCMD's own lines say about the download under way. Classifying
 * lines, the byte rate and the clock are shared (`$lib/components/app`).
 */

export type Progress = { percent: number; done: number; total: number };

// "Update state (0x61) downloading, progress: 42.17 (123456789 / 292837465)"
const PROGRESS = /progress:\s*([\d.]+)\s*\((\d+)\s*\/\s*(\d+)\)/i;

/** The byte progress a line reports, if it reports one. */
export function parseProgress(line: string): Progress | null {
  const m = PROGRESS.exec(line);
  return m ? { percent: Number(m[1]), done: Number(m[2]), total: Number(m[3]) } : null;
}
