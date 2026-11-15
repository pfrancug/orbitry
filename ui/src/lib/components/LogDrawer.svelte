<script lang="ts">
  import { fly } from "svelte/transition";
  import { ansiToHtml } from "$lib/ansi";
  import { logsSocket } from "$lib/api";
  import { closeDrawer, openRun, runState, stopSelected, upsertRun } from "$lib/runs.svelte";
  import type { LogLine, Run } from "$lib/types";

  let lines = $state<LogLine[]>([]);
  let scroller = $state<HTMLElement | undefined>(undefined);
  let pinned = $state(true);
  let socket: WebSocket | null = null;

  const selected = $derived(runState.runs.find((run) => run.id === runState.selectedId) ?? null);
  const tabs = $derived.by(() => {
    const running = runState.runs.filter((run) => run.status === "running");
    if (selected && !running.some((run) => run.id === selected.id)) {
      return [...running, selected];
    }
    return running;
  });

  $effect(() => {
    const id = runState.selectedId;
    const open = runState.drawerOpen;
    if (!open || !id) {
      socket?.close();
      socket = null;
      return;
    }
    lines = [];
    pinned = true;
    const next = logsSocket(id);
    socket = next;
    next.onmessage = (event) => {
      try {
        const message = JSON.parse(event.data as string) as
          | { type: "replay"; run: Run; lines: LogLine[] }
          | (LogLine & { type: "line" })
          | { type: "status"; run: Run };
        if (message.type === "replay") {
          upsertRun(message.run);
          lines = message.lines;
        } else if (message.type === "line") {
          const { type: _type, ...line } = message;
          lines = [...lines, line];
          if (lines.length > 2500) lines = lines.slice(-2000);
        } else if (message.type === "status") {
          upsertRun(message.run);
        }
      } catch {
        // Ignore a malformed log frame.
      }
    };
    return () => {
      next.close();
      if (socket === next) socket = null;
    };
  });

  $effect(() => {
    lines;
    if (!pinned || !scroller) return;
    requestAnimationFrame(() => {
      if (scroller && pinned) scroller.scrollTop = scroller.scrollHeight;
    });
  });

  function onScroll() {
    if (!scroller) return;
    const distance = scroller.scrollHeight - scroller.scrollTop - scroller.clientHeight;
    pinned = distance < 48;
  }

  function statusLabel(run: Run): string {
    if (run.status === "running") return "running";
    if (run.status === "failed") return run.exitCode == null ? "failed" : `failed (${run.exitCode})`;
    return run.exitCode == null ? "exited" : `exited (${run.exitCode})`;
  }
</script>

{#if runState.drawerOpen}
  <aside class="log-drawer" transition:fly={{ y: 28, duration: 220 }} aria-label="Process logs">
    <header class="log-head">
      <div class="log-tabs" role="tablist">
        {#each tabs as run (run.id)}
          <button
            class="log-tab"
            class:active={run.id === runState.selectedId}
            class:running={run.status === "running"}
            type="button"
            role="tab"
            aria-selected={run.id === runState.selectedId}
            onclick={() => openRun(run.id)}
          >
            {#if run.status === "running"}<span class="pulse" aria-hidden="true"></span>{/if}
            <span class="tab-name">{run.partName}</span>
            <span class="tab-script">{run.script}</span>
          </button>
        {/each}
        {#if tabs.length === 0}
          <p class="hint">No running processes.</p>
        {/if}
      </div>
      <div class="log-actions">
        {#if selected}
          <span class="log-status status-{selected.status}">{statusLabel(selected)}</span>
          {#if selected.status === "running"}
            <button class="text-btn" type="button" onclick={stopSelected}>Stop</button>
          {/if}
        {/if}
        <button class="text-btn" type="button" onclick={closeDrawer}>Close</button>
      </div>
    </header>
    {#if !pinned}
      <p class="scroll-hint">Auto-scroll paused</p>
    {/if}
    <div class="log-body" bind:this={scroller} onscroll={onScroll}>
      {#if lines.length === 0}
        <p class="hint">Waiting for output...</p>
      {:else}
        {#each lines as line (line.seq)}
          <div class="log-line stream-{line.stream}">{@html ansiToHtml(line.text)}</div>
        {/each}
      {/if}
    </div>
  </aside>
{/if}
