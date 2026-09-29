/**
 * What each launch option is: its flag, its group, its icon, the words that
 * name it, and whether it takes a value. The profile only carries keys,
 * on/off and a value; everything a person reads comes from here.
 */
import type { Component } from "svelte";
import type options from "$content/options.content";
import type { SystemSpecsDto } from "$lib/ipc/types";
import Monitor from "~icons/lucide/monitor";
import Rocket from "~icons/lucide/rocket";
import Gauge from "~icons/lucide/gauge";
import Globe from "~icons/lucide/globe";
import Code from "~icons/lucide/code-xml";
import AppWindow from "~icons/lucide/app-window";
import Frame from "~icons/lucide/frame";
import ImageOff from "~icons/lucide/image-off";
import SkipForward from "~icons/lucide/skip-forward";
import PlaneTakeoff from "~icons/lucide/square-arrow-out-up-right";
import ArrowUpNarrowWide from "~icons/lucide/arrow-up-narrow-wide";
import MemoryStick from "~icons/lucide/memory-stick";
import Cpu from "~icons/lucide/cpu";
import Layers from "~icons/lucide/layers";
import ChartNoAxes from "~icons/lucide/chart-no-axes-column";
import MapIcon from "~icons/lucide/map";
import CirclePause from "~icons/lucide/circle-pause";
import FileCode from "~icons/lucide/file-code";
import ScrollText from "~icons/lucide/scroll-text";
import Bug from "~icons/lucide/bug";
import Construction from "~icons/lucide/construction";
import Tv from "~icons/lucide/tv";
import FolderOpen from "~icons/lucide/folder-open";
import SlidersHorizontal from "~icons/lucide/sliders-horizontal";
import MonitorCog from "~icons/lucide/monitor-cog";

type Content = (typeof options)["content"];
/** Keys of plain words (not the ones with `{{…}}` inserts). */
export type Word = { [K in keyof Content]: Content[K] extends { nodeType: "insertion" } ? never : K }[keyof Content];

/** A word from the options dictionary chosen at run time. */
export function wordOf(dict: unknown, key: Word): string {
  const node = (dict as Record<string, { value?: string } | undefined>)[key];
  return node?.value ?? String(key);
}
type Icon = Component<{ class?: string }>;

/** How a value is typed in. */
export type ValueKind = "none" | "mb" | "count" | "threads" | "text" | "folder";

export type OptionMeta = {
  flag: string;
  label: Word;
  desc: Word;
  icon: Icon;
  value: ValueKind;
};

export const META: Record<string, OptionMeta> = {
  window: { flag: "-window", label: "labelWindowed", desc: "descWindow", icon: AppWindow, value: "none" },
  noborder: { flag: "-noborder", label: "labelBorderless", desc: "descNoborder", icon: Frame, value: "none" },
  nosplash: { flag: "-nosplash", label: "labelNosplash", desc: "descNosplash", icon: ImageOff, value: "none" },
  skipintro: { flag: "-skipIntro", label: "labelSkipintro", desc: "descSkipintro", icon: SkipForward, value: "none" },
  nolauncher: { flag: "-nolauncher", label: "labelNolauncher", desc: "descNolauncher", icon: PlaneTakeoff, value: "none" },
  high: { flag: "-high", label: "labelHigh", desc: "descHigh", icon: ArrowUpNarrowWide, value: "none" },
  max_mem: { flag: "-maxMem", label: "labelMaxMem", desc: "descMaxMem", icon: MemoryStick, value: "mb" },
  max_vram: { flag: "-maxVRAM", label: "labelMaxVram", desc: "descMaxVram", icon: MonitorCog, value: "mb" },
  cpu_count: { flag: "-cpuCount", label: "labelCpuCount", desc: "descCpuCount", icon: Cpu, value: "count" },
  ex_threads: { flag: "-exThreads", label: "labelExThreads", desc: "descExThreads", icon: Layers, value: "threads" },
  no_benchmark: { flag: "-noBenchmark", label: "labelNoBenchmark", desc: "descNoBenchmark", icon: ChartNoAxes, value: "none" },
  world: { flag: "-world", label: "labelWorld", desc: "descWorld", icon: MapIcon, value: "text" },
  no_pause: { flag: "-noPause", label: "labelNoPause", desc: "descNoPause", icon: CirclePause, value: "none" },
  file_patching: { flag: "-filePatching", label: "labelFilePatching", desc: "descFilePatching", icon: FileCode, value: "none" },
  do_logs: { flag: "-doLogs", label: "labelDoLogs", desc: "descDoLogs", icon: ScrollText, value: "none" },
  script_debug: { flag: "-scriptDebug", label: "labelScriptDebug", desc: "descScriptDebug", icon: Bug, value: "text" },
  buldozer: { flag: "-buldozer", label: "labelBuldozer", desc: "descBuldozer", icon: Construction, value: "none" },
  winxp: { flag: "-winxp", label: "labelWinxp", desc: "descWinxp", icon: Tv, value: "none" },
  profiles: { flag: "-profiles", label: "labelProfiles", desc: "descProfiles", icon: FolderOpen, value: "folder" },
};

export type Group = { id: string; label: Word; icon: Icon; tone: string; keys: string[] };

export const GROUPS: Group[] = [
  { id: "performance", label: "groupPerformance", icon: Gauge, tone: "text-accent", keys: ["high", "max_mem", "max_vram", "cpu_count", "ex_threads", "no_benchmark"] },
  { id: "window", label: "groupWindow", icon: Monitor, tone: "text-info", keys: ["window", "noborder"] },
  { id: "startup", label: "groupStartup", icon: Rocket, tone: "text-ok", keys: ["nosplash", "skipintro", "nolauncher"] },
  { id: "world", label: "groupWorld", icon: Globe, tone: "text-map", keys: ["world", "no_pause"] },
  { id: "developer", label: "groupDeveloper", icon: Code, tone: "text-mods", keys: ["file_patching", "do_logs", "script_debug", "buldozer", "winxp", "profiles"] },
];

export const OTHER: Group = { id: "other", label: "groupOther", icon: SlidersHorizontal, tone: "text-fg-muted", keys: [] };

/** The flag as DayZ receives it: `-maxMem=8192`, or `-window`. */
export function flagText(key: string, value: string | null): string {
  const flag = META[key]?.flag ?? `-${key}`;
  return value ? `${flag}=${value}` : flag;
}

export type Recommendation = { key: string; value?: string };

/**
 * Tuned flags for the detected hardware: physical cores for -cpuCount (the
 * engine favours physical over logical), the three worker threads (7 =
 * file + geometry + texture) on four cores or more, about 60 % of memory for
 * -maxMem clamped to 2–16 GB, and the two broadly safe wins.
 */
export function recommend(s: SystemSpecsDto): Recommendation[] {
  const cpu = Math.max(1, Math.min(s.physical_cores, s.logical_cores));
  const exThreads = cpu >= 4 ? "7" : cpu >= 2 ? "3" : "1";
  const maxMem = Math.min(16384, Math.max(2048, Math.round((s.total_memory_mb * 0.6) / 256) * 256));
  return [
    { key: "cpu_count", value: String(cpu) },
    { key: "ex_threads", value: exThreads },
    { key: "max_mem", value: String(maxMem) },
    { key: "high" },
    { key: "no_benchmark" },
  ];
}
