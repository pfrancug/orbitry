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
    { href: "/databases", label: t("nav.databases"), icon: "db", exact: false },
    { href: "/models", label: t("nav.models"), icon: "models", exact: false },
    { href: "/logs", label: t("nav.logs"), icon: "logs", exact: false },
    { href: "/settings", label: t("nav.settings"), icon: "settings", exact: false },
  ]);

  function active(href: string, exact: boolean) {
    const path = page.url.pathname;
    return exact ? path === href : path === href || path.startsWith(`${href}/`);
  }
</script>

<aside class="sidebar" class:collapsed aria-label={t("nav.sidebar")} data-locale={locale}>
  <div class="sidebar-brand">
    <span class="nav-icon">
      <img class="mark" src="/orbitry.svg" width="28" height="28" alt="" />
    </span>
    <div class="sidebar-brand-copy">
      <p class="logo">Orbitry</p>
      <p class="tagline">{t("nav.tagline")}</p>
    </div>
  </div>

  <nav class="sidebar-nav">
    {#each items as item (item.href)}
      <a
        class="nav-item"
        class:active={active(item.href, item.exact)}
        href={item.href}
        title={item.label}
      >
        <span class="nav-icon">
          <Icon name={item.icon} size={18} />
        </span>
        <span class="nav-label">{item.label}</span>
        {#if item.href === "/" && running > 0}
          <em class="nav-badge">{running}</em>
          <em class="nav-dot" aria-label={t("nav.runningCount", { count: running })}></em>
        {/if}
      </a>
    {/each}
  </nav>

  <button
    class="sidebar-toggle"
    type="button"
    onclick={ontoggle}
    title={collapsed ? t("nav.expand") : t("nav.collapse")}
    aria-label={collapsed ? t("nav.expand") : t("nav.collapse")}
    aria-expanded={!collapsed}
  >
    <span class="nav-icon">
      <Icon name={collapsed ? "expand" : "collapse"} />
    </span>
    <span class="sidebar-toggle-label">{t("nav.collapse")}</span>
  </button>
</aside>
