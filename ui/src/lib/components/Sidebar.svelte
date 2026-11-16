<script lang="ts">
  import { page } from "$app/state";
  import { runState } from "$lib/runs.svelte";
  import Icon from "./Icon.svelte";

  let {
    collapsed,
    ontoggle,
  }: {
    collapsed: boolean;
    ontoggle: () => void;
  } = $props();

  const running = $derived(runState.runs.filter((run) => run.status === "running").length);

  const items = [
    { href: "/", label: "Overview", icon: "overview", exact: true },
    { href: "/projects", label: "Projects", icon: "projects", exact: false },
    { href: "/models", label: "Modele AI", icon: "models", exact: false },
    { href: "/logs", label: "Logs", icon: "logs", exact: false },
    { href: "/settings", label: "Settings", icon: "settings", exact: false },
  ];

  function active(href: string, exact: boolean) {
    const path = page.url.pathname;
    return exact ? path === href : path === href || path.startsWith(`${href}/`);
  }
</script>

<aside class="sidebar" class:collapsed aria-label="Navigation">
  <div class="sidebar-brand">
    <span class="mark" aria-hidden="true"></span>
    {#if !collapsed}
      <div>
        <p class="logo">Orbitry</p>
        <p class="tagline">Control plane</p>
      </div>
    {/if}
  </div>

  <nav class="sidebar-nav">
    {#each items as item (item.href)}
      <a
        class="nav-item"
        class:active={active(item.href, item.exact)}
        href={item.href}
        title={item.label}
      >
        <Icon name={item.icon} size={18} />
        {#if !collapsed}
          <span>{item.label}</span>
          {#if item.href === "/" && running > 0}
            <em class="nav-badge">{running}</em>
          {/if}
        {:else if item.href === "/" && running > 0}
          <em class="nav-dot" aria-label="{running} running"></em>
        {/if}
      </a>
    {/each}
  </nav>

  <button
    class="sidebar-toggle"
    type="button"
    onclick={ontoggle}
    title={collapsed ? "Expand sidebar" : "Collapse sidebar"}
    aria-label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
  >
    <Icon name={collapsed ? "expand" : "collapse"} />
    {#if !collapsed}<span>Collapse</span>{/if}
  </button>
</aside>
