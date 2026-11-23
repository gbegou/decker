<script lang="ts">
  // The same bundle serves the three windows: the window's label picks
  // the view (a URL fragment is not reliable on every platform). The hash
  // remains as a fallback for a plain browser.
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Editor from "./lib/Editor.svelte";
  import Audience from "./lib/Audience.svelte";
  import Presenter from "./lib/Presenter.svelte";

  let label = "editor";
  try {
    label = getCurrentWindow().label;
  } catch {
    label = location.hash.replace("#", "") || "editor";
  }
</script>

{#if label === "audience"}
  <Audience />
{:else if label === "presenter"}
  <Presenter />
{:else}
  <Editor />
{/if}
