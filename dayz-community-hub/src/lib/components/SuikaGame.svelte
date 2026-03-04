<script lang="ts">
  import Matter from 'matter-js';
  import { onMount, onDestroy } from 'svelte';
  import * as m from '$lib/paraglide/messages.js';

  // ── Props ──────────────────────────────────────────────────────────────────
  interface Props {
    onClose: () => void;
  }
  let { onClose }: Props = $props();

  // ── Constants ──────────────────────────────────────────────────────────────
  const CW = 320;
  const CH = 460;
  const DANGER_Y = 60; // red line — stack above this = game over
  const DROP_FROM = 20; // y circles are released from (above danger line)
  const TWO_PI = Math.PI * 2;

  // 8 tiers — radius & terminal colour palette
  const TIERS = [
    { r: 13, color: '#4ade80', glow: '#16a34a' },
    { r: 19, color: '#34d399', glow: '#059669' },
    { r: 26, color: '#38bdf8', glow: '#0284c7' },
    { r: 33, color: '#818cf8', glow: '#4f46e5' },
    { r: 41, color: '#c084fc', glow: '#9333ea' },
    { r: 50, color: '#f472b6', glow: '#db2777' },
    { r: 60, color: '#fb923c', glow: '#ea580c' },
    { r: 72, color: '#facc15', glow: '#ca8a04' },
  ] as const;

  // Pre-computed per-tier values (avoid recalculating every frame per body)
  const TIER_LABELS: string[] = TIERS.map((_, i) => String(Math.pow(2, i + 1)));
  const TIER_FONTS: string[] = TIERS.map((t) => `bold ${Math.max(8, Math.floor(t.r * 0.52))}px monospace`);
  const TIER_FILL: string[] = TIERS.map((t) => t.color + 'bb');
  const TIER_DIAMETER: number[] = TIERS.map((t) => t.r * 2);

  const GRACE_FRAMES = 90;
  const MERGE_FLASH_DURATION = 22;
  const INV_FLASH_DURATION = 1 / MERGE_FLASH_DURATION;

  // ── Reactive state (HUD only) ──────────────────────────────────────────────
  type GameState = 'playing' | 'over';
  let gameState = $state<GameState>('playing');
  let score = $state(0);
  let highScore = $state(0);

  // ── Plain mutable (RAF loop, no reactivity needed) ─────────────────────────
  let gameCanvas: HTMLCanvasElement | null = null;
  let ctx: CanvasRenderingContext2D | null = null; // cached context
  let rafId: number | null = null;
  let cursorX = CW / 2;
  let currentTier = 0;

  let engine: Matter.Engine | null = null;
  const bodyTier = new Map<number, number>(); // bodyId → tier
  const bodyGrace = new Map<number, number>(); // bodyId → frames of immunity remaining

  // Merge flashes — managed as a flat pool to avoid GC churn
  let mergeFlashes: { x: number; y: number; tier: number; ttl: number }[] = [];
  let flashCount = 0;

  // Offscreen canvases for static layers (drawn once, blitted each frame)
  let gridCanvas: OffscreenCanvas | null = null;
  let scanlineCanvas: OffscreenCanvas | null = null;

  // ── Helpers ────────────────────────────────────────────────────────────────
  function randTier() {
    return Math.floor(Math.random() * 4);
  }

  function makeCircleBody(x: number, y: number, tier: number, grace = GRACE_FRAMES): Matter.Body {
    const body = Matter.Bodies.circle(x, y, TIERS[tier].r, {
      restitution: 0.4,
      friction: 0.8,
      frictionAir: 0.02,
      density: 0.002,
    });
    bodyTier.set(body.id, tier);
    bodyGrace.set(body.id, grace);
    return body;
  }

  /** Build static offscreen canvases for grid dots + scanlines */
  function buildStaticLayers() {
    // Terminal grid dots
    gridCanvas = new OffscreenCanvas(CW, CH);
    const gCtx = gridCanvas.getContext('2d')!;
    gCtx.fillStyle = 'rgba(0,255,0,0.03)';
    for (let gx = 0; gx < CW; gx += 18) for (let gy = 0; gy < CH; gy += 18) gCtx.fillRect(gx, gy, 1, 1);

    // CRT scanlines
    scanlineCanvas = new OffscreenCanvas(CW, CH);
    const sCtx = scanlineCanvas.getContext('2d')!;
    sCtx.fillStyle = 'rgba(0,0,0,0.12)';
    for (let sy = 0; sy < CH; sy += 4) sCtx.fillRect(0, sy, CW, 1);
  }

  function initWorld() {
    engine = Matter.Engine.create({ gravity: { y: 2 } });
    const wallOpts = { isStatic: true, restitution: 0.3, friction: 0.8 };
    Matter.Composite.add(engine.world, [
      // Floor is very thick so fast circles can't tunnel through
      Matter.Bodies.rectangle(CW / 2, CH + 150, CW + 40, 300, wallOpts),
      // Walls are tall enough to cover the full canvas + floor depth
      Matter.Bodies.rectangle(-25, CH / 2, 50, CH * 2, wallOpts),
      Matter.Bodies.rectangle(CW + 25, CH / 2, 50, CH * 2, wallOpts),
    ]);

    // Register collision event handler for merge detection
    Matter.Events.on(engine, 'collisionStart', onCollision);
  }

  function destroyWorld() {
    if (engine) {
      Matter.Events.off(engine, 'collisionStart', onCollision);
      Matter.Engine.clear(engine);
      engine = null;
    }
    bodyTier.clear();
    bodyGrace.clear();
    flashCount = 0;
  }

  function resetGame() {
    destroyWorld();
    initWorld();
    score = 0;
    cursorX = CW / 2;
    currentTier = randTier();
    gameState = 'playing';
  }

  function dropCircle() {
    if (!engine || gameState !== 'playing') return;
    const body = makeCircleBody(cursorX, DROP_FROM, currentTier, GRACE_FRAMES);
    Matter.Composite.add(engine.world, body);
    currentTier = randTier();
  }

  // ── Collision-based merge detection ────────────────────────────────────────
  // Queued merges to apply after the physics step (avoid mutating world mid-step)
  let pendingMerges: { aId: number; bId: number; a: Matter.Body; b: Matter.Body; tier: number }[] = [];

  function onCollision(event: Matter.IEventCollision<Matter.Engine>) {
    for (const pair of event.pairs) {
      const a = pair.bodyA;
      const b = pair.bodyB;

      // Skip static bodies (walls/floor)
      if (a.isStatic || b.isStatic) continue;

      const ta = bodyTier.get(a.id);
      const tb = bodyTier.get(b.id);
      if (ta === undefined || tb === undefined) continue;
      if (ta !== tb) continue;
      if (ta >= TIERS.length - 1) continue; // max tier can't merge

      pendingMerges.push({ aId: a.id, bId: b.id, a, b, tier: ta });
    }
  }

  function applyMerges() {
    if (!engine || pendingMerges.length === 0) return;

    const claimed = new Set<number>();

    for (const p of pendingMerges) {
      // Skip if either body was already consumed by an earlier merge this frame
      if (claimed.has(p.aId) || claimed.has(p.bId)) continue;
      // Verify bodies still exist in our tier map (not already removed)
      if (!bodyTier.has(p.aId) || !bodyTier.has(p.bId)) continue;

      claimed.add(p.aId);
      claimed.add(p.bId);

      const newTier = p.tier + 1;
      const mx = (p.a.position.x + p.b.position.x) * 0.5;
      const my = (p.a.position.y + p.b.position.y) * 0.5;
      const vx = (p.a.velocity.x + p.b.velocity.x) * 0.3;
      const vy = Math.min((p.a.velocity.y + p.b.velocity.y) * 0.3, -1);

      score += newTier * newTier * 2;

      Matter.Composite.remove(engine.world, p.a);
      Matter.Composite.remove(engine.world, p.b);
      bodyTier.delete(p.aId);
      bodyTier.delete(p.bId);
      bodyGrace.delete(p.aId);
      bodyGrace.delete(p.bId);

      const newBody = makeCircleBody(mx, my, newTier, 30);
      Matter.Body.setVelocity(newBody, { x: vx, y: vy });
      Matter.Composite.add(engine.world, newBody);

      // Add flash — reuse pool slot if available
      if (flashCount < mergeFlashes.length) {
        const f = mergeFlashes[flashCount];
        f.x = mx;
        f.y = my;
        f.tier = newTier;
        f.ttl = MERGE_FLASH_DURATION;
      } else {
        mergeFlashes.push({ x: mx, y: my, tier: newTier, ttl: MERGE_FLASH_DURATION });
      }
      flashCount++;
    }

    pendingMerges.length = 0;
  }

  function isGameOver(dynamicBodies: Matter.Body[]): boolean {
    for (let i = 0; i < dynamicBodies.length; i++) {
      const b = dynamicBodies[i];
      const grace = bodyGrace.get(b.id);
      if (grace !== undefined && grace > 0) continue;
      const tier = bodyTier.get(b.id);
      if (tier === undefined) continue;
      // Circle top is above the danger line and is at rest (low vertical speed)
      if (b.position.y - TIERS[tier].r < DANGER_Y && Math.abs(b.velocity.y) < 0.5) {
        return true;
      }
    }
    return false;
  }

  // ── Draw ──────────────────────────────────────────────────────────────────
  function draw(dynamicBodies: Matter.Body[]) {
    if (!ctx) return;

    // Background
    ctx.fillStyle = '#000';
    ctx.fillRect(0, 0, CW, CH);

    // Terminal grid dots (pre-rendered offscreen)
    if (gridCanvas) ctx.drawImage(gridCanvas, 0, 0);

    // Danger line
    ctx.save();
    ctx.setLineDash([4, 6]);
    ctx.strokeStyle = 'rgba(239,68,68,0.45)';
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(0, DANGER_Y);
    ctx.lineTo(CW, DANGER_Y);
    ctx.stroke();
    ctx.setLineDash([]);
    ctx.restore();

    // Ghost circle + drop guide line
    if (gameState === 'playing') {
      const def = TIERS[currentTier];
      ctx.save();
      ctx.shadowColor = def.glow;
      ctx.shadowBlur = 14;
      ctx.beginPath();
      ctx.arc(cursorX, DROP_FROM + def.r, def.r, 0, TWO_PI);
      ctx.strokeStyle = def.color + '55';
      ctx.lineWidth = 1.5;
      ctx.stroke();
      ctx.setLineDash([3, 6]);
      ctx.strokeStyle = def.color + '20';
      ctx.lineWidth = 1;
      ctx.beginPath();
      ctx.moveTo(cursorX, DROP_FROM + def.r * 2);
      ctx.lineTo(cursorX, CH);
      ctx.stroke();
      ctx.setLineDash([]);
      ctx.restore();
    }

    // Physics circles — batch setup to minimize save/restore calls
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';

    for (let i = 0; i < dynamicBodies.length; i++) {
      const body = dynamicBodies[i];
      const tier = bodyTier.get(body.id);
      if (tier === undefined) continue;
      const def = TIERS[tier];
      const x = body.position.x;
      const y = body.position.y;

      // Glow + fill + stroke
      ctx.save();
      ctx.shadowColor = def.glow;
      ctx.shadowBlur = 10;
      ctx.beginPath();
      ctx.arc(x, y, def.r, 0, TWO_PI);
      ctx.fillStyle = TIER_FILL[tier];
      ctx.fill();
      ctx.strokeStyle = def.color;
      ctx.lineWidth = 1.5;
      ctx.stroke();
      ctx.restore();

      // Label (no shadow needed — draw outside save/restore)
      ctx.fillStyle = 'rgba(0,0,0,0.8)';
      ctx.font = TIER_FONTS[tier];
      ctx.fillText(TIER_LABELS[tier], x, y);
    }

    // Merge flash rings
    for (let i = 0; i < flashCount; i++) {
      const f = mergeFlashes[i];
      const def = TIERS[Math.min(f.tier, TIERS.length - 1)];
      const t = f.ttl * INV_FLASH_DURATION;
      ctx.save();
      ctx.globalAlpha = t;
      ctx.shadowColor = def.glow;
      ctx.shadowBlur = 28 * t;
      ctx.strokeStyle = def.color;
      ctx.lineWidth = 2.5;
      ctx.beginPath();
      ctx.arc(f.x, f.y, def.r * (1.8 - t * 0.6), 0, TWO_PI);
      ctx.stroke();
      ctx.restore();
    }

    // CRT scanlines (pre-rendered offscreen)
    if (scanlineCanvas) ctx.drawImage(scanlineCanvas, 0, 0);
  }

  // ── RAF loop ──────────────────────────────────────────────────────────────
  function loop() {
    if (gameState !== 'playing' || !engine) return;

    // Tick down per-body grace periods
    for (const [id, g] of bodyGrace) {
      if (g > 0) bodyGrace.set(id, g - 1);
    }

    Matter.Engine.update(engine, 1000 / 60);
    applyMerges();

    // Update merge flashes in-place — compact the array by swapping
    let writeIdx = 0;
    for (let i = 0; i < flashCount; i++) {
      mergeFlashes[i].ttl--;
      if (mergeFlashes[i].ttl > 0) {
        if (writeIdx !== i) {
          // Swap to compact
          const tmp = mergeFlashes[writeIdx];
          mergeFlashes[writeIdx] = mergeFlashes[i];
          mergeFlashes[i] = tmp;
        }
        writeIdx++;
      }
    }
    flashCount = writeIdx;

    // Get dynamic bodies once per frame (used by both isGameOver and draw)
    const allBodies = Matter.Composite.allBodies(engine.world);
    const dynamicBodies: Matter.Body[] = [];
    for (let i = 0; i < allBodies.length; i++) {
      if (!allBodies[i].isStatic) dynamicBodies.push(allBodies[i]);
    }

    if (isGameOver(dynamicBodies)) {
      if (score > highScore) highScore = score;
      gameState = 'over';
      draw(dynamicBodies);
      if (rafId) {
        cancelAnimationFrame(rafId);
        rafId = null;
      }
      return;
    }
    draw(dynamicBodies);
    rafId = requestAnimationFrame(loop);
  }

  // ── Canvas use:action ─────────────────────────────────────────────────────
  function startCanvas(canvas: HTMLCanvasElement) {
    gameCanvas = canvas;
    ctx = canvas.getContext('2d');
    buildStaticLayers();
    draw([]);
    rafId = requestAnimationFrame(loop);
    return {
      destroy() {
        if (rafId) {
          cancelAnimationFrame(rafId);
          rafId = null;
        }
        gameCanvas = null;
        ctx = null;
      },
    };
  }

  // ── Input ─────────────────────────────────────────────────────────────────
  function onMouseMove(e: MouseEvent) {
    if (gameState !== 'playing' || !gameCanvas) return;
    const rect = gameCanvas.getBoundingClientRect();
    const r = TIERS[currentTier].r;
    cursorX = Math.max(r, Math.min(CW - r, (e.clientX - rect.left) * (CW / rect.width)));
  }

  function onKeydown(e: KeyboardEvent) {
    if (gameState === 'playing') {
      const r = TIERS[currentTier].r;
      if (e.key === 'ArrowLeft') {
        e.preventDefault();
        cursorX = Math.max(r, cursorX - 12);
      }
      if (e.key === 'ArrowRight') {
        e.preventDefault();
        cursorX = Math.min(CW - r, cursorX + 12);
      }
      if (e.key === ' ' || e.key === 'ArrowDown') {
        e.preventDefault();
        dropCircle();
      }
    }
    if (e.key === 'r' || e.key === 'R') {
      e.preventDefault();
      if (rafId) {
        cancelAnimationFrame(rafId);
        rafId = null;
      }
      resetGame();
      if (gameCanvas) rafId = requestAnimationFrame(loop);
    }
    if (gameState === 'over' && e.key === ' ') {
      e.preventDefault();
      resetGame();
      if (gameCanvas) rafId = requestAnimationFrame(loop);
    }
    if (e.key === 'Escape') onClose();
  }

  // ── Lifecycle ─────────────────────────────────────────────────────────────
  onMount(() => {
    initWorld();
    currentTier = randTier();
    window.addEventListener('keydown', onKeydown);
  });

  onDestroy(() => {
    window.removeEventListener('keydown', onKeydown);
    if (rafId) cancelAnimationFrame(rafId);
    destroyWorld();
  });
