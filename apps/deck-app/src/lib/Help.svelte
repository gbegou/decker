<script lang="ts">
  let { onclose }: { onclose: () => void } = $props();
  const sections: [string, [string, string][]][] = [
    ["Deck", [
      ["New · Open PPTX… · Open .deck folder…", "Start a deck, import a PowerPoint file, open a saved deck"],
      ["Save · Save as…", "Write the deck folder (deck.json + assets/)"],
      ["Export PPTX… · Export PDF…", "PowerPoint file; PDF with bookmarks for sections and slides"],
    ]],
    ["Slides", [
      ["Click a thumbnail", "Go to that slide"],
      ["Drag a thumbnail · Ctrl+Up / Ctrl+Down", "Reorder slides; sections travel with their first slide"],
      ["+ Slide · Duplicate · − Slide", "New slide from the current layout; copy; remove"],
      ["Page Up / Page Down", "Previous or next slide"],
      ["Insert from deck…", "Copy slides from another deck, with their images and layouts"],
      ["+ section · + subsection (hover a thumbnail)", "Start a section there; click a header to rename, clear to remove"],
    ]],
    ["Elements", [
      ["Click", "Select; drag to move; corner handle to resize; Delete to remove"],
      ["+ Text · + Plan", "A text box; the sections list with the current one highlighted"],
      ["Double-click a text", "Edit in place: type, Backspace/Delete, arrows, Home/End, Shift+arrows to select, Ctrl+A"],
      ["Enter · Escape", "New paragraph · finish editing"],
      ["Ctrl+B · Ctrl+I", "Bold · italic (selection, or the whole box when not editing)"],
      ["Style bar", "Size, color, font, alignment, bullets; fill and outline of boxes and shapes; plan colors"],
      ["Image panel", "Crop to a shape; percentage cut from each edge"],
    ]],
    ["Layouts", [
      ["Layout selector · Edit layout · + New layout", "The slide's layout; open the layout editor; create one"],
      ["+ Placeholder · Save layout… · Load layout…", "Slots new slides start from; save as .deck_layout; load one"],
      ["{{section}} {{subsection}} {{slide}} {{slides}} {{title}} {{deck}} {{date}}", "Variables in layout texts, filled per slide"],
    ]],
    ["Presenting", [
      ["Present · Esc", "Fullscreen on the projector, presenter view on the laptop · end"],
      ["Right / Down / Space / Enter / click · Left / Up / Backspace", "Next · previous step or slide"],
      ["Home · End", "First · last slide"],
      ["B · W", "Black · white screen (write on the whiteboard)"],
    ]],
    ["Everywhere", [
      ["F1 · ?", "This help"],
    ]],
  ];
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="modal" onclick={onclose}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="dialog" onclick={(e) => e.stopPropagation()}>
    <div class="head"><h2>Commands and keys</h2><button onclick={onclose}>Close</button></div>
    <div class="columns">
      {#each sections as [title, rows]}
        <section>
          <h3>{title}</h3>
          <table><tbody>
            {#each rows as [key, what]}
              <tr><td class="key">{key}</td><td>{what}</td></tr>
            {/each}
          </tbody></table>
        </section>
      {/each}
    </div>
  </div>
</div>

<style>
  .modal { position: fixed; inset: 0; background: rgba(0,0,0,0.6); display: flex; align-items: center; justify-content: center; z-index: 10; }
  .dialog { background: var(--panel); border: 1px solid var(--border); border-radius: 10px; padding: 16px 20px; width: min(1100px, 94vw); max-height: 90vh; overflow-y: auto; }
  .head { display: flex; justify-content: space-between; align-items: center; }
  h2 { margin: 0 0 8px; font-size: 16px; }
  h3 { margin: 12px 0 4px; font-size: 12px; color: var(--muted); text-transform: uppercase; }
  .columns { columns: 2; column-gap: 28px; }
  section { break-inside: avoid; }
  table { border-collapse: collapse; width: 100%; font-size: 13px; }
  td { padding: 3px 6px 3px 0; vertical-align: top; }
  .key { white-space: nowrap; color: var(--fg); font-weight: 500; width: 46%; }
</style>
