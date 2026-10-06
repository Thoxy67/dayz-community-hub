<script lang="ts">
  import { untrack } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { dict } from "$lib/i18n";
  import type { Component } from "svelte";
  import TriangleAlert from "~icons/lucide/triangle-alert";
  import { initI18n } from "$lib/i18n";
  import { TooltipProvider } from "$lib/components/ui/tooltip";
  import { Toaster } from "$lib/components/ui/toast";
  import { Button } from "$lib/components/ui/button";
  import { Spinner } from "$lib/components/ui/spinner";
  import { Topo } from "$lib/components/ui/topo";
  import { events, inTauri } from "$lib/ipc/core";
  import { getCliArgs } from "$lib/ipc/system";
  import type { CliArgs } from "$lib/ipc/types";
  import { theme } from "$lib/theme/theme.svelte";
  import { app, type ViewId } from "$lib/stores/app.svelte";
  import { servers } from "$lib/stores/servers.svelte";
  import { mods } from "$lib/stores/mods.svelte";
  import { profile } from "$lib/stores/profile.svelte";
  import { game } from "$lib/stores/game.svelte";
  import { connect } from "$lib/stores/connect.svelte";
  import { say } from "$lib/stores/say";
  import { dialogs } from "$lib/stores/dialogs.svelte";
  import { prefs } from "$lib/stores/prefs.svelte";
  import { pad, start as startPad } from "$lib/gamepad";
  import { words } from "$lib/i18n";
  import { PLACES } from "$shell/nav";
  import TitleBar from "$shell/TitleBar.svelte";
  import Rail from "$shell/Rail.svelte";
  import StatusBar from "$shell/StatusBar.svelte";
  import ConfirmHost from "$shell/ConfirmHost.svelte";
  import PasswordHost from "$shell/PasswordHost.svelte";
  import ConnectDialog from "$shell/ConnectDialog.svelte";
  import CommandPalette from "$shell/CommandPalette.svelte";
  import ShortcutsDialog from "$shell/ShortcutsDialog.svelte";
  import PadHints from "$shell/PadHints.svelte";

  initI18n();
  const s = dict("shell");

  // ── theme and frame ─────────────────────────────────────────────────────
  $effect(() => theme.apply());
  const frame = $derived(theme.frame);
  const radius = $derived(app.maximized ? 0 : frame.radius);

  // ── views: loaded on first visit, then kept mounted so a list keeps its
  // scroll, its filters and its selection when the player comes back ──────
  let opened = $state<ViewId[]>([]);
  const loaded = new Map<ViewId, Promise<{ default: Component }>>();
  $effect(() => {
    if (!opened.includes(app.view)) opened = [...opened, app.view];
  });
  function viewOf(id: ViewId) {
    let p = loaded.get(id);
    if (!p) loaded.set(id, (p = PLACES.find((x) => x.id === id)!.load()));
    return p;
  }

  const lazyModOp = () => import("$features/mods/ModOpDialog.svelte");
  const lazyWizard = () => import("$features/setup/Wizard.svelte");

  // ── controllers: the pads' events, the focus ring, a larger interface ────
  $effect(() => untrack(() => startPad(PLACES.map((p) => p.id))));
  $effect(() => {
    document.documentElement.dataset.input = pad.mode;
  });
  // Drawn larger while a pad drives (a Steam Deck's 7-inch screen): the
  // webview's own zoom, so every measure in CSS pixels stays what it was.
  $effect(() => {
    const zoom = pad.mode === "gamepad" ? prefs.padScale : 1;
    if (inTauri)
      void getCurrentWebview()
        .setZoom(zoom)
        .catch(() => {});
  });

  // ── keyboard ────────────────────────────────────────────────────────────
  /**
   * What a browser would do with a key, which an app must not: reload (F5,
   * Ctrl+R in a text field, which WebView2 honours and which threw away a
   * running download's state), print, find, view source, caret browsing,
   * history back and forward. Kept in development, where reloading helps.
   */
  function browserKey(e: KeyboardEvent): boolean {
    if (import.meta.env.DEV) return false;
    const k = e.key.toLowerCase();
    if (k === "f5" || k === "f3" || k === "f7" || k === "browserback" || k === "browserforward")
      return true;
    if (e.altKey && (k === "arrowleft" || k === "arrowright")) return true;
    return (e.ctrlKey || e.metaKey) && ["r", "p", "f", "g", "j", "s", "o"].includes(k);
  }

  function onkeydown(e: KeyboardEvent) {
    if (browserKey(e)) e.preventDefault();
    if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "k") {
      e.preventDefault();
      dialogs.palette = !dialogs.palette;
      return;
    }
    // `?` (Shift+/ or its own key): the sheet of shortcuts, when not typing.
    if (
      e.key === "?" &&
      !e.ctrlKey &&
      !e.altKey &&
      !e.metaKey &&
      !dialogs.open &&
      !(e.target as HTMLElement)?.closest("input, textarea, [contenteditable]")
    ) {
      e.preventDefault();
      dialogs.shortcuts = true;
      return;
    }
    if (!e.ctrlKey || e.shiftKey || e.altKey || e.metaKey) return;
    const typing = (e.target as HTMLElement)?.closest("input, textarea, [contenteditable]");
    const n = parseInt(e.key, 10);
    if (n >= 1 && n <= 9 && PLACES[n - 1]) {
      e.preventDefault();
      app.go(PLACES[n - 1]!.id);
    } else if (!typing && e.key === "r") {
      e.preventDefault();
      void servers.refresh();
    } else if (!typing && e.key === "u") {
      e.preventDefault();
      if (mods.stale.length > 0) {
        app.go("mods");
        mods.updateStale();
      } else say.ok(words("mods").upToDate);
    } else if (!typing && e.key === "l") {
      e.preventDefault();
      void connect.rejoin();
    }
  }

  // ── startup, the backend's events, the window ───────────────────────────
  $effect(() => {
    untrack(() => void app.init());
    game.init();
    if (!inTauri) return;
    // Listeners are registered asynchronously: one that arrives after the
    // effect was torn down is removed at once instead of leaking.
    let disposed = false;
    const off: Array<() => void> = [];
    const keep = (p: Promise<() => void>) =>
      void p.then((u) => (disposed ? u() : off.push(u))).catch(() => {});

    keep(
      events.launchDone.listen((e) => {
        say.ok(words("shell").statusLaunched({ name: e.payload }));
        game.launched();
        void profile.load();
      }),
    );
    keep(
      events.launchError.listen((e) =>
        say.err(words("shell").statusLaunchError({ error: e.payload })),
      ),
    );
    keep(events.cliArgs.listen((e) => void connect.cli(e.payload)));
    void getCliArgs()
      .then((a: CliArgs) => {
        if (a.connect || a.reconnect || a.open) void connect.cli(a);
      })
      .catch(() => {});

    // Whether the window is maximised, asked at most once a frame while it
    // is being resized.
    const win = getCurrentWindow();
    let raf = 0;
    const syncMax = () => {
      if (raf) return;
      raf = requestAnimationFrame(() => {
        raf = 0;
        void win.isMaximized().then((m) => (app.maximized = m));
      });
    };
    syncMax();
    keep(win.onResized(syncMax));
    const blur = () => app.away();
    const focus = () => void app.back();
    const vis = () => (document.hidden ? blur() : focus());
    window.addEventListener("blur", blur);
    window.addEventListener("focus", focus);
    document.addEventListener("visibilitychange", vis);

    // Mods are checked for updates every half hour while the app is open.
    const tick = setInterval(() => void mods.checkUpdates(true), 30 * 60 * 1000);
    return () => {
      disposed = true;
      off.forEach((u) => u());
      if (raf) cancelAnimationFrame(raf);
      window.removeEventListener("blur", blur);
      window.removeEventListener("focus", focus);
      document.removeEventListener("visibilitychange", vis);
      clearInterval(tick);
    };
  });
