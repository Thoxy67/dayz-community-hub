<script lang="ts">
  import { useIntlayer } from "svelte-intlayer";
  import Lock from "~icons/lucide/lock";
  import KeyRound from "~icons/lucide/key-round";

  /** What a server asks of you before you join: a password, first person, BattlEye. */
  let {
    password = false,
    firstPerson = false,
    battleye = false,
    savedPassword = false,
  }: { password?: boolean; firstPerson?: boolean; battleye?: boolean | null; savedPassword?: boolean } = $props();
  const c = useIntlayer("servers");
</script>

{#if password}<Lock class="size-3 shrink-0 text-err" aria-label={$c.passwordProtected.value} />{/if}
{#if savedPassword}<KeyRound class="size-3 shrink-0 text-ok" aria-hidden="true" />{/if}
{#if firstPerson}
  <span class="shrink-0 rounded-xs bg-warn/15 px-1 font-mono text-3xs font-semibold text-warn" title={$c.firstPerson.value}>1PP</span>
{/if}
{#if battleye}
  <img src="/battleeye.png" alt="BE" title={$c.battleye.value} class="h-3 w-auto shrink-0 rounded-[2px] opacity-80" />
{/if}
