import { fetchTools } from "./api";

export const tools = $state({
  vscodeInstalled: null as boolean | null,
});

export async function refreshTools() {
  try {
    tools.vscodeInstalled = (await fetchTools()).vscodeInstalled;
  } catch {
    // Keep the last known value if the probe fails.
  }
}
