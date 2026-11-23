<script lang="ts">
  // The same bundle serves the three windows; the hash picks the view.
  import Editor from "./lib/Editor.svelte";
  import Audience from "./lib/Audience.svelte";
  import Presenter from "./lib/Presenter.svelte";
  const view = location.hash.replace("#", "") || "editor";
</script>

{#if view === "audience"}
  <Audience />
{:else if view === "presenter"}
  <Presenter />
{:else}
  <Editor />
{/if}
