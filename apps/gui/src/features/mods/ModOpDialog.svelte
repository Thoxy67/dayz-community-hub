<script lang="ts">
  import { Dialog } from "bits-ui";
  import { dict } from "$lib/i18n";
  import X from "~icons/lucide/x";
  import Minimize from "~icons/lucide/minimize-2";
  import Check from "~icons/lucide/check";
  import CircleX from "~icons/lucide/circle-x";
  import CircleCheck from "~icons/lucide/circle-check";
  import Loader from "~icons/lucide/loader-circle";
  import ShieldAlert from "~icons/lucide/shield-alert";
  import KeyRound from "~icons/lucide/key-round";
  import Eye from "~icons/lucide/eye";
  import EyeOff from "~icons/lucide/eye-off";
  import Send from "~icons/lucide/send";
  import ShieldOff from "~icons/lucide/shield-off";
  import { Button } from "$lib/components/ui/button";
  import { Meter } from "$lib/components/ui/meter";
  import { Input } from "$lib/components/ui/input";
  import { LogPane, TransferProgress, Rate, clock } from "$lib/components/app";
  import { cn } from "$lib/cx";
  import { bytes } from "$lib/format";
  import { copyText } from "$lib/ipc/native";
  import { steamcmdDirs } from "$lib/ipc/system";
  import { mods } from "$lib/stores/mods.svelte";
  import { profile } from "$lib/stores/profile.svelte";
  import TerminalIcon from "~icons/lucide/terminal";
  import { latestProgress } from "./steamcmd-log";

  /**
   * A download operation (SteamCMD's, or the Steam client's: both report the
   * same steps), as it happens: the phase it is in, how far the
   * current mod and the whole batch have got (bytes, speed, time left), what
   * has come through, and the downloader's own log streaming live beside it.
   *
   * Closing it while it runs only hides it: the operation carries on and
   * the status bar keeps showing it.
   */
  const c = dict("progress");
  const m = dict("mods");
  const op = $derived(mods.op);
  const running = $derived(op.phase !== "finished");

  // ── time ────────────────────────────────────────────────────────────────
  let now = $state(Date.now());
  $effect(() => {
    if (!running) return;
    const t = setInterval(() => (now = Date.now()), 500);
    return () => clearInterval(t);
  });
  const elapsed = $derived(now - op.startedAt);

  // ── the current mod's bytes, read from the log ──────────────────────────
  // The newest progress line since the current mod started ("Downloading item").
  const progress = $derived.by(() => {
    const p = latestProgress(op.log, op.logAt);
    return p && { ...p, at: op.startedAt + p.at };
  });
  // Speed needs the history of samples, which a derivation does not keep.
  const rate = new Rate();
  let speed = $state<number | null>(null);
  $effect(() => {
    if (!progress) {
      rate.reset();
      speed = null;
      return;
    }
    rate.add(progress.at, progress.done);
    speed = rate.perSecond;
  });
  const eta = $derived(
    progress && speed && speed > 0 ? ((progress.total - progress.done) / speed) * 1000 : null,
  );
  const overall = $derived(
    op.total > 0 ? (op.completed.length + (progress ? progress.percent / 100 : 0)) / op.total : 0,
  );
  const okCount = $derived(op.completed.filter((e) => e.ok).length);
  const failCount = $derived(op.completed.length - okCount);

  // ── Steam Guard: SteamCMD waits about 65 s for the phone per attempt ────
  const GUARD_S = 65;
  let guardLeft = $state(GUARD_S);
  let seenLines = 0;
  $effect(() => {
    if (op.phase !== "steam_guard_mobile") return;
    guardLeft = GUARD_S;
    const t = setInterval(() => (guardLeft = Math.max(0, guardLeft - 1)), 1000);
    return () => clearInterval(t);
  });
  $effect(() => {
    const n = op.log.length;
    if (op.phase === "steam_guard_mobile") {
      for (let i = seenLines; i < n; i++) if (/retrying/i.test(op.log[i]!)) guardLeft = GUARD_S;
    }
    seenLines = n;
  });

  // ── password ────────────────────────────────────────────────────────────
  let password = $state("");
  let reveal = $state(false);
  let sending = $state(false);
  let distrust = $state(false);
  const login = $derived(profile.data?.steam_login || "YOUR_USERNAME");
  // On Linux SteamCMD runs with a HOME of its own (its login is cached
  // there, away from the Steam client's): a login by hand must use it too.
  let steamcmdHome = $state<string | null>(null);
  $effect(() => {
    if (op.phase === "password_required" && steamcmdHome === null)
      steamcmdDirs()
        .then((d) => (steamcmdHome = d.home))
        .catch(() => {});
  });
  const manualCmd = $derived(
    `${steamcmdHome ? `HOME="${steamcmdHome}" ` : ""}steamcmd +login ${login} +quit`,
  );
  $effect(() => {
    if (op.phase !== "password_required") {
      sending = false;
      distrust = false;
      password = "";
    }
  });
  async function sendPassword(e: SubmitEvent) {
    e.preventDefault();
    if (!password) return;
    sending = true;
    await mods.sendInput(password);
    password = "";
  }

  // ── status line ─────────────────────────────────────────────────────────
  const status = $derived.by(() => {
    switch (op.phase) {
      case "steam_guard_mobile":
        return $c.statusSteamGuard.value;
      case "password_required":
        return $c.statusPassword.value;
      case "finished":
        if (op.kind === "login")
          return op.ok > 0 ? $c.statusLoggedIn.value : $c.statusLoginFailed.value;
        if (op.hint)
          return op.via === "steamworks" ? $c.statusSteamFailed.value : $c.statusLoginFailed.value;
        if (op.failed === 0)
          return (op.ok === 1 ? $c.statusDone({ ok: op.ok }) : $c.statusDonePlural({ ok: op.ok }))
            .value;
        return $c.statusDoneFailed({ ok: op.ok, failed: op.failed }).value;
      default:
        return op.currentName
          ? $c.statusDownloading({ name: op.currentName }).value
          : $c.statusPreparing.value;
    }
  });

  // Hidden while it runs; once finished behind a closed dialog, it tidies up.
  $effect(() => {
    if (op.minimised && !running) void mods.dismiss();
  });

  function close() {
    if (running) mods.op.minimised = true;
    else void mods.dismiss();
  }
