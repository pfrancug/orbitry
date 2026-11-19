<script lang="ts">
  import { ansiToHtml } from "$lib/ansi";
  import { logsSocket } from "$lib/api";
  import { t } from "$lib/i18n.svelte";
  import { upsertRun } from "$lib/runs.svelte";
  import type { LogLine, Run } from "$lib/types";

  let { runId }: { runId: string | null } = $props();

  let lines = $state<LogLine[]>([]);
  let scroller = $state<HTMLElement | undefined>(undefined);
  let pinned = $state(true);
  let socket: WebSocket | null = null;
  let runStatus = $state<Run["status"] | null>(null);

  $effect(() => {
    const id = runId;
    if (!id) {
      socket?.close();
      socket = null;
      lines = [];
      runStatus = null;
      return;
    }
    lines = [];
    runStatus = null;
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
          runStatus = message.run.status;
          lines = message.lines;
        } else if (message.type === "line") {
          const { type: _type, ...line } = message;
          lines = [...lines, line];
          if (lines.length > 2500) lines = lines.slice(-2000);
        } else if (message.type === "status") {
          upsertRun(message.run);
          runStatus = message.run.status;
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
</script>

<div class="log-viewer">
  {#if !pinned}
    <p class="scroll-hint">Auto-scroll paused</p>
  {/if}
  <div class="log-body" bind:this={scroller} onscroll={onScroll}>
    {#if !runId}
      <p class="hint">{t("logs.pick")}</p>
    {:else if lines.length === 0}
      <p class="hint">{runStatus === "running" ? t("logs.noOutputRunning") : t("logs.noOutput")}</p>
    {:else}
      {#each lines as line (line.seq)}
        <div class="log-line stream-{line.stream}">{@html ansiToHtml(line.text)}</div>
      {/each}
    {/if}
  </div>
</div>
