<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { useIntlayer } from "svelte-intlayer";
  import type { Component } from "svelte";
  import TriangleAlert from "~icons/lucide/triangle-alert";
  import { initI18n } from "$lib/i18n";
  import { TooltipProvider } from "$lib/components/ui/tooltip";
  import { Toaster } from "$lib/components/ui/toast";
  import { Button } from "$lib/components/ui/button";
  import { Spinner } from "$lib/components/ui/spinner";
  import { Topo } from "$lib/components/ui/topo";
  import { inTauri } from "$lib/ipc/core";
  import { getCliArgs } from "$lib/ipc/system";
  import type { CliArgs } from "$lib/ipc/types";
  import { theme } from "$lib/theme/theme.svelte";
  import { app, type ViewId } from "$lib/stores/app.svelte";
  import { servers } from "$lib/stores/servers.svelte";
  import { mods } from "$lib/stores/mods.svelte";
  import { profile } from "$lib/stores/profile.svelte";
  import { connect } from "$lib/stores/connect.svelte";
  import { say } from "$lib/stores/say";
  import { words } from "$lib/i18n";
  import { PLACES } from "$shell/nav";
  import TitleBar from "$shell/TitleBar.svelte";
  import Rail from "$shell/Rail.svelte";
  import StatusBar from "$shell/StatusBar.svelte";
  import ConfirmHost from "$shell/ConfirmHost.svelte";
  import ConnectDialog from "$shell/ConnectDialog.svelte";

  initI18n();
  const s = useIntlayer("shell");

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

  // ── keyboard ────────────────────────────────────────────────────────────
  function onkeydown(e: KeyboardEvent) {
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
    void app.init();
    if (!inTauri) return;
    const off: Array<() => void> = [];
    const on = <T,>(ev: string, fn: (p: T) => void) =>
      listen<T>(ev, (e) => fn(e.payload)).then((u) => off.push(u));

    void on<string>("launch-done", (name) => {
      say.ok(words("shell").statusLaunched({ name }));
      void profile.load();
    });
    void on<string>("launch-error", (error) => say.err(words("shell").statusLaunchError({ error })));
    void on<CliArgs>("cli-args", (a) => connect.cli(a));
    void getCliArgs()
      .then((a) => (a.connect || a.reconnect || a.open) && connect.cli(a))
      .catch(() => {});

    const win = getCurrentWindow();
    const syncMax = () => win.isMaximized().then((m) => (app.maximized = m));
    void syncMax();
    void win.onResized(syncMax).then((u) => off.push(u));
    const blur = () => app.away();
    const focus = () => void app.back();
    const vis = () => (document.hidden ? blur() : focus());
    window.addEventListener("blur", blur);
    window.addEventListener("focus", focus);
    document.addEventListener("visibilitychange", vis);

    // Mods are checked for updates every half hour while the app is open.
    const tick = setInterval(() => void mods.checkUpdates(true), 30 * 60 * 1000);
    return () => {
      off.forEach((u) => u());
      window.removeEventListener("blur", blur);
      window.removeEventListener("focus", focus);
      document.removeEventListener("visibilitychange", vis);
      clearInterval(tick);
    };
  });
</script>

<svelte:window {onkeydown} />

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
      <main class="relative min-w-0 flex-1 bg-panel">
        {#if !app.initialized}
          <div class="relative grid h-full place-items-center overflow-hidden">
            <Topo opacity={0.6} />
            <div class="relative flex flex-col items-center gap-3 text-center">
              {#if app.initError}
                <TriangleAlert class="size-8 text-err" />
                <h1 class="m-0 title-display text-xl text-fg">{$s.initFailed.value}</h1>
                <p class="m-0 max-w-md font-mono text-2xs text-fg-muted" data-selectable>{app.initError}</p>
                <Button variant="accent" onclick={() => location.reload()}>{$s.initRetry.value}</Button>
              {:else}
                <Spinner class="size-7 text-accent" />
                <p class="m-0 label-stencil text-fg-muted">{$s.initLoading.value}</p>
              {/if}
            </div>
          </div>
        {:else}
          {#each opened as id (id)}
            <section class="absolute inset-0 flex flex-col" hidden={app.view !== id}>
              {#await viewOf(id) then mod}
                <mod.default />
              {/await}
            </section>
          {/each}
        {/if}
      </main>
    </div>
    <StatusBar />
  </div>

  <ConfirmHost />
  <ConnectDialog />
  {#if mods.op.active}
    {#await lazyModOp() then M}<M.default />{/await}
  {/if}
  {#if app.showWizard}
    {#await lazyWizard() then W}<W.default />{/await}
  {/if}
  <Toaster />
</TooltipProvider>
