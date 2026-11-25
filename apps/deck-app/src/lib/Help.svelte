<script lang="ts">
  let { onclose }: { onclose: () => void } = $props();
  const sections: [string, [string, string][]][] = [
    ["Deck", [
      ["New · Open file… · Open .deck folder…", "Start a deck; import a PowerPoint file or open a single-file deck (.deckz); open a deck folder"],
      ["Save · Save as…", "Write the deck folder (deck.json + assets/), or a single .deckz file when saved under that extension"],
      ["Export PPTX… · Export PDF…", "PowerPoint file; PDF with bookmarks for sections and slides"],
    ]],
    ["Slides", [
      ["Click a thumbnail", "Go to that slide"],
      ["Right-click between or below thumbnails", "New slide, new section or new subsection there (a section starts with a new slide)"],
      ["Right-click a thumbnail", "New slide after, duplicate, delete, start or remove a section or subsection"],
      ["Drag a thumbnail · Ctrl+Up / Ctrl+Down", "Reorder slides; sections travel with their first slide"],
      ["+ Slide · Duplicate · − Slide", "New slide from the current layout; copy; remove"],
      ["Page Up / Page Down", "Previous or next slide"],
      ["Insert from deck…", "Copy slides from another deck, with their images and layouts"],
      ["+ section · + subsection (hover a thumbnail)", "Start a section there; click a header to rename, clear to remove"],
    ]],
    ["Elements", [
      ["Click · Shift+click", "Select; add to the selection; drag to move; corner handle to resize; Delete to remove"],
      ["Drag on the empty slide", "A rubber band: selects every element it touches"],
      ["Round handle above the selection", "Rotates the element (Shift: steps of 15°; it catches at 0, 90, 180, 270); double-click to reset. At a later click it makes a rotation step"],
      ["Drag", "Moves the whole selection; edges and centers snap to the slide and to other elements, with a guide line (hold Alt to move freely)"],
      ["Arrange bar", "Align to the selection (or to the slide for one element), distribute with equal gaps, mirror, bring forward or send back, stroke color and width"],
      ["Group · Ungroup (Ctrl+G · Ctrl+Shift+G)", "Several elements become one, moved, resized and animated together"],
      ["+ Table", "A table with a header row; double-click a cell to type in it (Tab: next cell); rows, columns, widths and fills in the panel, the rules in the arrange bar"],
      ["+ Shape", "A rectangle, an ellipse, a line or an arrow in the theme's accent color; the style bar sets fill, outline and, for lines, the direction"],
      ["+ Text · + Plan · + Figure", "A text box; the sections list with the current one highlighted; a Typst figure (CeTZ plots and drawings) edited as source"],
      ["Ctrl+Z · Ctrl+Y (or Ctrl+Shift+Z)", "Undo and redo any edit of the deck; a run of typing in one box is one step"],
      ["Ctrl+C · Ctrl+X · Ctrl+V", "Copy or cut the selected elements; paste them (also into another deck or slide, images shared within a deck, copied across decks), a clipboard image, or text into the box being edited"],
      ["Drop a file · + Video…", "Drop image, video or GIF files on the canvas where they should go, or pick a clip"],
      ["Playback (panel)", "A video or GIF plays when it appears or on its play button, looping or once; sound off by default"],
      ["Double-click a text", "Edit in place: type, Backspace/Delete, arrows, Home/End, Shift+arrows to select, Ctrl+A"],
      ["Mouse in a text being edited", "Click places the caret, drag selects, double-click selects a word, triple-click a paragraph"],
      ["Enter · Escape", "New paragraph · finish editing"],
      ["Alt+=", "Inline formula: types $$ with the caret inside (while editing text)"],
      ["Ctrl+B · Ctrl+I", "Bold · italic (selection, or the whole box when not editing)"],
      ["Style bar", "Size, color, font, alignment, bullets; fill and outline of boxes and shapes; plan colors"],
      ["Image panel", "Crop to a shape; percentage cut from each edge"],
      ["Build steps panel", "Appear or disappear on click, move steps (drag the element while viewing the step), rotation, fill, timing"],
      ["◀ ▶ under the canvas", "View the slide at a given click"],
    ]],
    ["Layouts", [
      ["Layout selector · Edit layout · + New layout", "The slide's layout; open the layout editor; create one"],
      ["Select a placeholder", "Its default text style in the panel: size, bold, color, font, alignment, bullets"],
      ["+ Placeholder · Save layout… · Load layout…", "Slots new slides start from; save as .deck_layout; load one"],
      ["{{section}} {{subsection}} {{slide}} {{slides}} {{title}} {{deck}} {{date}}", "Variables in layout texts, filled per slide"],
      ["Theme (panel)", "Default font, size, text color, accent and palette of the deck, or of the layout under edit; fonts/ in the deck adds fonts"],
    ]],
    ["Macros", [
      ["Macros (panel) · Run script…", "Python scripts in the deck's scripts/ folder, or any .py file, run against the open deck with the decker package; the result is one undo step"],
    ]],
    ["Presenting", [
      ["P (presenter window) · Pointer button", "Mirror the mouse over the current slide as a red dot on the audience window"],
      ["D · C (presenter window) · Pen button", "Draw on the current slide, shown to the audience; C or leaving the slide erases it"],
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