</script>

<Dialog.Root open={!op.minimised} onOpenChange={(v) => !v && close()}>
  <Dialog.Portal>
    <Dialog.Overlay class="fixed inset-0 z-dialog bg-plot/75" />
    <Dialog.Content
      class="fixed top-1/2 left-1/2 z-dialog flex h-[min(46rem,calc(100vh-3rem))] w-[min(68rem,calc(100vw-2rem))]
             -translate-x-1/2 -translate-y-1/2 flex-col overflow-hidden rounded-md border border-border-strong bg-panel shadow-pop"
      interactOutsideBehavior="ignore"
    >
      <!-- ── header ──────────────────────────────────────────────────────── -->
      <header class="flex shrink-0 items-center gap-3 border-b border-border px-4 py-2.5">
        <span class="grid size-8 shrink-0 place-items-center rounded-md border border-border bg-bg">
          {#if running}
            <Loader class="size-icon animate-spin text-accent" />
          {:else if op.failed > 0 || op.hint}
            <CircleX class="size-icon text-err" />
          {:else}
            <CircleCheck class="size-icon text-ok" />
          {/if}
        </span>
        <div class="min-w-0 flex-1">
          <Dialog.Title class="m-0 title-display text-lg text-fg">
            {$c.modOperation.value}
            {#if op.total > 0}
              <span class="ml-1 font-mono text-sm font-medium text-fg-faint normal-case">
                {op.completed.length}/{op.total}
              </span>
            {/if}
          </Dialog.Title>
          <Dialog.Description
            class={cn(
              "m-0 truncate text-xs",
              op.hint ? "text-err" : !running && op.failed === 0 ? "text-ok" : "text-fg-muted",
            )}
          >
            {status}
          </Dialog.Description>
        </div>
        <div class="flex shrink-0 items-center gap-4 font-mono text-2xs text-fg-faint">
          <span class="flex flex-col items-end">
            <span class="num text-sm text-fg">{clock(elapsed)}</span>
            <span>{$c.elapsed.value}</span>
          </span>
          <span class="flex flex-col items-end">
            <span class="num text-sm text-ok">{okCount}</span>
            <span>{$c.succeeded.value}</span>
          </span>
          <span class="flex flex-col items-end">
            <span class={cn("num text-sm", failCount > 0 ? "text-err" : "text-fg")}
              >{failCount}</span
            >
            <span>{$c.failedLabel.value}</span>
          </span>
        </div>
        {#if running}
          <Button
            variant="ghost"
            size="icon"
            aria-label={$c.minimise.value}
            title={$c.minimise.value}
            onclick={close}
          >
            <Minimize class="size-icon" />
          </Button>
        {:else}
          <Button variant="ghost" size="icon" aria-label={$c.dismiss.value} onclick={close}>
            <X class="size-icon" />
          </Button>
        {/if}
      </header>

      <div class="grid min-h-0 flex-1 grid-cols-[22rem_minmax(0,1fr)]">
        <!-- ── left: where it is, and what came through ───────────────────── -->
        <aside
          class="flex min-h-0 flex-col gap-3 overflow-y-auto border-r border-border bg-bg/40 p-3"
        >
          {#if op.phase === "steam_guard_mobile"}
            <section class="rounded-md border border-warn/35 bg-warn/8 p-3">
              <div class="flex items-start gap-2.5">
                <ShieldAlert class="mt-0.5 size-icon-lg shrink-0 text-warn" />
                <div class="min-w-0">
                  <h3 class="m-0 text-sm font-semibold text-fg">{$c.steamguardTitle.value}</h3>
                  <p class="m-0 mt-1 text-xs leading-snug text-fg-muted">
                    {$c.steamguardDesc({ app: $c.steamguardApp.value }).value}
                  </p>
                </div>
              </div>
              <ol class="m-0 mt-3 flex list-none flex-col gap-1.5 p-0 text-2xs text-fg-muted">
                {#each [$c.steamguardStep1.value, $c.steamguardStep2.value, $c.steamguardStep3.value] as step, i (i)}
                  <li class="flex items-center gap-2">
                    <span
                      class={cn(
                        "grid size-4 shrink-0 place-items-center rounded-full font-mono text-3xs font-bold",
                        i === 2 ? "bg-ok/20 text-ok" : "bg-warn/20 text-warn",
                      )}>{i + 1}</span
                    >
                    {step}
                  </li>
                {/each}
              </ol>
              <div class="mt-3 flex items-center justify-between text-2xs">
                <span class="text-fg-faint">{$c.steamguardWaiting.value}</span>
                <span
                  class={cn(
                    "num font-mono font-semibold",
                    guardLeft <= 15 ? "text-err" : guardLeft <= 30 ? "text-warn" : "text-fg-muted",
                  )}>{clock(guardLeft * 1000)}</span
                >
              </div>
              <Meter
                class="mt-1"
                value={guardLeft}
                max={GUARD_S}
                tone={guardLeft <= 15 ? "err" : "warn"}
                label={$c.steamguardWaiting.value}
              />
              {#if guardLeft === 0}
                <p class="m-0 mt-1.5 text-2xs text-err">{$c.steamguardTimeout.value}</p>
              {:else if guardLeft <= 15}
                <p class="m-0 mt-1.5 text-2xs text-err">{$c.steamguardHurry.value}</p>
              {/if}
            </section>
          {/if}

          {#if op.phase === "password_required"}
            <section class="rounded-md border border-warn/35 bg-warn/8 p-3">
              {#if distrust}
                <div class="flex items-center gap-2">
                  <TerminalIcon class="size-icon shrink-0 text-info" />
                  <h3 class="m-0 text-sm font-semibold text-fg">{$c.manualTitle.value}</h3>
                </div>
                <p class="m-0 mt-2 text-2xs text-fg-muted">{$c.manualCopied.value}</p>
                <code
                  class="mt-1.5 block rounded-sm border border-border bg-bg px-2 py-1.5 font-mono text-2xs text-fg select-all"
                  data-selectable>{manualCmd}</code
                >
                <ol class="m-0 mt-2 list-decimal space-y-1 pl-4 text-2xs text-fg-muted">
                  <li>{$c.manualStep1.value}</li>
                  <li>{$c.manualStep2.value}</li>
                  <li>{$c.manualStep3.value}</li>
                  <li>{$c.manualStep4.value}</li>
                </ol>
                <p class="m-0 mt-2 text-3xs leading-snug text-fg-faint">{$c.manualNote.value}</p>
                <div class="mt-2.5 flex justify-end">
                  <Button variant="accent" onclick={() => mods.cancel()}>
                    <Check class="size-icon-sm" />{$c.manualConfirm.value}
                  </Button>
                </div>
              {:else}
                <div class="flex items-start gap-2.5">
                  <KeyRound class="mt-0.5 size-icon-lg shrink-0 text-warn" />
                  <div>
                    <h3 class="m-0 text-sm font-semibold text-fg">{$c.passwordTitle.value}</h3>
                    <p class="m-0 mt-0.5 text-2xs leading-snug text-fg-muted">
                      {$c.passwordDesc.value}
                    </p>
                  </div>
                </div>
                <form class="mt-2.5 flex items-center gap-1.5" onsubmit={sendPassword}>
                  <Input
                    class="flex-1"
                    type={reveal ? "text" : "password"}
                    placeholder={$c.passwordPlaceholder.value}
                    autocomplete="current-password"
                    disabled={sending}
                    bind:value={password}
                  />
                  <Button
                    variant="ghost"
                    size="icon"
                    aria-label={$c.showPassword.value}
                    onclick={() => (reveal = !reveal)}
                  >
                    {#if reveal}<EyeOff class="size-icon-sm" />{:else}<Eye
                        class="size-icon-sm"
                      />{/if}
                  </Button>
                  <Button type="submit" variant="accent" disabled={!password || sending}>
                    {#if sending}<Loader class="size-icon-sm animate-spin" />{:else}<Send
                        class="size-icon-sm"
                      />{/if}
                    {$c.passwordSend.value}
                  </Button>
                </form>
                <div class="mt-2 flex justify-end">
                  <Button
                    variant="danger"
                    size="xs"
                    onclick={async () => {
                      await copyText(manualCmd);
                      distrust = true;
                    }}
                  >
                    <ShieldOff class="size-3" />{$c.dontTrust.value}
                  </Button>
                </div>
              {/if}
            </section>
          {/if}

          {#if op.hint}
            <section class="rounded-md border border-err/35 bg-err/8 p-3">
              <pre
                class="m-0 font-mono text-2xs leading-snug whitespace-pre-wrap text-err"
                data-selectable>{op.hint}</pre>
            </section>
          {/if}

          <!-- overall -->
          <section class="rounded-md border border-border bg-panel p-3">
            <div class="flex items-baseline justify-between">
              <span class="label-stencil text-fg-faint">{$c.overall.value}</span>
              <span class="num font-mono text-xs text-fg">{Math.round(overall * 100)}%</span>
            </div>
            <Meter
              class="mt-1.5"
              value={overall}
              max={1}
              size="md"
              tone={failCount > 0 ? "warn" : "accent"}
              label={$c.overall.value}
            />
            <div class="mt-1.5 flex justify-between font-mono text-2xs text-fg-faint">
              <span>{op.completed.length} / {op.total || "—"}</span>
              <span>{clock(elapsed)}</span>
            </div>
          </section>

          <!-- the mod being worked on -->
          {#if running && op.phase === "downloading"}
            <TransferProgress
              label={$c.itemProgress.value}
              name={op.currentName}
              percent={progress?.percent ?? null}
              done={progress?.done ?? null}
              total={progress?.total ?? null}
              {speed}
              {eta}
              elapsed={now - op.itemStartedAt}
              labels={{
                size: $m.colSize.value,
                speed: $c.speed.value,
                eta: "ETA",
                elapsed: $c.elapsed.value,
              }}
            />
          {/if}

          <!-- what came through -->
          <section class="flex min-h-32 flex-1 flex-col rounded-md border border-border bg-panel">
            <div class="flex items-center justify-between border-b border-border/70 px-3 py-1.5">
              <span class="label-stencil text-fg-faint">{$c.queue.value}</span>
              <span class="font-mono text-2xs text-fg-faint">{op.completed.length}</span>
            </div>
            <ul class="m-0 min-h-0 flex-1 list-none divide-y divide-border/50 overflow-y-auto p-0">
              {#each [...op.completed].reverse() as e (`${e.id}-${e.ms}`)}
                {@const size = mods.byId.get(e.id)?.size}
                <li class="flex items-center gap-2 px-3 py-1.5">
                  {#if e.ok}<Check class="size-icon-sm shrink-0 text-ok" />{:else}<X
                      class="size-icon-sm shrink-0 text-err"
                    />{/if}
                  <div class="min-w-0 flex-1">
                    <p class="m-0 truncate text-xs text-fg">{e.name}</p>
                    <p class="m-0 font-mono text-3xs text-fg-faint">
                      #{e.id}{#if size}<span> · {bytes(size)}</span>{/if}
                    </p>
                  </div>
                  <span class="num shrink-0 font-mono text-2xs text-fg-faint">{clock(e.ms)}</span>
                </li>
              {:else}
                <li class="px-3 py-3 text-2xs text-fg-faint">{$c.noneYet.value}</li>
              {/each}
            </ul>
          </section>
        </aside>

        <!-- ── right: the downloader, live ────────────────────────────────── -->
        <LogPane
          lines={op.log}
          times={op.logAt}
          title={op.via === "steamworks" ? "$ steam (steamworks)" : "$ steamcmd"}
          live={running}
        />
      </div>

      <!-- ── footer ──────────────────────────────────────────────────────── -->
      <footer class="flex shrink-0 items-center gap-2 border-t border-border px-4 py-2">
        <span class="text-2xs text-fg-faint"
          >{op.via === "steamworks" ? $c.steamworksFooter.value : $m.confirmWarning.value}</span
        >
        <div class="ml-auto flex items-center gap-1.5">
          {#if running}
            <Button variant="ghost" onclick={close}>
              <Minimize class="size-icon-sm" />{$c.minimise.value}
            </Button>
            <Button variant="danger" onclick={() => mods.cancel()}>
              <X class="size-icon-sm" />{$c.cancel.value}
            </Button>
          {:else}
            <Button variant="accent" onclick={() => mods.dismiss()}>{$c.dismiss.value}</Button>
          {/if}
        </div>
      </footer>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
