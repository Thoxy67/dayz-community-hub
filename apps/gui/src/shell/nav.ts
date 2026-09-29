/**
 * Every place the window can show, in one list. The rail, the keyboard
 * shortcuts (Ctrl+1…9) and the view outlet all read it, so adding a view is
 * one entry here and a `View.svelte` in its feature folder.
 */
import type { Component } from "svelte";
import type nav from "$content/nav.content";
import type { ViewId } from "$lib/stores/app.svelte";
import IconServers from "~icons/lucide/server";
import IconFavorites from "~icons/lucide/star";
import IconHistory from "~icons/lucide/history";
import IconConnect from "~icons/lucide/plug-zap";
import IconOffline from "~icons/game-icons/camping-tent";
import IconMods from "~icons/lucide/puzzle";
import IconOptions from "~icons/lucide/sliders-horizontal";
import IconNews from "~icons/lucide/newspaper";
import IconSettings from "~icons/lucide/settings";
import IconAbout from "~icons/lucide/info";

export type Group = "play" | "gear" | "intel" | "foot";
type Label = keyof (typeof nav)["content"];

export type Place = {
  id: ViewId;
  group: Group;
  label: Label;
  icon: Component<{ class?: string }>;
  load: () => Promise<{ default: Component }>;
};

export const PLACES: readonly Place[] = [
  {
    id: "servers",
    group: "play",
    label: "servers",
    icon: IconServers,
    load: () => import("$features/servers/View.svelte"),
  },
  {
    id: "favorites",
    group: "play",
    label: "favorites",
    icon: IconFavorites,
    load: () => import("$features/favorites/View.svelte"),
  },
  {
    id: "history",
    group: "play",
    label: "history",
    icon: IconHistory,
    load: () => import("$features/history/View.svelte"),
  },
  {
    id: "connect",
    group: "play",
    label: "connect",
    icon: IconConnect,
    load: () => import("$features/connect/View.svelte"),
  },
  {
    id: "offline",
    group: "play",
    label: "offline",
    icon: IconOffline,
    load: () => import("$features/offline/View.svelte"),
  },
  {
    id: "mods",
    group: "gear",
    label: "mods",
    icon: IconMods,
    load: () => import("$features/mods/View.svelte"),
  },
  {
    id: "options",
    group: "gear",
    label: "options",
    icon: IconOptions,
    load: () => import("$features/options/View.svelte"),
  },
  {
    id: "news",
    group: "intel",
    label: "news",
    icon: IconNews,
    load: () => import("$features/news/View.svelte"),
  },
  {
    id: "settings",
    group: "foot",
    label: "settings",
    icon: IconSettings,
    load: () => import("$features/settings/View.svelte"),
  },
  {
    id: "about",
    group: "foot",
    label: "about",
    icon: IconAbout,
    load: () => import("$features/about/View.svelte"),
  },
];

export const GROUPS: readonly { id: Exclude<Group, "foot">; label: Label }[] = [
  { id: "play", label: "groupPlay" },
  { id: "gear", label: "groupGear" },
  { id: "intel", label: "groupIntel" },
];

export const placeOf = (id: ViewId) => PLACES.find((p) => p.id === id)!;
