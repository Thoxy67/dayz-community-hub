<script lang="ts">
  import { Dialog } from "bits-ui";
  import { dict } from "$lib/i18n";
  import X from "~icons/lucide/x";
  import Circle from "~icons/lucide/circle";
  import PanelRightOpen from "~icons/lucide/panel-right-open";
  import PanelRightClose from "~icons/lucide/panel-right-close";
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
  // The current mod's share counts only until it is among the finished: the
  // last mod's 100 % added to a full count once showed 125 %.
  const currentDone = $derived(op.completed.some((e) => e.id === op.currentId));
  const overall = $derived(
    op.total > 0
      ? Math.min(
          1,
          (op.completed.length + (progress && !currentDone ? progress.percent / 100 : 0)) /
            op.total,
        )
      : 0,
  );
  const okCount = $derived(op.completed.filter((e) => e.ok).length);
  const failCount = $derived(op.completed.length - okCount);

  // ── what the operation is, said in its title ────────────────────────────
  const kindLabel = $derived(
    op.kind === "login"
      ? $c.kindLogin.value
      : op.kind === "repair"
        ? $c.kindRepair.value
        : op.kind === "install_server" || op.kind === "install_manual"
          ? $c.kindInstall.value
          : $c.kindUpdate.value,
  );

  // ── the queue: every mod asked for, by name, with where it is ───────────
  type QueueState = "queued" | "downloading" | "done" | "failed";
  const queue = $derived.by(() => {
    const done = new Map(op.completed.map((e) => [e.id, e]));
    // The ids known at the start, then any that came through without being
    // announced (a server's mods resolved on the way).
    const ids = [...op.ids, ...op.completed.map((e) => e.id).filter((id) => !op.ids.includes(id))];
    return ids.map((id) => {
      const e = done.get(id);
      const state: QueueState = e
        ? e.ok
          ? "done"
          : "failed"
        : running && id === op.currentId
          ? "downloading"
          : "queued";
      return { id, name: e?.name || mods.nameOf(id), state, ms: e?.ms ?? null };
    });
  });
  // The row being downloaded stays in sight as the queue moves on.
  let queueBox = $state<HTMLElement>();
  $effect(() => {
    void op.currentId;
    queueBox
      ?.querySelector('[data-state="downloading"]')
      ?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  });

  // The downloader's own output: there for whoever wants it, out of the way
  // otherwise. Kept for the session.
  let showLog = $state(true);

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
    <Dialog.Overlay
      class="fixed inset-0 z-dialog bg-plot/75 data-[state=open]:animate-fade-in data-[state=closed]:animate-fade-out"
    />
    <!-- One frame for every phase (downloading, Steam Guard, password): the
         header and footer stay put, only the left column's top changes. -->
    <Dialog.Content
      class={cn(
        "fixed top-1/2 left-1/2 z-dialog flex h-[min(46rem,calc(100vh-3rem))] flex-col overflow-hidden data-[state=open]:animate-pop-in data-[state=closed]:animate-pop-out",
        "-translate-x-1/2 -translate-y-1/2 rounded-md border border-border-strong bg-panel shadow-pop",
        showLog ? "w-[min(68rem,calc(100vw-2rem))]" : "w-[min(30rem,calc(100vw-2rem))]",
      )}
      interactOutsideBehavior="ignore"
    >
      <!-- ── header: what it is, where it is, the whole batch's progress ── -->
      <header class="flex shrink-0 flex-col gap-2.5 border-b border-border px-4 pt-3 pb-3">
        <div class="flex items-center gap-3">
          <span
            class="grid size-8 shrink-0 place-items-center rounded-md border border-border bg-bg"
          >
            {#if running}
              <Loader class="size-icon animate-spin text-accent" />
            {:else if op.failed > 0 || op.hint}
              <CircleX class="size-icon text-err" />
            {:else}
              <CircleCheck class="size-icon text-ok" />
            {/if}
          </span>
          <div class="min-w-0 flex-1">
            <Dialog.Title class="m-0 truncate title-display text-lg leading-tight text-fg">
              {kindLabel}
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
          <div class="flex shrink-0 items-end gap-x-5">
            <span class="flex flex-col items-end gap-0.5">
              <span class="label-stencil text-fg-faint">{$c.elapsed.value}</span>
              <span class="num font-mono text-sm font-semibold text-fg">{clock(elapsed)}</span>
            </span>
            <span class="flex flex-col items-end gap-0.5">
              <span class="label-stencil text-fg-faint">{$c.succeeded.value}</span>
              <span class="num font-mono text-sm font-semibold text-ok">{okCount}</span>
            </span>
            <span class="flex flex-col items-end gap-0.5">
              <span class="label-stencil text-fg-faint">{$c.failedLabel.value}</span>
              <span
                class={cn(
                  "num font-mono text-sm font-semibold",
                  failCount > 0 ? "text-err" : "text-fg-muted",
                )}>{failCount}</span
              >
            </span>
          </div>
        </div>
        {#if op.total > 0}
          <div class="flex items-center gap-3">
            <span class="label-stencil shrink-0 text-fg-faint">{$c.overall.value}</span>
            <Meter
              class="flex-1"
              value={overall}
              max={1}
              size="md"
              tone={failCount > 0 ? "warn" : "accent"}
              label={$c.overall.value}
            />
            <span class="num w-24 shrink-0 text-right font-mono text-2xs text-fg-faint">
              <span class="text-fg">{Math.round(overall * 100)}%</span> · {op.completed
                .length}/{op.total}
            </span>
          </div>
        {/if}
      </header>

      <div
        class={cn(
          "grid min-h-0 flex-1",
          showLog ? "grid-cols-[22rem_minmax(0,1fr)]" : "grid-cols-1",
        )}
      >
        <!-- ── left: what needs the player, the current mod, the queue ──── -->
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

          <!-- the mod being worked on: its bytes, speed and time left -->
          {#if running && op.phase === "downloading" && op.currentId && !currentDone}
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

          <!-- the queue: every mod by name, with where it is. A bounded
               scroller: its box is sized by the column, not by the list. -->
          {#if queue.length > 0}
            <section class="flex min-h-32 flex-1 flex-col rounded-md border border-border bg-panel">
              <div class="flex items-center justify-between border-b border-border/70 px-3 py-1.5">
                <span class="label-stencil text-fg-faint">{$c.queue.value}</span>
                <span class="num font-mono text-2xs text-fg-faint"
                  >{op.completed.length}/{queue.length}</span
                >
              </div>
              <div class="relative min-h-0 flex-1">
                <ul
                  bind:this={queueBox}
                  class="absolute inset-0 m-0 list-none divide-y divide-border/50 overflow-y-auto p-0"
                >
                  {#each queue as q (q.id)}
                    {@const size = mods.byId.get(q.id)?.size}
                    <li
                      data-state={q.state}
                      class={cn(
                        "relative flex items-center gap-2 px-3 py-1.5 transition-colors duration-base",
                        q.state === "downloading" && "bg-accent/8",
                        q.state === "queued" && "opacity-60",
                      )}
                    >
                      {#if q.state === "downloading"}
                        <span class="absolute inset-y-1 left-0 w-0.5 rounded-full bg-accent"></span>
                        <Loader class="size-icon-sm shrink-0 animate-spin text-accent" />
                      {:else if q.state === "done"}
                        <Check class="size-icon-sm shrink-0 animate-pop-in text-ok" />
                      {:else if q.state === "failed"}
                        <X class="size-icon-sm shrink-0 animate-pop-in text-err" />
                      {:else}
                        <Circle class="size-icon-sm shrink-0 text-fg-faint" />
                      {/if}
                      <div class="min-w-0 flex-1">
                        <p
                          class={cn(
                            "m-0 truncate text-xs",
                            q.state === "downloading" ? "font-semibold text-fg" : "text-fg",
                          )}
                          title={q.name}
                        >
                          {q.name}
                        </p>
                        <p class="m-0 truncate font-mono text-3xs text-fg-faint" data-selectable>
                          {q.id}{#if size}<span class="ml-1">· {bytes(size)}</span>{/if}
                        </p>
                      </div>
                      <span class="num shrink-0 text-right font-mono text-2xs">
                        {#if q.state === "downloading"}
                          <span class="text-accent"
                            >{progress && !currentDone
                              ? `${Math.floor(progress.percent)}%`
                              : "…"}</span
                          >
                        {:else if q.state === "done"}
                          <span class="text-fg-faint">{clock(q.ms ?? 0)}</span>
                        {:else if q.state === "failed"}
                          <span class="text-err">{$c.itemFailed.value}</span>
                        {:else}
                          <span class="text-fg-faint">{$c.itemQueued.value}</span>
                        {/if}
                      </span>
                    </li>
                  {/each}
                </ul>
              </div>
            </section>
          {/if}
        </aside>

        <!-- ── right: the downloader, live ────────────────────────────────── -->
        {#if showLog}
          <LogPane
            lines={op.log}
            times={op.logAt}
            title={op.via === "steamworks" ? "$ steam (steamworks)" : "$ steamcmd"}
            live={running}
          />
        {/if}
      </div>

      <!-- ── footer: the note, then the log, minimise and cancel, always in
           that order at the right ─────────────────────────────────────────── -->
      <footer class="flex shrink-0 items-center gap-3 border-t border-border px-4 py-2">
        <span class="min-w-0 flex-1 text-2xs leading-snug text-fg-faint"
          >{op.via === "steamworks" ? $c.steamworksFooter.value : $m.confirmWarning.value}</span
        >
        <div class="flex shrink-0 items-center gap-1">
          <Button variant="ghost" onclick={() => (showLog = !showLog)}>
            {#if showLog}<PanelRightClose class="size-icon-sm" />{:else}<PanelRightOpen
                class="size-icon-sm"
              />{/if}
            {showLog ? $c.hideLog.value : $c.showLog.value}
          </Button>
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
