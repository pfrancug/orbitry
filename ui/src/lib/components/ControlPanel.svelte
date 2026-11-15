<script lang="ts">
  import { ansiToHtml } from "$lib/ansi";
  import { logsSocket } from "$lib/api";
  import { formatPorts, healthLabel, partTitle, portsFor, roleLabel, shortPath, timeAgo } from "$lib/format";
  import {
    closePanel,
    isPending,
    latestRun,
    restartScript,
    runningFor,
    runState,
    selectPart,
    startScript,
    stopScript,
    upsertRun,
  } from "$lib/runs.svelte";
  import type { LogLine, ProjectPart, Run } from "$lib/types";

  let { part }: { part: ProjectPart | null } = $props();

  let lines = $state<LogLine[]>([]);
  let scroller = $state<HTMLElement | undefined>(undefined);
  let pinned = $state(true);
  let socket: WebSocket | null = null;

  const scriptName = $derived.by(() => {
    if (!part) return null;
    if (runState.selectedScript && part.scripts.some((item) => item.name === runState.selectedScript)) {
      return runState.selectedScript;
    }
    return runningFor(part.id)?.script ?? part.scripts[0]?.name ?? null;
  });

  const current = $derived.by(() => {
    if (!part) return null;
    return (
      (scriptName ? runningFor(part.id, scriptName) : null) ??
      (runState.selectedId ? runState.runs.find((run) => run.id === runState.selectedId) ?? null : null) ??
      latestRun(part.id, scriptName) ??
      null
    );
  });

  const live = $derived(part && scriptName ? runningFor(part.id, scriptName) : undefined);
  const pending = $derived(part && scriptName ? isPending(part.id, scriptName) : false);
  const ports = $derived(part ? portsFor(part, runState.runs) : []);
  const health = $derived(live?.health ?? current?.health);

  $effect(() => {
    const id = current?.id;
    const open = runState.panelOpen;
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

  function statusLabel(run: Run | null): string {
    if (!run) return "idle";
    if (run.status === "running") return "running";
    if (run.status === "failed") return run.exitCode == null ? "failed" : `failed (${run.exitCode})`;
    return run.exitCode == null ? "exited" : `exited (${run.exitCode})`;
  }
</script>

<aside class="control-panel" aria-label="Control panel">
  {#if !part}
    <div class="panel-empty">
      <strong>Control panel</strong>
      <p>Select an app to start, stop, and watch its logs.</p>
    </div>
  {:else}
    <header class="panel-head">
      <div>
        <div class="title-row">
          <span class="badge role role-{part.role}">{roleLabel(part.role)}</span>
          <h2 title={part.name}>{partTitle(part)}</h2>
        </div>
        <p class="path" title={part.path}>{shortPath(part.path)}</p>
      </div>
      <button class="text-btn panel-close" type="button" onclick={closePanel}>Close</button>
    </header>

    <div class="panel-meta">
      {#if part.version}<span class="badge quiet">v{part.version}</span>{/if}
      {#if part.framework}<span class="badge framework">{part.framework}</span>{/if}
      {#if part.git}
        <span class="git">
          <span class="dot {part.git.dirty ? 'dirty' : 'clean'}"></span>
          {part.git.branch}
        </span>
      {/if}
      <span class="build" class:ok={part.build.built}>
        {part.build.built ? `Built ${timeAgo(part.build.lastBuiltAt)}` : "Not built"}
      </span>
    </div>

    <div class="panel-controls">
      {#if part.scripts.length === 0}
        <p class="hint">No dev or preview scripts on this app.</p>
      {:else}
        <label class="script-picker">
          <span>Script</span>
          <select
            value={scriptName ?? ""}
            onchange={(event) => {
              const value = (event.currentTarget as HTMLSelectElement).value;
              if (value) selectPart(part.id, value);
            }}
          >
            {#each part.scripts as script (script.kind + script.name)}
              <option value={script.name}>{script.name} · {script.scope}</option>
            {/each}
          </select>
        </label>
        <div class="panel-actions">
          {#if live}
            <button
              class="btn danger"
              type="button"
              disabled={pending}
              onclick={() => scriptName && stopScript(part.id, scriptName)}
            >
              Stop
            </button>
            <button
              class="btn"
              type="button"
              disabled={pending}
              onclick={() => scriptName && restartScript(part.id, scriptName)}
            >
              Restart
            </button>
          {:else}
            <button
              class="btn primary"
              type="button"
              disabled={pending || !scriptName}
              onclick={() => scriptName && startScript(part.id, scriptName)}
            >
              Start
            </button>
          {/if}
        </div>
      {/if}
    </div>

    <div class="panel-status">
      <span class="log-status status-{current?.status ?? 'idle'}">{statusLabel(current)}</span>
      <span class="port-cell">{formatPorts(ports) || "no port yet"}</span>
      <span class="health health-{health?.status ?? 'unknown'}" title={health?.url ?? ""}>
        {healthLabel(health)}
      </span>
      {#if current?.pid}
        <span class="muted">pid {current.pid}</span>
      {/if}
    </div>

    {#if !pinned}
      <p class="scroll-hint">Auto-scroll paused</p>
    {/if}
    <div class="log-body" bind:this={scroller} onscroll={onScroll}>
      {#if !current}
        <p class="hint">Start a script to stream logs here.</p>
      {:else if lines.length === 0}
        <p class="hint">Waiting for output...</p>
      {:else}
        {#each lines as line (line.seq)}
          <div class="log-line stream-{line.stream}">{@html ansiToHtml(line.text)}</div>
        {/each}
      {/if}
    </div>
  {/if}
</aside>
