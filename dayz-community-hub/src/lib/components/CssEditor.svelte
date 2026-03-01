<script lang="ts">
  interface Props {
    value: string;
    onInput: (value: string) => void;
    placeholder?: string;
  }

  let { value, onInput, placeholder = '' }: Props = $props();

  let textareaEl: HTMLTextAreaElement | null = $state(null);
  let preEl: HTMLPreElement | null = $state(null);

  // Internal editable copy of the value
  let text = $state(value);

  // Sync from parent when value prop changes (e.g., reset)
  $effect(() => {
    text = value;
  });

  // Sync scroll between textarea and pre
  function handleScroll() {
    if (textareaEl && preEl) {
      preEl.scrollTop = textareaEl.scrollTop;
      preEl.scrollLeft = textareaEl.scrollLeft;
    }
  }

  // Handle tab key for indentation
  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Tab') {
      e.preventDefault();
      const target = e.target as HTMLTextAreaElement;
      const start = target.selectionStart;
      const end = target.selectionEnd;
      text = text.substring(0, start) + '  ' + text.substring(end);
      onInput(text);
      // Restore cursor position after the tab
      requestAnimationFrame(() => {
        target.selectionStart = target.selectionEnd = start + 2;
      });
    }
  }

  // Simple CSS syntax highlighting
  function highlightCss(code: string): string {
    if (!code) return '';

    // Escape HTML
    let html = code.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

    // Comments /* */
    html = html.replace(/(\/\*[\s\S]*?\*\/)/g, '<span class="css-comment">$1</span>');

    // Strings
    html = html.replace(/("[^"]*"|'[^']*')/g, '<span class="css-string">$1</span>');

    // CSS variables --name
    html = html.replace(/(--[\w-]+)/g, '<span class="css-variable">$1</span>');

    // Properties (word followed by :)
    html = html.replace(/\b([\w-]+)(\s*:)/g, '<span class="css-property">$1</span>$2');

    // Values: oklch, rgb, hsl, hex colors
    html = html.replace(/\b(oklch|rgb|rgba|hsl|hsla|hwb|lab|lch|color)\(/g, '<span class="css-function">$1</span>(');

    // Numbers with units
    html = html.replace(/\b(\d+\.?\d*)(px|rem|em|%|deg|s|ms)?\b/g, '<span class="css-number">$1$2</span>');

    // Selectors (lines starting with [ or . or # or element)
    html = html.replace(
      /^(\s*)(\[data-theme[^\]]*\]|\.[a-zA-Z][\w-]*|#[a-zA-Z][\w-]*)/gm,
      '$1<span class="css-selector">$2</span>',
    );

    // Curly braces
    html = html.replace(/([{}])/g, '<span class="css-brace">$1</span>');

    return html;
  }

  const highlighted = $derived(highlightCss(text));
</script>

<div class="css-editor">
  <pre bind:this={preEl} class="css-highlight" aria-hidden="true"><code>{@html highlighted}</code></pre>
  <textarea
    bind:this={textareaEl}
    bind:value={text}
    class="css-textarea"
    {placeholder}
    onchange={() => onInput(text)}
    oninput={() => onInput(text)}
    onscroll={handleScroll}
    onkeydown={handleKeydown}
    spellcheck="false"
    autocomplete="off"
    autocorrect="off"
    autocapitalize="off"
  ></textarea>
</div>

<style>
  .css-editor {
    position: relative;
    width: 100%;
    height: 100%;
    min-height: 300px;
    font-family: ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Consolas, monospace;
    font-size: 0.75rem;
    line-height: 1.5;
    tab-size: 2;
  }

  .css-highlight,
  .css-textarea {
    position: absolute;
    inset: 0;
    margin: 0;
    padding: 1rem;
    border: 1px solid var(--color-base-300, #333);
    border-radius: 0.5rem;
    overflow: auto;
    white-space: pre-wrap;
    word-wrap: break-word;
    font: inherit;
    line-height: inherit;
    tab-size: inherit;
  }

  .css-highlight {
    background: var(--color-base-200, #1a1a1a);
    color: var(--color-base-content, #e0e0e0);
    pointer-events: none;
    z-index: 1;
  }

  .css-highlight code {
    font: inherit;
  }

  .css-textarea {
    background: transparent;
    color: transparent;
    caret-color: var(--color-base-content, #e0e0e0);
    resize: none;
    z-index: 2;
    outline: none;
  }

  .css-textarea:focus {
    border-color: var(--color-primary, #6366f1);
  }

  .css-textarea::placeholder {
    color: color-mix(in oklch, var(--color-base-content, #e0e0e0) 30%, transparent);
  }

  /* Syntax highlighting colors */
  :global(.css-comment) {
    color: oklch(55% 0.02 120);
    font-style: italic;
  }

  :global(.css-selector) {
    color: oklch(70% 0.18 200);
    font-weight: 600;
  }

  :global(.css-property) {
    color: oklch(70% 0.16 280);
  }

  :global(.css-variable) {
    color: oklch(75% 0.18 180);
  }

  :global(.css-string) {
    color: oklch(70% 0.16 80);
  }

  :global(.css-number) {
    color: oklch(70% 0.18 140);
  }

  :global(.css-function) {
    color: oklch(70% 0.16 320);
  }

  :global(.css-brace) {
    color: color-mix(in oklch, var(--color-base-content, #e0e0e0) 60%, transparent);
  }
</style>
