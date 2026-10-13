<script lang="ts">
  import { dict } from "$lib/i18n";
  import ShieldCheck from "~icons/lucide/shield-check";
  import ShieldAlert from "~icons/lucide/shield-alert";
  import ShieldX from "~icons/lucide/shield-x";
  import { app } from "$lib/stores/app.svelte";
  import { battleye } from "$lib/stores/battleye.svelte";
  import { game } from "$lib/stores/game.svelte";
  import { cn } from "$lib/cx";
  import Segment, { WORD, type Tone } from "./Segment.svelte";

  /**
   * BattlEye, which most servers require. On Windows its service must be
   * installed and start with the game; under Proton a runtime stands in for
   * it. Read with the bar's clock while the window has focus, and every few
   * seconds while DayZ runs, to catch a game started without it. Only a
   * problem keeps it on a narrow bar; a click reinstalls it.
   */
  let { now }: { now: number } = $props();
  const s = dict("shell");

  /** How long a running game may go without the service before it is a fault. */
  const GRACE_MS = 20_000;
  const EVERY_MS = 5_000;

  /** When the game was first seen running (ms), 0 while it is not. */
  let gameSeen = $state(0);
  /** The clock while the game runs, every few seconds. */
  let clock = $state(0);

  $effect(() => {
    void now;
    void clock;
    if (app.focused || game.running) void battleye.refresh();
  });
  $effect(() => {
    if (!game.running) {
      gameSeen = 0;
      return;
    }
    gameSeen = clock = Date.now();
    const t = setInterval(() => (clock = Date.now()), EVERY_MS);
    return () => clearInterval(t);
  });

  const status = $derived(battleye.status);
  const repairing = $derived(battleye.repairing);
  const be = $derived(status?.state ?? "no_game");
  const service = $derived(status?.service ?? false);
  /** DayZ is running and BattlEye's service is not, past the grace time. */
  const notStarted = $derived(
    service && gameSeen > 0 && be !== "running" && clock - gameSeen > GRACE_MS,
  );
  const broken = $derived(be === "missing" || be === "disabled");

  const tone = $derived<Tone>(repairing ? "info" : notStarted ? "err" : broken ? "warn" : "idle");
  const icon = $derived(notStarted ? ShieldX : broken ? ShieldAlert : ShieldCheck);
  const tip = $derived(
    repairing
      ? $s.sbBeRepairing.value
      : [
          notStarted
            ? $s.sbBeNotStarted.value
            : be === "running"
              ? $s.sbBeRunning.value
              : be === "disabled"
                ? $s.sbBeDisabled.value
                : be === "missing"
                  ? service
                    ? $s.sbBeMissing.value
                    : $s.sbBeMissingProton.value
                  : service
                    ? $s.sbBeReady.value
                    : $s.sbBeReadyProton.value,
          be === "running" && !notStarted
            ? ""
            : service
              ? $s.sbBeRepairAction.value
              : broken
                ? $s.sbBeInstallAction.value
                : "",
        ]
          .filter(Boolean)
          .join(" "),
  );
  /** Running needs nothing; a ready Proton runtime has nothing to repair. */
  const acts = $derived(!repairing && be !== "running" && (service || broken));
</script>

{#if status && be !== "no_game"}
  <Segment
    {icon}
    {tone}
    {tip}
    label={tip}
    onclick={acts ? () => void battleye.repair() : undefined}
    progress={repairing ? null : undefined}
    class={notStarted || broken ? "" : "@max-[900px]:hidden"}
  >
    <span class={WORD}>BattlEye</span>
    {#if notStarted || broken}
      <span>{$s.sbBeOff.value}</span>
    {:else}
      <span
        class={cn("size-1.5 rounded-full", be === "running" ? "bg-ok" : "bg-fg-faint")}
        aria-hidden="true"
      ></span>
    {/if}
  </Segment>
{/if}
