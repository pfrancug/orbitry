<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";

  let {
    title = "",
    label = "",
    align = "end",
    children,
  }: {
    title?: string;
    label?: string;
    align?: "start" | "end";
    children: Snippet;
  } = $props();

  let open = $state(false);
  let root: HTMLDivElement | undefined;

  $effect(() => {
    if (!open) return;
    const onPointer = (event: PointerEvent) => {
      if (root && !root.contains(event.target as Node)) open = false;
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") open = false;
    };
    const onClick = (event: MouseEvent) => {
      event.stopPropagation();
      open = false;
    };
    document.addEventListener("pointerdown", onPointer);
    document.addEventListener("keydown", onKey);
    root?.addEventListener("click", onClick);
    return () => {
      document.removeEventListener("pointerdown", onPointer);
      document.removeEventListener("keydown", onKey);
      root?.removeEventListener("click", onClick);
    };
  });
</script>

<div class="action-menu" class:open bind:this={root}>
  <button
    class="icon-btn ghost menu-trigger"
    type="button"
    {title}
    aria-label={title || label || "More"}
    aria-haspopup="menu"
    aria-expanded={open}
    onclick={(event) => {
      event.stopPropagation();
      open = !open;
    }}
  >
    {#if label}
      <span>{label}</span>
    {/if}
    <Icon name="more" />
  </button>
  {#if open}
    <div class="menu-list" class:align-start={align === "start"} role="menu" tabindex="-1">
      {@render children()}
    </div>
  {/if}
</div>

<style>
  .action-menu {
    position: relative;
    display: inline-flex;
  }
  .menu-list {
    position: absolute;
    top: calc(100% + 4px);
    right: 0;
    z-index: 50;
    min-width: 11rem;
    padding: 4px;
    border: 1px solid var(--line-strong);
    border-radius: var(--radius-lg);
    background: rgba(16, 16, 22, 0.98);
    box-shadow: var(--shadow);
    display: flex;
    flex-direction: column;
  }
  .menu-list.align-start {
    right: auto;
    left: 0;
  }
  .menu-list :global(button),
  .menu-list :global(a) {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    margin: 0;
    padding: 7px 10px;
    border: 0;
    border-radius: var(--radius);
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }
  .menu-list :global(button:hover),
  .menu-list :global(a:hover) {
    background: rgba(255, 255, 255, 0.06);
  }
  .menu-list :global(button:disabled) {
    opacity: 0.45;
    cursor: default;
  }
  .menu-list :global(button.danger) {
    color: #fecdd3;
  }
  .menu-list :global(.menu-hint) {
    padding: 6px 10px 4px;
    font-size: 10px;
    font-weight: 650;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
  }
</style>
