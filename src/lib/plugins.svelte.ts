// The installed plugins as last listed by the backend, kept up to date while scanning.
import * as api from "./api";

export const plugins = $state({
  list: [] as api.PluginEntry[],
  progress: null as api.ScanProgress | null,
});

let fetching: Promise<void> | null = null;
let again = false;

/** Fetch the plugin list, once more after any fetch already running. */
export function refreshPlugins(): Promise<void> {
  if (fetching) {
    again = true;
    return fetching;
  }
  fetching = (async () => {
    do {
      again = false;
      try {
        const result = await api.listPlugins();
        plugins.list = result.plugins;
        plugins.progress = result.progress;
      } catch {
        // Keep what is shown; the next change refreshes it.
      }
    } while (again);
    fetching = null;
  })();
  return fetching;
}

/** Keep the list current as scan results arrive and plugins are enabled or disabled. */
export function initPlugins() {
  void api.onPluginsChanged((progress) => {
    plugins.progress = progress;
    void refreshPlugins();
  });
  void refreshPlugins();
}
