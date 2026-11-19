<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    children,
    content,
    disabled = false,
  }: {
    children: Snippet;
    content: Snippet;
    disabled?: boolean;
  } = $props();

  let trigger = $state<HTMLSpanElement | undefined>(undefined);
  let layer = $state<HTMLDivElement | undefined>(undefined);
  let open = $state(false);
  let placed = $state({ top: 0, left: 0, maxHeight: 240 });

  function place() {
    if (!trigger || !layer) return;
    const gap = 6;
    const pad = 8;
    const rect = trigger.getBoundingClientRect();
    const vw = window.innerWidth;
    const vh = window.innerHeight;

    // Measure with current styles.
    const pop = layer.getBoundingClientRect();
    const width = pop.width || 240;
    const height = Math.min(pop.height || 160, 240);

    let top = rect.bottom + gap;
    let left = rect.left;
    let maxHeight = Math.min(240, vh - pad * 2);

    if (top + height > vh - pad && rect.top - gap - height >= pad) {
      top = rect.top - gap - height;
    } else if (top + height > vh - pad) {
      maxHeight = Math.max(120, vh - pad - top);
    }

    if (left + width > vw - pad) {
      left = Math.max(pad, vw - pad - width);
    }
    if (left < pad) left = pad;

    placed = { top, left, maxHeight };
  }

  function show() {
    if (disabled) return;
    open = true;
    queueMicrotask(place);
    requestAnimationFrame(place);
  }

  function hide() {
    open = false;
  }

  function onLeave(event: MouseEvent) {
    const next = event.relatedTarget as Node | null;
    if (layer && next && layer.contains(next)) return;
    if (trigger && next && trigger.contains(next)) return;
    hide();
  }

  $effect(() => {
    if (!open) return;
    place();
    const onScroll = () => place();
    const onResize = () => place();
    window.addEventListener("scroll", onScroll, true);
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("scroll", onScroll, true);
      window.removeEventListener("resize", onResize);
    };
  });
</script>

<span
  class="hover-popover-trigger"
  bind:this={trigger}
  onmouseenter={show}
  onmouseleave={onLeave}
  onfocusin={show}
  onfocusout={(event) => {
    const next = event.relatedTarget as Node | null;
    if (layer && next && layer.contains(next)) return;
    hide();
  }}
  role="presentation"
>
  {@render children()}
</span>

{#if open}
  <div
    class="hover-popover-layer"
    bind:this={layer}
    style={`top:${placed.top}px;left:${placed.left}px;max-height:${placed.maxHeight}px`}
    onmouseenter={show}
    onmouseleave={onLeave}
    role="tooltip"
  >
    {@render content()}
  </div>
{/if}

<style>
  .hover-popover-trigger {
    display: inline-flex;
    align-items: center;
    position: relative;
  }
  .hover-popover-layer {
    position: fixed;
    z-index: 1000;
    min-width: 240px;
    max-width: min(360px, calc(100vw - 16px));
    overflow: auto;
    overscroll-behavior: contain;
    padding: 8px;
    border-radius: 10px;
    border: 1px solid var(--line-strong);
    background: rgba(14, 14, 19, 0.98);
    box-shadow: var(--shadow);
    color: var(--text);
    font-weight: 400;
    letter-spacing: 0;
    text-transform: none;
    white-space: normal;
    font-size: 12px;
  }
</style>