</script>

<svelte:window
  {onkeydown}
  onpaste={(e) => {
    // An address or a dzch:// link pasted outside a text field opens it in
    // Direct Connect: copied from Discord, a website, a friend.
    if ((e.target as HTMLElement)?.closest("input, textarea, [contenteditable]")) return;
    const text = e.clipboardData?.getData("text")?.trim() ?? "";
    if (text.startsWith("dzch://")) {
      e.preventDefault();
      void connect.openDzch(text);
      return;
    }
    const m = /^(\d{1,3}(?:\.\d{1,3}){3}|[a-z0-9-]+(?:\.[a-z0-9-]+)+):(\d{2,5})$/i.exec(text);
    const port = m ? Number(m[2]) : 0;
    if (m && port > 0 && port < 65536) {
      e.preventDefault();
      connect.openInDirect(m[1]!, port);
      say.info(words("connect").pastedAddress({ address: text }));
    }
  }}
  oncontextmenu={(e) => {
    // The webview's own menu (Back, Reload, Print, Inspect) is a browser's,
    // not this app's; text fields and selectable text keep theirs (copy, paste).
    if (import.meta.env.DEV) return;
    const t = e.target as HTMLElement | null;
    if (t?.closest("input, textarea, [contenteditable], [data-selectable]")) return;
    e.preventDefault();
  }}
/>

<TooltipProvider>
  <div
    class="flex h-screen w-screen flex-col overflow-hidden bg-bg text-fg"
    style:border-radius="{radius}px"
    style:border={frame.border > 0 && !app.maximized
      ? `${frame.border}px solid ${app.focused ? frame.borderFocus : frame.borderBlur}`
      : undefined}
  >
    <TitleBar />
    <div class="flex min-h-0 flex-1">
      <Rail />
      <main class="relative min-w-0 flex-1 bg-panel" data-pad-region>
        {#if !app.initialized}
          <div class="relative grid h-full place-items-center overflow-hidden">
            <Topo opacity={0.6} />
            <div class="relative flex flex-col items-center gap-3 text-center">
              {#if app.initError}
                <TriangleAlert class="size-8 text-err" />
                <h1 class="m-0 title-display text-xl text-fg">{$s.initFailed.value}</h1>
                <p class="m-0 max-w-md font-mono text-2xs text-fg-muted" data-selectable>
                  {app.initError}
                </p>
                <Button variant="accent" onclick={() => location.reload()}
                  >{$s.initRetry.value}</Button
                >
              {:else}
                <Spinner class="size-7 text-accent" />
                <p class="m-0 label-stencil text-fg-muted">{$s.initLoading.value}</p>
              {/if}
            </div>
          </div>
        {:else}
          {#each opened as id (id)}
            <section class="absolute inset-0 flex flex-col" hidden={app.view !== id} data-view={id}>
              {#await viewOf(id) then mod}
                <mod.default />
              {/await}
            </section>
          {/each}
        {/if}
      </main>
    </div>
    {#if pad.mode === "gamepad"}<PadHints />{/if}
    <StatusBar />
  </div>

  <ConfirmHost />
  <PasswordHost />
  <ConnectDialog />
  <CommandPalette />
  <ShortcutsDialog />
  {#if mods.op.active}
    {#await lazyModOp() then M}<M.default />{/await}
  {/if}
  {#if app.showWizard}
    {#await lazyWizard() then W}<W.default />{/await}
  {/if}
  <Toaster />
</TooltipProvider>
