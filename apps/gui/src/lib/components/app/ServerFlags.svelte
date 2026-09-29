<script lang="ts">
  import { dict } from "$lib/i18n";
  import Lock from "~icons/lucide/lock";
  import KeyRound from "~icons/lucide/key-round";
  import ShieldCheck from "~icons/lucide/shield-check";
  import BadgeCheck from "~icons/lucide/badge-check";
  import TriangleAlert from "~icons/lucide/triangle-alert";

  /**
   * What a server is and asks of you before you join: official (or only
   * named like one), a password, first person, BattlEye.
   */
  let {
    password = false,
    firstPerson = false,
    battleye = false,
    savedPassword = false,
    official = false,
    mimicsOfficial = false,
  }: {
    password?: boolean;
    firstPerson?: boolean;
    battleye?: boolean | null;
    savedPassword?: boolean;
    official?: boolean;
    mimicsOfficial?: boolean;
  } = $props();
  const c = dict("servers");
</script>

{#if official}
  <span
    class="inline-flex shrink-0 items-center gap-0.5 rounded-xs bg-ok/15 px-1 font-mono text-3xs font-semibold text-ok"
    title={$c.officialTitle.value}
  >
    <BadgeCheck class="size-2.5" />{$c.officialBadge.value}
  </span>
{:else if mimicsOfficial}
  <span
    class="inline-flex shrink-0 items-center gap-0.5 rounded-xs border border-warn/40 px-1 text-3xs text-warn"
    title={$c.mimicsOfficialTitle.value}
  >
    <TriangleAlert class="size-2.5" />{$c.mimicsOfficial.value}
  </span>
{/if}
{#if password}<Lock class="size-3 shrink-0 text-err" aria-label={$c.passwordProtected.value} />{/if}
{#if savedPassword}<KeyRound class="size-3 shrink-0 text-ok" aria-label={$c.savedPassword.value} />{/if}
{#if firstPerson}
  <span class="shrink-0 rounded-xs bg-warn/15 px-1 font-mono text-3xs font-semibold text-warn" title={$c.firstPerson.value}>1PP</span>
{/if}
{#if battleye}
  <!-- A drawn shield rather than BattlEye's logo, which at 12 px was a yellow
       blot in both themes. -->
  <ShieldCheck class="size-3 shrink-0 text-info" aria-label={$c.battleye.value} />
{/if}
