/**
 * A pretend backend, for looking at the interface in a plain browser
 * (`bun run dev`, then open the page): screenshots, design work. Installed by
 * `main.ts` in development only, and only when the page is not inside Tauri.
 * The data is invented but shaped like the real thing.
 */
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";

const MAPS = [
  "chernarusplus",
  "enoch",
  "sakhal",
  "deerisle",
  "namalsk",
  "banov",
  "pripyat",
  "esseker",
  "takistanplus",
];
const TAGS = [
  "PVP",
  "PVE",
  "1PP",
  "3PP",
  "Loot x2",
  "Trader",
  "Raid Weekends",
  "Vanilla+",
  "Hardcore",
  "KOTH",
  "Airdrops",
];
const REGIONS = ["EU", "US", "RU", "UK", "DE", "FR", "AU", "PL", "NA-East"];

let seed = 42;
const rnd = () => (seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648;
const pick = <T>(a: readonly T[]) => a[Math.floor(rnd() * a.length)]!;

const servers = Array.from({ length: 9500 }, (_, i) => {
  const max = pick([40, 50, 60, 60, 60, 80, 100, 127]);
  const players = rnd() < 0.3 ? 0 : rnd() < 0.1 ? max : Math.floor(rnd() * max);
  const ip = `${45 + (i % 180)}.${(i * 7) % 255}.${(i * 13) % 255}.${(i * 31) % 250}`;
  const map = pick(MAPS);
  const fp = rnd() < 0.35;
  return {
    game_port: 2302 + (i % 5) * 100,
    ip,
    query_port: 27016 + (i % 5) * 100,
    name: `[${pick(REGIONS)}] ${pick(["Frontline", "Wasteland", "Survivor", "Nomad", "Blackout", "Last Light", "Iron Wolves", "Dead Frequency"])} ${map === "chernarusplus" ? "Chernarus" : map} | ${pick(TAGS)} | ${pick(TAGS)}${fp ? " | 1PP" : ""}`,
    map,
    players,
    max_players: max,
    environment: rnd() < 0.8 ? "w" : "l",
    password: rnd() < 0.06,
    version: "1.28.162391",
    first_person_only: fp,
    time: `${String(Math.floor(rnd() * 24)).padStart(2, "0")}:${String(Math.floor(rnd() * 60)).padStart(2, "0")}`,
    mods_count: rnd() < 0.35 ? 0 : Math.floor(rnd() * 40),
    vac: true,
    battl_eye: rnd() < 0.9,
    official: false,
    mimics_official: false,
  };
});

// Twenty official servers, and between them a few community ones wearing their names.
const OFFICIAL_REGIONS = [
  "EUROPE - DE",
  "EUROPE - FR",
  "EUROPE - GB",
  "UNITED STATES - NY",
  "AUSTRALIA - SYD",
];
servers.forEach((x, i) => {
  if (i >= 26) return;
  if (i % 4 !== 3) {
    const map = ["chernarusplus", "enoch", "sakhal"][i % 3]!;
    Object.assign(x, {
      name: `${4100 + i * 7} | ${OFFICIAL_REGIONS[i % 5]}${x.first_person_only ? " | 1st Person Only" : ""}`,
      map,
      password: false,
      mods_count: 0,
      battl_eye: true,
      official: true,
    });
  } else {
    x.name = `${4200 + i} | ${OFFICIAL_REGIONS[i % 5]} | x10 Loot`;
    x.mimics_official = true;
  }
});

const mods = Array.from({ length: 24 }, (_, i) => {
  const size = Math.floor(rnd() * 2_000_000_000) + 2_000_000;
  const local = 1_750_000_000 + Math.floor(rnd() * 5_000_000);
  const stale = i % 6 === 0;
  return {
    name: [
      "CF",
      "Community-Online-Tools",
      "Dabs Framework",
      "VPPAdminTools",
      "Expansion-Core",
      "BuilderItems",
      "MuchCarKey",
      "Code Lock",
      "BaseBuildingPlus",
      "DayZ-Expansion-Map",
      "SchanaModParty",
      "Trader",
      "MMG Storage",
      "RedFalcon Heliz",
      "Airdrop-Upgraded",
      "Breachingcharge",
      "Server_Information_Panel",
      "GoreZ",
      "Survivor Animations",
      "WindstridesClothing",
      "Mass'sManyItemOverhaul",
      "CannabisPlus",
      "SNAFU Weapons",
      "DeerIsle",
    ][i]!,
    id: 1_559_212_036 + i * 97_331,
    local_updated: local,
    size,
    size_human: size > 1e9 ? `${(size / 1e9).toFixed(1)} GB` : `${(size / 1e6).toFixed(0)} MB`,
    managed: i % 5 !== 0,
    // Some in a Steam library (subscriptions), the rest the launcher's.
    source: i % 3 === 0 ? ("steam" as const) : ("launcher" as const),
    path:
      i % 3 === 0
        ? `/home/player/.local/share/Steam/steamapps/workshop/content/221100/${1_559_212_036 + i * 97_331}`
        : `/home/player/.local/share/dayz-community-hub/steamcmd-content/steamapps/workshop/content/221100/${1_559_212_036 + i * 97_331}`,
    other_copy: i === 3,
    // One the Workshop check has not reached (a hidden or removed item).
    remote_updated: i === 7 ? null : stale ? local + 86400 * (4 + i) : local,
    update_available: stale,
  };
});

const now = Math.floor(Date.now() / 1000);
const profile = {
  steam_login: "survivor_42",
  has_saved_password: false,
  steamcmd_logged_in: "survivor_42",
  steam_root: "/home/player/.local/share/Steam",
  steamcmd_enabled: true,
  // `?downloader=steamworks` starts on the Steam client.
  mod_downloader:
    new URLSearchParams(location.search).get("downloader") === "steamworks"
      ? "steamworks"
      : "steamcmd",
  steamcmd_path: null,
  player: "Survivor",
  steam_api_key: "XXXXXXXX",
  steam_id: "76561198000000000",
  user_location: [2.35, 48.85],
  favorites: servers
    .slice(3, 9)
    .map((s) => ({ name: s.name, ip: s.ip, port: s.query_port, password: null })),
  history: servers.slice(10, 22).map((s, i) => ({
    name: s.name,
    ip: s.ip,
    port: s.query_port,
    ts: now - i * 7200 - 600,
    relative_time: "",
  })),
  options: [
    ["window", "Run in windowed mode"],
    ["noborder", "Borderless window"],
    ["nosplash", "Skip splash screen"],
    ["skipintro", "Skip intro videos"],
    ["nolauncher", "Skip the Bohemia launcher"],
    ["file_patching", "Load unpacked files"],
    ["do_logs", "Write RPT logs"],
    ["high", "High process priority"],
    ["world", "World loaded at start"],
    ["no_pause", "Keep running when unfocused"],
    ["max_mem", "Maximum memory (MB)"],
    ["max_vram", "Maximum video memory (MB)"],
    ["cpu_count", "CPU cores to use"],
    ["ex_threads", "Extra threads mask"],
    ["no_benchmark", "Skip the benchmark"],
    ["script_debug", "Script debugging"],
    ["buldozer", "Buldozer mode"],
    ["winxp", "DirectX 9"],
    ["profiles", "Profile folder"],
  ].map(([key, description], i) => ({
    key,
    description,
    enabled: i % 3 === 0,
    value: key === "max_mem" ? "8192" : key === "world" ? "empty" : null,
  })),
  excluded_ips: [servers[40]!.ip],
  ping_concurrency: 64,
  ping_timeout_auto: 2000,
  ping_timeout_manual: 10000,
  ping_max_retries: 0,
  ping_scan_favorites: true,
  ping_scan_history: true,
  ping_scan_servers: true,
};

const articles = Array.from({ length: 8 }, (_, i) => ({
  title: [
    "Update 1.28 is live",
    "Status Report – September",
    "Frostline: what's next",
    "Community spotlight",
    "Server hosting changes",
    "Stable Update 1.27",
    "Winter event",
    "Console patch notes",
  ][i]!,
  slug: `article-${i}`,
  excerpt:
    "Survivors, a new update brings changes to vehicles, base building and the economy across every map.",
  content_text: "Lorem ipsum dolor sit amet.",
  content_html:
    "<p>Survivors, a new update brings changes to vehicles, base building and the economy.</p><h2>Vehicles</h2><p>Handling was reworked on every terrain type.</p>",
  date: new Date(Date.now() - i * 86400000 * 9).toISOString(),
  url: "https://dayz.com/article/updates/stable-update",
  image_url: null,
  category: i % 2 ? "Status Report" : "Updates",
  author: "Bohemia Interactive",
}));

type Ch<T> = { onmessage: (m: T) => void };

type Server = (typeof servers)[number];
const toRow = (x: Server) => {
  const k = `${x.ip}:${x.query_port}`;
  const ms = pingOf(k);
  return {
    ...x,
    bots: 0,
    ping_ms: ms > 270 ? 9999 : ms,
    ping_failed: ms > 270,
    favorite: profile.favorites.some((f) => f.ip === x.ip && f.port === x.query_port),
    excluded: profile.excluded_ips.includes(x.ip),
    unverified_full: false,
    // A pretend disk: a third of modded servers ready, the rest missing or behind a few.
    mods_missing:
      x.mods_count === 0 ? 0 : [0, 0, 3, 1, x.mods_count][x.ip.charCodeAt(x.ip.length - 1) % 5]!,
    mods_stale: x.mods_count === 0 ? 0 : [0, 2, 0, 1, 0][(x.ip.length + x.query_port) % 5]!,
  };
};

// What the backend does for real in `servers_query`, done here just enough for screenshots.
function queryServers(q: Record<string, unknown>) {
  const tri = (f: unknown, v: boolean) => f === "all" || (f === "only" ? v : !v);
  const search = String(q.search ?? "").toLowerCase();
  let list = servers.filter(
    (x) =>
      (!search ||
        x.name.toLowerCase().includes(search) ||
        x.ip.includes(search) ||
        x.map.includes(search)) &&
      (!q.map || x.map === q.map) &&
      tri(q.firstPerson, x.first_person_only) &&
      tri(q.password, x.password) &&
      tri(q.battleye, x.battl_eye) &&
      tri(q.modded, x.mods_count > 0) &&
      tri(q.official, x.official) &&
      (!q.hideEmpty || x.players > 0) &&
      (!q.hideFull || x.players < x.max_players) &&
      (q.showExcluded || !profile.excluded_ips.includes(x.ip)),
  );
  const dir = q.asc ? 1 : -1;
  const by: Record<string, (x: Server) => number | string> = {
    ping: (x) => pingOf(`${x.ip}:${x.query_port}`),
    players: (x) => x.players,
    name: (x) => x.name,
    map: (x) => x.map,
    mods: (x) => x.mods_count,
    time: (x) => x.time,
  };
  const f = by[String(q.sort)];
  if (f) list = [...list].sort((a, b) => (f(a) > f(b) ? dir : f(a) < f(b) ? -dir : 0));
  const offset = Number(q.offset ?? 0);
  const rows = list.slice(offset, offset + Number(q.limit ?? 100)).map(toRow);
  const pings = list.map((x) => pingOf(`${x.ip}:${x.query_port}`)).filter((m) => m <= 270);
  return {
    total: list.length,
    rows,
    stats: {
      shown: list.length,
      players: list.reduce((n, x) => n + x.players, 0),
      full: list.filter((x) => x.players > 0 && x.players >= x.max_players).length,
      empty: list.filter((x) => x.players === 0).length,
      modded: list.filter((x) => x.mods_count > 0).length,
      official: list.filter((x) => x.official).length,
      pinged: pings.length,
      best_ping: pings.length ? Math.min(...pings) : null,
    },
    generation: 1,
  };
}
const pingOf = (k: string) => 20 + ((k.length * 37 + k.charCodeAt(k.length - 1) * 11) % 260);

export function installMock() {
  // `?theme=chernarus` (or any preset id) and `?view=mods` pick what a screenshot shows.
  const q = new URLSearchParams(location.search);
  const t = q.get("theme");
  if (t) localStorage.setItem("dzch.theme", JSON.stringify({ preset: t, custom: null, frame: {} }));
  if (q.get("rail") === "collapsed")
    localStorage.setItem(
      "dzch.prefs",
      JSON.stringify({ panes: {}, railCollapsed: true, dismissedRejoin: null }),
    );
  // `?official=only` (or `none`) sets the servers view's official filter.
  const official = q.get("official");
  if (official === "only" || official === "none")
    void import("../../features/servers/filters.svelte").then(
      ({ filters }) => (filters.official = official),
    );
  // `?mods=none`: nothing installed, to look at the mods view's empty state.
  if (q.get("mods") === "none") mods.length = 0;
  // `?modop=1` starts a pretend SteamCMD operation, to look at its dialog.
  if (q.get("modop") === "1")
    setTimeout(async () => (await import("$lib/stores/mods.svelte")).mods.updateStale(), 1500);
  // `?pad=1` (`?pad=ps`: a PlayStation one) pretends a controller is connected and drives the window
  // (keyboard stand-ins, `&padseq=down,accept`): see lib/gamepad/mock.ts.
  const padKind = q.get("pad");
  if (padKind) void import("$lib/gamepad/mock").then(({ installMockPad }) => installMockPad(q));
  const v = q.get("view");
  if (v)
    queueMicrotask(async () =>
      (await import("$lib/stores/app.svelte")).app.go(v as never, q.get("focus")),
    );
  mockWindows("main");
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    switch (cmd) {
      case "check_first_launch":
        // `?wizard=1` opens the first-launch setup.
        return new URLSearchParams(location.search).get("wizard") === "1";
      case "initialize":
        return { server_count: servers.length, from_cache: false, is_first_launch: false };
      case "get_servers":
        return servers;
      case "refresh_servers":
        return servers.length;
      case "server_maps": {
        const m = new Map<string, number>();
        for (const x of servers) m.set(x.map, (m.get(x.map) ?? 0) + 1);
        return [...m].sort((a, b) => b[1] - a[1]).map(([map, count]) => ({ map, count }));
      }
      case "servers_query":
        return queryServers(a.query as Record<string, unknown>);
      case "servers_lookup":
        return (a.keys as string[]).map((k) => {
          const [ip, port] = k.split(":");
          const x = servers.find(
            (s) => s.ip === ip && (s.query_port === Number(port) || s.game_port === Number(port)),
          );
          return x ? toRow(x) : null;
        });
      case "start_scan": {
        const ch = a.onProgress as Ch<unknown>;
        let done = 0;
        const step = () => {
          done = Math.min(servers.length, done + 700);
          ch.onmessage({
            done,
            total: servers.length,
            paused: false,
            running: done < servers.length,
          });
          if (done < servers.length) setTimeout(step, 250);
        };
        setTimeout(step, 100);
        return null;
      }
      case "get_server_details": {
        const s = servers.find((x) => x.ip === a.ip);
        return {
          ...s,
          mods: mods
            .slice(0, s?.mods_count ?? 0)
            .map((m) => ({ name: m.name, steam_workshop_id: m.id })),
        };
      }
      case "get_app_stats":
        return {
          server_count: servers.length,
          total_players: servers.reduce((n, s) => n + s.players, 0),
          player_name: profile.player,
          steam_login: profile.steam_login,
          has_steamcmd: true,
        };
      case "fetch_steam_player_count":
        return 61_204;
      case "get_profile":
        return profile;
      case "get_installed_mods":
      case "check_mod_updates":
        return mods;
      case "mods_usage":
        // The first mods run by two favourites and a played server; a few by history only.
        return mods.slice(0, 14).map((m, i) => ({
          id: m.id,
          favorites: i < 6 ? profile.favorites.slice(0, 1 + (i % 3)).map((f) => f.name) : [],
          history: i % 2 === 0 ? [profile.history[0]?.name ?? "4170"] : [],
        }));
      case "steam_subscriptions": {
        // Most Steam-library mods subscribed (one not), one downloading, one
        // waiting, a launcher mod also subscribed, and two subscriptions not
        // on disk yet (one downloading, one waiting).
        const cycle = (Date.now() % 60_000) / 60_000;
        const item = (id: number, o: Record<string, unknown> = {}) => ({
          id,
          subscribed: true,
          installed: true,
          needs_update: false,
          downloading: false,
          pending: false,
          bytes_done: 0,
          bytes_total: 0,
          ...o,
        });
        const at = (i: number) => mods[i]?.id ?? 0;
        return {
          available: true,
          reason: null,
          items: [
            ...[0, 3, 6, 9, 21].map((i) => item(at(i))),
            item(at(1)),
            item(at(12), {
              needs_update: true,
              downloading: true,
              bytes_total: 1_400_000_000,
              bytes_done: Math.floor(1_400_000_000 * cycle),
            }),
            item(at(18), { needs_update: true, pending: true }),
            item(2_900_000_001, {
              installed: false,
              needs_update: true,
              downloading: true,
              bytes_total: 640_000_000,
              bytes_done: Math.floor(640_000_000 * cycle),
            }),
            item(2_900_000_002, { installed: false, needs_update: true }),
          ].filter((x) => x.id !== 0),
          // The first mod (CF-like) required by a few others, one of which
          // also needs a mod that is not installed.
          details: [
            { id: at(0), title: mods[0]?.name ?? "", requires: [] },
            { id: at(3), title: mods[3]?.name ?? "", requires: [at(0)] },
            { id: at(6), title: mods[6]?.name ?? "", requires: [at(0), 2_900_000_003] },
            { id: 2_900_000_001, title: "Incoming Weapons Pack", requires: [at(0)] },
            { id: 2_900_000_002, title: "Base Building Plus", requires: [] },
            { id: 2_900_000_003, title: "Missing Library", requires: [] },
          ].filter((d) => d.id !== 0),
        };
      }
      case "fetch_news":
        return articles;
      case "get_offline_missions":
        return [
          "DayZCommunityOfflineMode.ChernarusPlus",
          "DayZCommunityOfflineMode.Enoch",
          "DayZCommunityOfflineMode.Sakhal",
        ];
      case "gamepad_status":
        return {
          available: true,
          pads: padKind
            ? [
                padKind === "ps"
                  ? { name: "DualSense Wireless Controller", kind: "playStation" }
                  : { name: "Xbox Wireless Controller", kind: "xbox" },
              ]
            : [],
          steam_ui: false,
        };
      case "get_system_specs":
        return {
          logical_cores: 16,
          physical_cores: 8,
          total_memory_mb: 32768,
          cpu_name: "AMD Ryzen 7 5800X3D 8-Core Processor",
          os: "Linux 24.04 Ubuntu",
          gpus: [{ name: "NVIDIA GeForce RTX 4070", vram_mb: 12282 }],
        };
      case "get_cli_args":
        return { connect: null, reconnect: false, open: null };
      case "check_for_update":
        return {
          version: "0.5.0",
          currentVersion: "0.4.1",
          body: "- New interface",
          date: new Date().toISOString(),
        };
      case "detect_steamcmd":
        return { found: true, path: "/usr/bin/steamcmd", platform: "linux" };
      case "game_running":
        return new URLSearchParams(location.search).get("game") === "1";
      case "kill_game":
        return 2;
      case "steam_launch_info":
        return {
          linux: true,
          launcher: ["/usr/bin/steam"],
          applaunch: ["-applaunch", "221100", "-malloc=system", "-name=Survivor"],
          launch_options:
            "PROTON_USE_NTSYNC=1 PROTON_ENABLE_WAYLAND=1 RADV_PERFTEST=gpl,nggc,sam mangohud game-performance %command% -nolauncher",
          compat_tool: "proton-cachyos-native",
          compat_tool_default: false,
          prefix: "/mnt/ssd2/SteamLibrary/steamapps/compatdata/221100",
        };
      case "detect_dayz":
        return {
          steamapps: a.path || "/mnt/ssd2/SteamLibrary/steamapps",
          dayz_dir: `${a.path || "/mnt/ssd2/SteamLibrary/steamapps"}/common/DayZ`,
          workshop_mods: 37,
        };
      case "set_mod_downloader":
        profile.mod_downloader = a.downloader as typeof profile.mod_downloader;
        return null;
      case "steamworks_status":
        return { library: true, error: null, steam_running: true };
      case "steamworks_check":
        return null;
      case "steamcmd_dirs":
        return {
          content: "/home/player/.local/share/dayz-community-hub/steamcmd-content",
          home: "/home/player/.local/share/dayz-community-hub/steamcmd-home",
        };
      case "delete_mod":
        return mods.find((m) => m.id === a.modId)?.source === "steam";
      case "delete_mods_bulk":
        return (a.modIds as number[]).filter(
          (id) => mods.find((m) => m.id === id)?.source === "steam",
        );
      case "ping_all_background":
      case "ping_servers": {
        const targets = a.targets as string[];
        const ch = a.onProgress as Ch<unknown[]>;
        let i = 0;
        const step = () => {
          const batch = targets.slice(i, i + 200).map((k) => {
            const [ip, port] = k.split(":");
            const ms = pingOf(k);
            return { ip, port: Number(port), ms: ms > 270 ? 9999 : ms, failed: ms > 270 };
          });
          i += 200;
          ch.onmessage(batch);
          if (i < targets.length) setTimeout(step, 60);
        };
        setTimeout(step, 200);
        return null;
      }
      case "ping_single":
        return pingOf(`${a.ip}:${a.port}`);
      case "query_a2s": {
        const s = servers.find((x) => x.ip === a.ip);
        return {
          server_name: s?.name ?? "?",
          game: "DayZ",
          players: s?.players ?? 0,
          max_players: s?.max_players ?? 60,
          bots: 0,
          map: s?.map ?? "",
          version: s?.version ?? "",
          players_list: Array.from({ length: Math.min(12, s?.players ?? 0) }, (_, i) => ({
            name: `Survivor ${i + 1}`,
            score: 0,
            duration: 300 + i * 611,
          })),
          mods: [],
          // What DayZ puts in A2S: the plain rules, the mods from the binary
          // block and the keywords' settings. An unlisted server has only these.
          mods_from_a2s: s ? [] : ["Community Framework", "VPPAdminTools", "Dabs Framework"],
          mods_a2s: s
            ? []
            : [
                { name: "Community Framework", steam_workshop_id: 1559212036 },
                { name: "VPPAdminTools", steam_workshop_id: 1828439124 },
                { name: "Dabs Framework", steam_workshop_id: 2545327648 },
              ],
          dayz: {
            battleye: true,
            first_person_only: s?.first_person_only ?? false,
            official: false,
            private_hive: true,
            whitelisted: false,
            dlc: s?.map === "enoch" || s?.map === "sakhal",
            shard: "000",
            login_queue: (s?.players ?? 0) >= (s?.max_players ?? 60) ? 7 : 0,
            time_accel: 8,
            night_time_accel: 2,
            game_time: "14:09",
          },
          rules: [
            ["allowedBuild", "0"],
            ["clientPort", "0"],
            ["dedicated", "1"],
            ["island", s?.map ?? "chernarusplus"],
            ["language", "65545"],
            ["platform", "win"],
            ["requiredBuild", "0"],
            ["requiredVersion", "129"],
            ["timeLeft", "15"],
          ].map(([name, value]) => ({ name, value })),
          query_port: s?.query_port ?? 27016,
          game_port: s?.game_port ?? 2302,
        };
      }
      case "plugin:app|version":
        return "0.4.1";
      case "plugin:window|is_maximized":
        return false;
      case "start_mod_operation": {
        // A plausible SteamCMD run: log in (Steam stays open), then each mod
        // with live progress lines (rewritten in place), one failure, a summary.
        const ch = a.onProgress as Ch<Record<string, unknown>>;
        const ids =
          (a.modIds as number[] | undefined) ??
          mods.filter((m) => m.update_available).map((m) => m.id);
        const list = ids.length ? ids : mods.slice(0, 3).map((m) => m.id);
        const ev = (kind: string, extra: Record<string, unknown> = {}) =>
          ch.onmessage({
            kind,
            current: 0,
            total: list.length,
            mod_id: 0,
            name: "",
            ok: 0,
            failed: 0,
            hint: null,
            log_line: null,
            ...extra,
          });
        const log = (line: string, progress = false) =>
          ev(progress ? "log_progress" : "log_line", { log_line: line });
        const steps: Array<[number, () => void]> = [];
        let t = 0;
        const at = (dt: number, fn: () => void) => steps.push([(t += dt), fn]);
        if (profile.mod_downloader === "steamworks" && a.opType !== "login") {
          // What dz-steamworks reports: subscribe, Steam downloads, installed.
          at(200, () => log("Connected to Steam as DayZ (app 221100)"));
          list.forEach((id) => at(40, () => log(`Subscribing to ${id}`)));
          list.forEach((id) => at(80, () => log(`Subscribed ${id}`)));
          let ok = 0;
          list.forEach((id, i) => {
            const m = mods.find((x) => x.id === id);
            const name = m?.name ?? `Workshop ${id}`;
            const total = m?.size ?? 300_000_000;
            at(120, () => ev("starting", { current: i + 1, name, mod_id: id }));
            at(10, () => log(`Downloading item ${id} (${name}) through Steam`));
            at(40, () => log(`${id}: subscribed, needs update, downloading, download pending`));
            for (let k = 1; k <= 12; k++) {
              at(90, () => {
                const done = Math.floor((total * k) / 12);
                log(
                  `Steam: item ${id} downloading, progress: ${((done / total) * 100).toFixed(2)} (${done} / ${total})`,
                  true,
                );
              });
            }
            if (i === 1 && list.length > 2) {
              const error =
                "Download failed: access denied: the item is private, friends-only or hidden";
              at(80, () => log(`${id} failed: ${error}`));
              at(10, () =>
                ev("failed", { current: i + 1, name: `${name} (${error})`, mod_id: id }),
              );
            } else {
              ok++;
              at(60, () => log(`${id}: subscribed, installed`));
              at(10, () =>
                log(
                  `Installed ${id} at /home/player/.local/share/Steam/steamapps/workshop/content/221100/${id}`,
                ),
              );
              at(10, () => ev("done", { current: i + 1, name, mod_id: id }));
            }
          });
          const failed = list.length - ok;
          at(150, () => log("Disconnected from Steam"));
          at(20, () => ev("finished", { ok, failed }));
          for (const [when, fn] of steps) setTimeout(fn, when);
          return null;
        }
        at(300, () =>
          log(
            "Redirecting stderr to '/home/player/.local/share/dayz-community-hub/steamcmd-home/.steam/steamcmd/logs/stderr.txt'",
          ),
        );
        at(100, () => log("[  0%] Checking for available updates..."));
        at(100, () => log("[----] Verifying installation..."));
        at(100, () => log("Steam Console Client (c) Valve Corporation - version 1726604893"));
        at(100, () => log("Logging in user 'survivor_42' to Steam Public...OK"));
        at(80, () => log("Waiting for client config...OK"));
        at(80, () => log("Waiting for user info...OK"));
        at(10, () => ev("logged_in"));
        if (a.opType === "login") {
          at(200, () => log("Unloading Steam API...OK"));
          at(50, () => ev("finished", { ok: 1, failed: 0, total: 1 }));
          for (const [when, fn] of steps) setTimeout(fn, when);
          return null;
        }
        let ok = 0;
        list.forEach((id, i) => {
          const m = mods.find((x) => x.id === id);
          const name = m?.name ?? `Workshop ${id}`;
          const total = m?.size ?? 300_000_000;
          at(120, () => ev("starting", { current: i + 1, name, mod_id: id }));
          at(60, () => log(`Downloading item ${id} ...`));
          for (let k = 1; k <= 12; k++) {
            at(90, () => {
              const done = Math.floor((total * k) / 12);
              log(
                `Update state (0x61) downloading, progress: ${((done / total) * 100).toFixed(2)} (${done} / ${total})`,
                true,
              );
            });
          }
          if (i === 1 && list.length > 2) {
            at(80, () => log(`ERROR! Download item ${id} failed (Failure).`));
            at(20, () => ev("failed", { current: i + 1, name, mod_id: id }));
          } else {
            ok++;
            at(80, () =>
              log(
                `Success. Downloaded item ${id} to "/home/player/.local/share/dayz-community-hub/steamcmd-content/steamapps/workshop/content/221100/${id}" (${total} bytes)`,
              ),
            );
            at(20, () => ev("done", { current: i + 1, name, mod_id: id }));
          }
        });
        const failed = list.length - ok;
        at(200, () => log("Unloading Steam API...OK"));
        at(50, () => ev("finished", { ok, failed }));
        for (const [when, fn] of steps) setTimeout(fn, when);
        return null;
      }
      case "fetch_metrics_history": {
        const span = { "1d": 1, "7d": 7, "2w": 14, "1m": 30 }[a.range as string] ?? 1;
        const step = span <= 1 ? 300 : span <= 7 ? 1800 : 3600;
        const n = Math.floor((span * 86400) / step);
        const now = Math.floor(Date.now() / 1000);
        return Array.from({ length: n }, (_, i) => {
          const t = now - (n - 1 - i) * step;
          const d = new Date(t * 1000);
          const wave = 0.5 + 0.4 * Math.sin(((d.getUTCHours() - 6) / 24) * Math.PI * 2);
          const weekend = d.getUTCDay() % 6 === 0 ? 1.15 : 1;
          return [t, Math.round(60 * wave * weekend)];
        });
      }
      case "fetch_metrics_rank_history": {
        const today = Date.now();
        return Array.from({ length: 30 }, (_, i) => [
          new Date(today - (29 - i) * 86400e3).toISOString().slice(0, 10),
          40 - Math.round(i * 1.1),
        ]);
      }
      case "fetch_metrics_heatmap":
        return Array.from({ length: 168 }, (_, i) => {
          const dow = Math.floor(i / 24);
          const hour = i % 24;
          const wave = 0.5 + 0.45 * Math.sin(((hour - 13) / 24) * Math.PI * 2);
          return { dow, hour, avg: Math.round(70 * wave * (dow % 6 === 0 ? 1.2 : 1)) };
        });
      case "fetch_server_metrics": {
        const idx = servers.findIndex((x) => x.ip === a.ip);
        const s = servers[Math.max(0, idx)]!;
        const now = Math.floor(Date.now() / 1000);
        const fake = idx % 7 === 3;
        const hist = Array.from({ length: 288 }, (_, i) => {
          const t = now - (287 - i) * 300;
          const h = new Date(t * 1000).getUTCHours();
          const wave = 0.55 + 0.4 * Math.sin(((h - 6) / 24) * Math.PI * 2);
          return [t, Math.max(0, Math.round(s.max_players * wave * (fake ? 1 : 0.8)))] as [
            number,
            number,
          ];
        });
        return {
          id: 160155 + Math.max(0, idx),
          url: `https://dayzmetrics.com/server/${160155 + Math.max(0, idx)}`,
          name: s.name,
          map: s.map,
          version: s.version,
          status: "online",
          country: ["DE", "FR", "US", "RU", "GB"][Math.max(0, idx) % 5],
          players: s.players,
          max_players: s.max_players,
          queue: 0,
          rank_pos: 8 + Math.max(0, idx),
          rank_score: 4.0021,
          rank_alive: 0.888,
          rank_demand: 0.17,
          avg_players_7d: s.max_players * 0.45,
          peak_7d: s.max_players,
          uptime_7d: 99.3,
          wow_pct: fake ? -12 : 8,
          first_seen: "2026-05-24T07:17:31+00:00",
          last_seen: new Date().toISOString(),
          ping_lo: 6,
          ping_hi: 83,
          ping_jitter: 7,
          ping_stability: 0.9,
          time_accel: 8,
          night_time_accel: 2,
          restart: {
            period_hours: 3,
            last_restart: new Date(Date.now() - 2 * 3600e3).toISOString(),
            next_restart: new Date(Date.now() + 57 * 60e3).toISOString(),
            confidence: "high",
            slots_utc: ["00:00", "03:00", "06:00", "09:00", "12:00", "15:00", "18:00", "21:00"],
            coverage: 0.96,
            unscheduled_7d: 2,
            restart_loops_7d: 0,
            last_unscheduled: new Date(Date.now() - 3 * 86400e3).toISOString(),
          },
          wipe: {
            last: "2026-09-19",
            last_source: "announced",
            days_since: 10,
            next: "2026-10-17",
            next_source: "predicted",
            days_until: 18,
            period_days: 28,
            phase: "mid",
            confidence: 0.8,
            events: [
              { on: "2026-09-19", source: "announced", confidence: 1, corroborated: true },
              { on: "2026-08-22", source: "surge", confidence: 0.4, corroborated: false },
            ],
          },
          is_fake: fake,
          fake_reasons: fake
            ? ["Player count never drops below 90 %", "Same names reappear on every restart"]
            : [],
          behavior_verdict: fake ? "fake" : "real",
          behavior_score: fake ? 0.1 : 0.76,
          flagged: false,
          flag_reason: null,
          suspect: false,
          mimics_official: false,
          description: null,
          game_time: "14:20",
          game_time_at: new Date().toISOString(),
          discord: "https://discord.gg/example",
          website: null,
          links: [],
          notices: [],
          playstyle: null,
          vanilla_band: "Lightly Modded",
          vanilla_score: 37.9,
          mod_count: s.mods_count,
          mod_total_bytes: 5228339261,
          player_history: hist,
          ping_history: hist.map(([t]) => [t, 8]),
        };
      }
      case "toggle_ping_pause":
        return false;
      case "fetch_steam_avatar":
        return null;
      default:
        return null;
    }
  });
}
