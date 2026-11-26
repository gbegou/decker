<script lang="ts">
  // A button that opens a small menu of choices (the Shape menu).
  let {
    label,
    title = "",
    items,
    onpick,
    primary = false,
    align = "left",
  }: {
    /** Styled as the main action. */
    primary?: boolean;
    /** The edge of the button the menu lines up with. */
    align?: "left" | "right";
    label: string;
    title?: string;
    items: { value: string; label: string; title?: string; key?: string }[];
    onpick: (value: string) => void;
  } = $props();
  let open = $state(false);
</script>

<div class="menu-button" class:primary>
  <button class:primary {title} aria-haspopup="menu" aria-expanded={open} onclick={() => (open = !open)}>{label} ▾</button>
  {#if open}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div class="backdrop" onclick={() => (open = false)} oncontextmenu={(e) => { e.preventDefault(); open = false; }}></div>
    <div class="menu" class:right={align === "right"} role="menu">
      {#each items as item}
        <button role="menuitem" title={item.title ?? ""} onclick={() => { open = false; onpick(item.value); }}>
          {item.label}{#if item.key}<kbd>{item.key}</kbd>{/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .menu-button { position: relative; }
  .backdrop { position: fixed; inset: 0; z-index: 40; }
  .menu { position: absolute; left: 0; top: calc(100% + 4px); z-index: 41; min-width: 190px; background: var(--panel); border: 1px solid var(--border); border-radius: 8px; padding: 4px; box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45); display: flex; flex-direction: column; }
  .menu.right { left: auto; right: 0; }
  .menu button { text-align: left; border: none; background: none; padding: 5px 8px; border-radius: 4px; white-space: nowrap; }
  .menu button:hover { background: var(--bg); }
  kbd { float: right; margin-left: 16px; color: var(--muted); font: inherit; font-size: 11px; }
</style>
