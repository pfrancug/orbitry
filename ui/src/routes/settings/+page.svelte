<script lang="ts">
  import SettingsForm from "$lib/components/SettingsForm.svelte";
  import { i18n, t } from "$lib/i18n.svelte";
  const locale = $derived(i18n.locale);
</script>

<div class="page-head" data-locale={locale}>
  <div>
    <h1>{t("settings.title")}</h1>
    <p>{t("settings.subtitle")}</p>
  </div>
</div>

<div class="settings-page">
  <SettingsForm />
</div>
