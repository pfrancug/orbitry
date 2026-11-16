<script lang="ts">
  import { page } from "$app/state";
  import { runState } from "$lib/runs.svelte";
  import { i18n, t } from "$lib/i18n.svelte";
  import Icon from "./Icon.svelte";

  let {
    collapsed,
    ontoggle,
  }: {
    collapsed: boolean;
    ontoggle: () => void;
  } = $props();

  const running = $derived(runState.runs.filter((run) => run.status === "running").length);

  const locale = $derived(i18n.locale);
  const items = $derived([
    { href: "/", label: t("nav.overview"), icon: "overview", exact: true },
    { href: "/projects", label: t("nav.projects"), icon: "projects", exact: false },
    { href: "/models", label: t("nav.models"), icon: "models", exact: false },
    { href: "/logs", label: t("nav.logs"), icon: "logs", exact: false },
    { href: "/settings", label: t("nav.settings"), icon: "settings", exact: false },
  ]);

  function active(href: string, exact: boolean) {
    const path = page.url.pathname;
    return exact ? path === href : path === href || path.startsWith(`${href}/`);
  }
</script>

<aside class="sidebar" class:collapsed aria-label={t("nav.overview")} data-locale={locale}>
  <div class="sidebar-brand">
    <img class="mark" src="/orbitry.svg" width="28" height="28" alt="" />
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