</script>

<div
  class="egg-overlay fixed inset-x-0 bottom-0 z-[9999] flex flex-col items-center justify-center bg-black/95 select-none"
  style="top: 36px"
  role="dialog"
  aria-modal="true"
>
  <!-- CRT scanline layer -->
  <div class="scanlines"></div>

  <!-- Header -->
  <div class="relative z-10 flex items-center gap-6 mb-3">
    <p class="glitch-title font-black font-mono text-white tracking-[0.3em] uppercase text-lg">{m.suika_title()}</p>
    <span class="text-white/20 font-mono text-[10px] tracking-widest">{m.suika_hint()}</span>
  </div>

  <!-- HUD -->
  <div class="relative z-10 flex items-center gap-8 mb-3 font-mono text-[11px] tracking-widest">
    <span class="text-white/40">{m.suika_score()}<span class="text-score font-bold ml-2">{score}</span></span>
    <span class="text-white/40">{m.suika_best()}<span class="text-best/70 font-bold ml-2">{highScore}</span></span
    >
  </div>

  <!-- Canvas -->
  <canvas
    use:startCanvas
    onmousemove={onMouseMove}
    onclick={() => dropCircle()}
    width={CW}
    height={CH}
    class="relative z-10 border border-white/10 rounded-sm block cursor-crosshair"
  ></canvas>

  <!-- Controls / game over -->
  <div class="relative z-10 mt-3 text-center">
    {#if gameState === 'playing'}
      <p class="font-mono text-[10px] text-white/25 tracking-widest">
        <kbd class="game-kbd">← →</kbd>
        {m.suika_move()} &nbsp;·&nbsp;
        <kbd class="game-kbd">SPACE</kbd>
        {m.suika_drop()} &nbsp;·&nbsp;
        <kbd class="game-kbd">R</kbd>
        {m.suika_restart()} &nbsp;·&nbsp;
        <kbd class="game-kbd">ESC</kbd>
        {m.suika_close()}
      </p>
    {:else}
      <p class="glitch-title font-mono text-sm text-error font-black tracking-[0.4em] uppercase mb-2">
        {m.suika_overflow()}
      </p>
      <p class="font-mono text-[10px] text-white/25 tracking-widest">
        <kbd class="game-kbd">SPACE</kbd> or <kbd class="game-kbd">R</kbd>
        {m.suika_retry()} &nbsp;·&nbsp;
        <kbd class="game-kbd">ESC</kbd>
        {m.suika_close()}
      </p>
    {/if}
  </div>

  <!-- Decorative terminal chars -->
  <div class="relative z-10 flex gap-3 mt-4 font-mono text-white/[0.06] text-base pointer-events-none tracking-widest">
    {#each ['█', '▓', '▒', '░', '▒', '▓', '█'] as c}<span>{c}</span>{/each}
  </div>
</div>

<style>
  .egg-overlay {
    animation: egg-in 0.25s cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  @keyframes egg-in {
    from {
      opacity: 0;
      transform: scale(0.95);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }

  .scanlines {
    position: absolute;
    inset: 0;
    z-index: 1;
    background: repeating-linear-gradient(
      0deg,
      transparent,
      transparent 2px,
      rgba(0, 0, 0, 0.18) 2px,
      rgba(0, 0, 0, 0.18) 4px
    );
    pointer-events: none;
  }

  .glitch-title {
    animation: glitch-rgb 0.11s infinite alternate;
  }
  @keyframes glitch-rgb {
    0% {
      text-shadow:
        2px 0 #ff003c,
        -2px 0 #00d4ff;
    }
    100% {
      text-shadow:
        -2px 0 #ff003c,
        2px 0 #00d4ff;
    }
  }

  .game-kbd {
    font-family: monospace;
    font-size: 10px;
    padding: 1px 5px;
    border-radius: 3px;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.12);
    color: rgba(255, 255, 255, 0.5);
  }
</style>
