<script lang="ts">
  import { cn } from "$lib/cx";

  /**
   * Contour lines, as on a survey map of Chernarus: the texture behind empty
   * states and the rail's foot. Decorative, in the border colour, and never
   * behind anything that has to be read.
   */
  let { class: klass = "", opacity = 0.5 }: { class?: string; opacity?: number } = $props();
</script>

<svg
  class={cn("pointer-events-none absolute inset-0 size-full text-border", klass)}
  style="opacity: {opacity}"
  aria-hidden="true"
  preserveAspectRatio="xMidYMid slice"
  viewBox="0 0 400 300"
>
  <g fill="none" stroke="currentColor" stroke-width="1">
    <path d="M-10 250 C 60 200, 120 260, 190 220 S 320 150, 410 190" />
    <path d="M-10 226 C 70 176, 130 236, 196 196 S 318 128, 410 166" />
    <path d="M-10 202 C 80 150, 140 210, 202 172 S 316 106, 410 142" />
    <path d="M30 175 C 90 128, 150 186, 208 150 S 300 92, 360 104 S 404 96, 410 92" />
    <path d="M70 150 C 120 108, 168 160, 214 130 S 290 84, 330 90" />
    <path d="M120 128 C 150 100, 186 136, 220 114 S 270 84, 296 90 C 280 116, 230 146, 180 150 C 150 152, 128 142, 120 128 Z" />
    <path d="M168 118 C 184 106, 204 120, 226 106 C 244 96, 260 100, 262 104 C 248 122, 212 134, 190 132 C 176 131, 168 126, 168 118 Z" />
    <path d="M-10 60 C 40 40, 90 80, 150 56 S 250 10, 300 26 S 380 60, 410 44" />
    <path d="M-10 36 C 50 16, 100 56, 156 32 S 246 -10, 300 4" />
    <path d="M260 300 C 280 270, 330 262, 360 280 S 400 300, 410 296" />
  </g>
</svg>
