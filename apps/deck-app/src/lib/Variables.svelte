<script lang="ts">
  // The {{ }} variables a theme's or a layout's texts can hold, filled per
  // slide. Clicking one inserts it where the text is being typed: the field
  // last focused (a placeholder's prefix) or the text box being edited;
  // otherwise it is copied to the clipboard.
  let { oninsert }: { oninsert: (text: string) => boolean } = $props();

  const VARS: [string, string][] = [
    ["{{section}}", "the section the slide belongs to"],
    ["{{subsection}}", "its subsection"],
    ["{{slide}}", "the slide's number"],
    ["{{slides}}", "the number of slides"],
    ["{{title}}", "the slide's title"],
    ["{{deck}}", "the deck's title"],
    ["{{date}}", "today, YYYY-MM-DD"],
    ['{{section, " | ", subsection}}', "joined: the separator shows only between two values"],
    ['{{slide, " / ", slides}}', "3 / 12"],
  ];
  let copied = $state("");

  async function pick(v: string) {
    if (oninsert(v)) return;
    try {
      await navigator.clipboard.writeText(v);
      copied = v;
      setTimeout(() => (copied = ""), 1500);
    } catch {
      /* no clipboard: nothing to do */
    }
  }
</script>

<div class="vars">
  {#each VARS as [v, what]}
    <!-- Kept from taking the focus: the text being typed keeps it. -->
    <button title={what} onpointerdown={(e) => e.preventDefault()} onclick={() => pick(v)}><code>{v}</code></button>
  {/each}
  <p class="muted">{copied ? `Copied ${copied}: paste it in a text.` : "Click to insert where you type (a prefix, a text box being edited)."}</p>
</div>

<style>
  .vars { display: flex; flex-wrap: wrap; gap: 4px; }
  .vars button { padding: 1px 5px; font-size: 11px; }
  .vars code { font-size: 11px; }
  .muted { width: 100%; margin: 2px 0 0; font-size: 11px; color: var(--muted); }
</style>
