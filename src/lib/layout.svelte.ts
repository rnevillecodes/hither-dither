// Window layout preferences, remembered across restarts.

const DETAILS_KEY = "hither-dither.detailsOpen";

function readDetailsOpen(): boolean {
  try {
    return localStorage.getItem(DETAILS_KEY) !== "false";
  } catch {
    return true;
  }
}

export const layout = $state({
  /** Whether the details panel (metadata and stats) is shown while files are open. */
  detailsOpen: readDetailsOpen(),
});

export function toggleDetails() {
  layout.detailsOpen = !layout.detailsOpen;
  try {
    localStorage.setItem(DETAILS_KEY, String(layout.detailsOpen));
  } catch {
    // Storage unavailable: the choice lasts for this session only.
  }
}
