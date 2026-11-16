<script lang="ts">
  let { name, size = 16 }: { name: string; size?: number } = $props();
</script>

<svg
  class="icon"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.75"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if name === "overview"}
    <rect x="3" y="3" width="7" height="9" rx="1.5" />
    <rect x="14" y="3" width="7" height="5" rx="1.5" />
    <rect x="14" y="12" width="7" height="9" rx="1.5" />
    <rect x="3" y="16" width="7" height="5" rx="1.5" />
  {:else if name === "projects"}
    <path d="M4 7h16v12H4z" />
    <path d="M8 7V5h8v2" />
    <path d="M4 12h16" />
  {:else if name === "models"}
    <circle cx="12" cy="6" r="2.4" />
    <circle cx="6" cy="17" r="2.4" />
    <circle cx="18" cy="17" r="2.4" />
    <path d="M12 8.4v2.2M7.8 15.2l2.6-2.3M16.2 15.2l-2.6-2.3" />
  {:else if name === "logs"}
    <path d="M5 4h14v16H5z" />
    <path d="M8 8h8M8 12h8M8 16h5" />
  {:else if name === "settings"}
    <circle cx="12" cy="12" r="3" />
    <path
      d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8V9c.3.6.9 1 1.5 1.1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"
    />
  {:else if name === "refresh"}
    <path d="M21 12a9 9 0 1 1-2.6-6.4" />
    <path d="M21 3v6h-6" />
  {:else if name === "close"}
    <path d="M6 6l12 12M18 6L6 18" />
  {:else if name === "panel"}
    <rect x="3" y="4" width="18" height="16" rx="2" />
    <path d="M15 4v16" />
  {:else if name === "external"}
    <path d="M14 5h5v5" />
    <path d="M19 5l-9 9" />
    <path d="M19 13v5a1 1 0 0 1-1 1H6a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1h5" />
  {:else if name === "play"}
    <path d="M8 5v14l12-7z" fill="currentColor" stroke="none" />
  {:else if name === "stop"}
    <rect x="6" y="6" width="12" height="12" rx="1.5" fill="currentColor" stroke="none" />
  {:else if name === "restart"}
    <path d="M3 12a9 9 0 1 0 3-6.7" />
    <path d="M3 4v5h5" />
  {:else if name === "star"}
    <path d="M12 3l2.6 5.4L21 9.3l-4.5 4.2L17.7 21 12 17.8 6.3 21l1.2-7.5L3 9.3l6.4-.9z" />
  {:else if name === "star-fill"}
    <path
      d="M12 3l2.6 5.4L21 9.3l-4.5 4.2L17.7 21 12 17.8 6.3 21l1.2-7.5L3 9.3l6.4-.9z"
      fill="currentColor"
      stroke="none"
    />
  {:else if name === "collapse"}
    <path d="M15 6l-6 6 6 6" />
  {:else if name === "expand"}
    <path d="M9 6l6 6-6 6" />
  {:else if name === "menu"}
    <path d="M4 7h16M4 12h16M4 17h16" />
  {:else if name === "activity"}
    <path d="M3 12h4l3 8 4-16 3 8h4" />
  {:else if name === "port"}
    <circle cx="12" cy="12" r="3" />
    <path d="M12 3v3M12 18v3M3 12h3M18 12h3" />
  {:else if name === "heart"}
    <path d="M12 21s-7-4.6-9-9.2C1.5 8.2 3.4 5 7 5c2 0 3.2 1.1 5 3 1.8-1.9 3-3 5-3 3.6 0 5.5 3.2 4 6.8C19 16.4 12 21 12 21z" />
  {:else if name === "alert"}
    <path d="M12 8v5" />
    <circle cx="12" cy="17" r="0.8" fill="currentColor" stroke="none" />
    <path d="M12 3 3 20h18z" />
  {:else if name === "search"}
    <circle cx="11" cy="11" r="6" />
    <path d="M20 20l-4-4" />
  {:else if name === "folder"}
    <path d="M3 7h6l2 2h10v10H3z" />
  {:else if name === "terminal"}
    <rect x="3" y="5" width="18" height="14" rx="2" />
    <path d="M7 10l3 2-3 2M13 14h4" />
  {:else if name === "code"}
    <path d="M8 8l-4 4 4 4M16 8l4 4-4 4" />
  {:else if name === "more"}
    <circle cx="6" cy="12" r="1.4" fill="currentColor" stroke="none" />
    <circle cx="12" cy="12" r="1.4" fill="currentColor" stroke="none" />
    <circle cx="18" cy="12" r="1.4" fill="currentColor" stroke="none" />
  {/if}
</svg>
