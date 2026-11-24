<script lang="ts">
  // The same bundle serves the three windows; the window's label picks
  // the view.
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Viewer from "./lib/Viewer.svelte";
  import Audience from "./lib/Audience.svelte";
  import Presenter from "./lib/Presenter.svelte";

  let label = "viewer";
  try {
    label = getCurrentWindow().label;
  } catch {
    label = location.hash.replace("#", "") || "viewer";
  }
</script>

{#if label === "audience"}
  <Audience />
{:else if label === "presenter"}
  <Presenter />
{:else}
  <Viewer />
{/if}
