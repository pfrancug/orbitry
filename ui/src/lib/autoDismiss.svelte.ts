/** Shared auto-dismiss for overlay toasts/banners. Pause while hovered. */

export type AutoDismissOpts = {
  /** Milliseconds before dismiss. */
  ms: number;
  /** Called when the timer fires (and hover is clear). */
  dismiss: () => void;
  /** When false, the timer is not armed. */
  enabled?: boolean;
};

/**
 * Attach mouseenter/mouseleave pause + a timeout. Returns a cleanup function
 * for use inside `$effect`.
 */
export function armAutoDismiss(
  node: HTMLElement | null | undefined,
  opts: AutoDismissOpts,
): () => void {
  if (!node || opts.enabled === false) return () => {};
  let timer: ReturnType<typeof setTimeout> | null = null;
  let paused = false;
  let remaining = opts.ms;
  let started = Date.now();

  const clear = () => {
    if (timer) {
      clearTimeout(timer);
      timer = null;
    }
  };

  const schedule = () => {
    clear();
    started = Date.now();
    timer = setTimeout(() => {
      timer = null;
      if (!paused) opts.dismiss();
    }, remaining);
  };

  const onEnter = () => {
    if (paused) return;
    paused = true;
    remaining = Math.max(0, remaining - (Date.now() - started));
    clear();
  };

  const onLeave = () => {
    if (!paused) return;
    paused = false;
    if (remaining <= 0) {
      opts.dismiss();
      return;
    }
    schedule();
  };

  node.addEventListener("pointerenter", onEnter);
  node.addEventListener("pointerleave", onLeave);
  schedule();

  return () => {
    clear();
    node.removeEventListener("pointerenter", onEnter);
    node.removeEventListener("pointerleave", onLeave);
  };
}
