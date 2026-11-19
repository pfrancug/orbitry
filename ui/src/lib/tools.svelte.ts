import { fetchTools } from "./api";

export const tools = $state({
  vscodeInstalled: null as boolean | null,
  cursorInstalled: null as boolean | null,
});

export async function refreshTools() {
  try {
    const info = await fetchTools();
    tools.vscodeInstalled = info.vscodeInstalled;
    tools.cursorInstalled = info.cursorInstalled ?? false;
  } catch {
    // Keep the last known value if the probe fails. Never break the page.
  }
}
