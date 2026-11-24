<script lang="ts">
  // The viewer's main window: open a PDF, see its pages, write notes,
  // start the show. Once a PDF is open, the page loader reports its
  // page count and size to the Rust side, which the windows then read.
  import { onMount } from "svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { show, presentationKeys } from "$shared";
  import { api, watchView, type View } from "./api";
  import { info } from "./pdf";
  import PdfPage from "./PdfPage.svelte";

  let view = $state<View | null>(null);
  let status = $state("");
  let loaded = "";
  let notesDraft = $state("");

  onMount(() =>
    watchView(async (v) => {
      view = v;
      notesDraft = v.notes;
      if (v.pdf && v.pdf !== loaded) {
        loaded = v.pdf;
        try {
          const i = await info(v.pdf);
          await api.setPages(i.pages, i.width, i.height);
          status = `${v.pdf}: ${i.pages} pages`;
        } catch (e) {
          status = `Cannot read ${v.pdf}: ${e}`;
        }
      }
    }),
  );

  async function openFile() {
    const picked = await openDialog({ multiple: false, filters: [{ name: "PDF", extensions: ["pdf"] }] });
    if (typeof picked === "string") {
      try { await api.open(picked); } catch (e) { status = String(e); }
    }
  }
  async function present(both: boolean) {
    try { await show.startPresentation(both); } catch (e) { status = String(e); }
  }
  function onKey(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement).tagName;
    if (tag === "TEXTAREA" || tag === "INPUT") return;
    if (view?.pdf && presentationKeys(e)) e.preventDefault();
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="viewer">
  <div class="toolbar">
    <button onclick={openFile}>Open PDF…</button>
    <span class="spacer"></span>
    {#if view?.presenting}
      <button onclick={() => show.stopPresentation()}>End presentation (Esc)</button>
    {:else}
      <button class="primary" disabled={!view?.slide_count} onclick={() => present(false)} title="Fullscreen on the second monitor, with the presenter view on this one">Present</button>
      <button disabled={!view?.slide_count} onclick={() => present(true)} title="Also open the presenter view on a single monitor">Present + presenter</button>
    {/if}
  </div>
  {#if view?.pdf && view.slide_count}
    <div class="body">
      <div class="pages">
        {#each { length: view.slide_count } as _, i}
          <button class="thumb" class:current={i === view.slide} onclick={() => show.goto(i)}>
            <PdfPage path={view.pdf} index={i} width={view.width} height={view.height} />
            <span class="n">{i + 1}</span>
          </button>
        {/each}
      </div>
      <div class="main">
        <div class="stage"><PdfPage path={view.pdf} index={view.slide} width={view.width} height={view.height} fit="box" /></div>
        <div class="under">Page {view.slide + 1} / {view.slide_count} · arrows, space, Page Up and Down move; B and W blank the screen</div>
      </div>
      <div class="side">
        <h3>Notes for page {view.slide + 1}</h3>
        <textarea rows="14" bind:value={notesDraft} onchange={() => api.setNotes(view!.slide, notesDraft)}></textarea>
        <p class="muted">Saved next to the PDF as <code>{view.pdf.replace(/\.pdf$/i, "")}.notes.md</code>, one <code># page</code> section per page. Shown in the presenter view.</p>
      </div>
    </div>
  {:else}
    <div class="empty">
      <p>Open a PDF to present it: any PDF works, decker's exports, Beamer, PowerPoint.</p>
      <p class="muted">The audience window goes fullscreen on the second monitor; the presenter view shows the current and next pages, your notes, a clock and a pointer.</p>
    </div>
  {/if}
  <div class="status">{status}</div>
</div>

<style>
  .viewer { display: flex; flex-direction: column; height: 100vh; }
  .toolbar { display: flex; gap: 8px; padding: 8px 12px; border-bottom: 1px solid var(--border); align-items: center; }
  .spacer { flex: 1; }
  .body { flex: 1; display: grid; grid-template-columns: 150px 1fr 280px; min-height: 0; }
  .pages { overflow-y: auto; padding: 8px; display: flex; flex-direction: column; gap: 8px; border-right: 1px solid var(--border); }
  .thumb { padding: 2px; position: relative; }
  .thumb.current { border-color: var(--accent); }
  .thumb .n { position: absolute; left: 4px; top: 2px; font-size: 11px; background: rgba(0,0,0,0.6); color: white; padding: 0 4px; border-radius: 4px; }
  .main { display: flex; flex-direction: column; align-items: center; justify-content: center; padding: 16px; gap: 8px; min-height: 0; }
  .stage { width: 100%; flex: 1; min-height: 0; display: flex; align-items: center; justify-content: center; }
  .under { color: var(--muted); font-size: 12px; }
  .side { padding: 12px; border-left: 1px solid var(--border); display: flex; flex-direction: column; gap: 8px; }
  .side h3 { margin: 0; font-size: 13px; text-transform: uppercase; color: var(--muted); }
  .side textarea { width: 100%; resize: vertical; }
  .muted { color: var(--muted); font-size: 12px; }
  .empty { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; padding: 40px; text-align: center; max-width: 560px; margin: 0 auto; }
  .status { padding: 4px 12px; border-top: 1px solid var(--border); color: var(--muted); font-size: 12px; min-height: 24px; }
</style>
