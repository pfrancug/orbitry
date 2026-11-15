<script lang="ts">
  import { timeAgo } from "$lib/format";
  import type { BuildInfo, GitInfo } from "$lib/types";

  let { git, build }: { git: GitInfo | null; build: BuildInfo } = $props();
</script>

<div class="card-foot">
  <span class="git">
    {#if git}
      <span
        class="dot {git.dirty ? 'dirty' : 'clean'}"
        title={git.dirty ? "Uncommitted changes" : "Clean working tree"}
      ></span>
      <span title={git.commit}>{git.branch}</span>
    {:else}
      <span class="muted">No git</span>
    {/if}
  </span>
  <span class="build" class:ok={build.built}>
    {build.built ? `Built ${timeAgo(build.lastBuiltAt)}` : "Not built"}
  </span>
</div>