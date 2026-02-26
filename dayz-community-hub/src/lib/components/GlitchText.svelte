<script lang="ts">
  import { onMount, onDestroy } from 'svelte';

  interface Props {
    text: string;
    /** Total ms before the real text is fully revealed (default 4000) */
    revealDuration?: number;
    /** Interval between scramble frames in ms (default 30) */
    speed?: number;
    /** Extra CSS classes forwarded to the wrapping element */
    class?: string;
    /** Tag to render as — 'span' for inline, 'h1'/'h2'/... for block */
    tag?: string;
    /**
     * Increment this counter from outside to imperatively trigger a glitch.
     * Bypasses the hover cooldown (same rules as the visibility-change path).
     */
    externalTrigger?: number;
  }

  let {
    text,
    revealDuration = 1250,
    speed = 4,
    class: klass = '',
    tag = 'span',
    externalTrigger = 0,
  }: Props = $props();

  const CHARS = '░▒░ ░██░> ████▓ >█> ░/█>█ ██░░ █<▒ ▓██░ ░/░▒';

  /** How the animation guard works:
   *  - `running`  : animation is currently playing → block all new triggers
   *  - `cooldown` : animation just finished, hover blocked for 10 s
   *                 page-focus / prop-change still allowed
   *  - `idle`     : ready for any trigger
   */
  type State = 'idle' | 'running' | 'cooldown';
  let animState: State = 'idle';

  let displayed = $state('');
  /** Locked pixel width measured from the real text on first render */
  let lockedWidth = $state<string | null>(null);
  let el = $state<HTMLElement | null>(null);

  let frameTimer:    ReturnType<typeof setInterval> | null = null;
  let revealTimer:   ReturnType<typeof setTimeout>  | null = null;
  let cooldownTimer: ReturnType<typeof setTimeout>  | null = null;

  function randomChar(): string {
    return CHARS[Math.floor(Math.random() * CHARS.length)];
  }

  function lockedCount(elapsed: number): number {
    return Math.floor((elapsed / revealDuration) * text.length);
  }

  function cleanup() {
    if (frameTimer)    clearInterval(frameTimer);
    if (revealTimer)   clearTimeout(revealTimer);
    if (cooldownTimer) clearTimeout(cooldownTimer);
    frameTimer    = null;
    revealTimer   = null;
    cooldownTimer = null;
  }

  /**
   * @param fromHover — true when triggered by mouse; applies the 10 s cooldown
   *                    guard and is blocked while animation is running or cooling.
   */
  function startGlitch(fromHover = false) {
    // Hover is blocked while running or cooling down
    if (fromHover && animState !== 'idle') return;

    // Snapshot the element width before scrambling so layout never shifts
    if (el && !lockedWidth) {
      lockedWidth = `${el.getBoundingClientRect().width}px`;
    }

    cleanup();
    animState = 'running';

    const start = Date.now();
    const len = text.length;

    frameTimer = setInterval(() => {
      const elapsed = Date.now() - start;
      const locked = lockedCount(elapsed);
      let out = '';
      for (let i = 0; i < len; i++) {
        if (i < locked) {
          out += text[i];
        } else if (text[i] === ' ') {
          out += ' ';
        } else {
          out += randomChar();
        }
      }
      displayed = out;
    }, speed);

    revealTimer = setTimeout(() => {
      if (frameTimer) clearInterval(frameTimer);
      frameTimer = null;
      displayed = text;

      // Enter 10 s cooldown — hover cannot restart during this window
      animState = 'cooldown';
      cooldownTimer = setTimeout(() => {
        animState = 'idle';
        cooldownTimer = null;
      }, 10_000);
    }, revealDuration);
  }

  function onVisibilityChange() {
    if (document.visibilityState === 'visible') {
      // Page focus bypasses the hover cooldown but not an in-progress animation
      if (animState !== 'running') startGlitch();
    }
  }

  onMount(() => {
    startGlitch();
    document.addEventListener('visibilitychange', onVisibilityChange);
    return () => {
      cleanup();
      document.removeEventListener('visibilitychange', onVisibilityChange);
    };
  });

  // Re-run when `text` prop changes (e.g. About tab loads appName from Tauri)
  $effect(() => {
    const _ = text;
    // Reset locked width so it re-measures with the new text
    lockedWidth = null;
    // Prop change bypasses cooldown but not a running animation
    if (animState !== 'running') startGlitch();
  });

  // External imperative trigger — increment `externalTrigger` from a parent to
  // force a glitch (e.g. on window focus-return). Bypasses hover cooldown.
  $effect(() => {
    if (externalTrigger > 0 && animState !== 'running') startGlitch();
  });

  onDestroy(cleanup);
</script>

{#if tag === 'h1'}
  <h1
    bind:this={el}
    class={klass}
    style={lockedWidth ? `width:${lockedWidth};overflow:hidden;white-space:nowrap` : 'white-space:nowrap'}
    onmouseenter={() => startGlitch(true)}
  >{displayed}</h1>
{:else if tag === 'h2'}
  <h2
    bind:this={el}
    class={klass}
    style={lockedWidth ? `width:${lockedWidth};overflow:hidden;white-space:nowrap` : 'white-space:nowrap'}
    onmouseenter={() => startGlitch(true)}
  >{displayed}</h2>
{:else if tag === 'h3'}
  <h3
    bind:this={el}
    class={klass}
    style={lockedWidth ? `width:${lockedWidth};overflow:hidden;white-space:nowrap` : 'white-space:nowrap'}
    onmouseenter={() => startGlitch(true)}
  >{displayed}</h3>
{:else}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <span
    bind:this={el}
    class={klass}
    style={lockedWidth ? `width:${lockedWidth};overflow:hidden;white-space:nowrap;display:inline-block` : 'white-space:nowrap;display:inline-block'}
    onmouseenter={() => startGlitch(true)}
  >{displayed}</span>
{/if}
