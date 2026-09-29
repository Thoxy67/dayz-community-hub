<script lang="ts">
  import { dict } from "$lib/i18n";
  import Lock from "~icons/lucide/lock";
  import KeyRound from "~icons/lucide/key-round";
  import ShieldCheck from "~icons/lucide/shield-check";

  /** What a server asks of you before you join: a password, first person, BattlEye. */
  let {
    password = false,
    firstPerson = false,
    battleye = false,
    savedPassword = false,
  }: { password?: boolean; firstPerson?: boolean; battleye?: boolean | null; savedPassword?: boolean } = $props();
  const c = dict("servers");
</script>

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
