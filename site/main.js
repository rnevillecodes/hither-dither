// Points the download links at the latest release's files and draws the hero
// waveform, which dissolves into ordered-dither dots like the app icon.

const REPO = "rnevillecodes/hither-dither";

const PRIMARY = {
  windows: { suffix: "x64-setup.exe", label: "Download for Windows" },
  mac: { suffix: "_universal.dmg", label: "Download for macOS" },
  linux: { suffix: ".AppImage", label: "Download for Linux" },
};

function detectOs() {
  const platform = (navigator.userAgentData?.platform || navigator.platform || "").toLowerCase();
  const ua = navigator.userAgent.toLowerCase();
  if (/android|iphone|ipad|ipod/.test(ua)) return null;
  if (platform.startsWith("win") || ua.includes("windows")) return "windows";
  if (platform.startsWith("mac") || ua.includes("mac os")) return "mac";
  if (platform.includes("linux") || ua.includes("linux")) return "linux";
  return null;
}

function setupDownloads() {
  const os = detectOs();
  const primary = document.getElementById("primary-download");
  const label = document.getElementById("primary-label");

  if (os) {
    label.textContent = PRIMARY[os].label;
    document.querySelector(`.platform[data-os="${os}"]`)?.classList.add("current");
  } else {
    label.textContent = "Download";
    primary.href = "#download";
  }

  // Without the API (offline, rate limited) every link still goes to the
  // latest release page.
  fetch(`https://api.github.com/repos/${REPO}/releases/latest`)
    .then((r) => (r.ok ? r.json() : Promise.reject(r.status)))
    .then((release) => {
      const find = (suffix) => release.assets.find((a) => a.name.endsWith(suffix));

      for (const link of document.querySelectorAll("[data-asset]")) {
        const asset = find(link.dataset.asset);
        if (asset) link.href = asset.browser_download_url;
      }
      if (os) {
        const asset = find(PRIMARY[os].suffix);
        if (asset) primary.href = asset.browser_download_url;
      }

      const version = release.tag_name.replace(/^v/, "");
      const date = new Date(release.published_at).toLocaleDateString(undefined, {
        year: "numeric",
        month: "long",
        day: "numeric",
      });
      document.getElementById("release-meta").textContent =
        `Version ${version} · released ${date} · free for Windows, macOS and Linux`;
      document.getElementById("download-lede").innerHTML =
        `Version ${version}. Older versions and release notes are on the ` +
        `<a href="${release.html_url.replace(/\/tag\/.*$/, "")}">Releases page</a>.`;
    })
    .catch(() => {});
}

// 4x4 Bayer matrix, normalised to 0..1.
const BAYER = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5].map((v) => (v + 0.5) / 16);

function setupDither() {
  const canvas = document.getElementById("dither");
  const ctx = canvas.getContext("2d");
  const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  let width = 0;
  let height = 0;
  let cell = 6;

  function resize() {
    const dpr = window.devicePixelRatio || 1;
    width = canvas.clientWidth;
    height = canvas.clientHeight;
    canvas.width = Math.round(width * dpr);
    canvas.height = Math.round(height * dpr);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    cell = width < 560 ? 5 : 6;
  }

  // A sum of slow sines gives a plausible, non-repeating envelope.
  function amplitude(x, t) {
    const env =
      0.55 +
      0.25 * Math.sin(x * 0.011 + t * 0.6) +
      0.15 * Math.sin(x * 0.037 - t * 1.1) +
      0.08 * Math.sin(x * 0.091 + t * 1.7);
    return Math.max(0.08, Math.min(1, env));
  }

  function draw(t) {
    ctx.clearRect(0, 0, width, height);
    const gradient = ctx.createLinearGradient(0, 0, width, 0);
    gradient.addColorStop(0, "#2fd3c9");
    gradient.addColorStop(0.55, "#7fd98a");
    gradient.addColorStop(1, "#ffc23a");
    ctx.fillStyle = gradient;

    const mid = height / 2;
    const cols = Math.ceil(width / cell);
    const rows = Math.ceil(height / cell);

    for (let c = 0; c < cols; c++) {
      const x = c * cell;
      const half = amplitude(x, t) * mid * 0.92;
      // Solid on the left, fading to sparse dots on the right.
      const solidity = 1 - Math.min(1, Math.max(0, (x / width - 0.3) / 0.65));

      for (let r = 0; r < rows; r++) {
        const y = r * cell + cell / 2;
        if (Math.abs(y - mid) > half) continue;
        const threshold = BAYER[(r % 4) * 4 + (c % 4)];
        if (solidity * 0.85 + 0.12 < threshold) continue;
        ctx.globalAlpha = 0.35 + 0.5 * solidity;
        const size = cell - 1.5;
        ctx.fillRect(x, r * cell + 0.75, size, size);
      }
    }
    ctx.globalAlpha = 1;
  }

  let visible = true;
  new IntersectionObserver(([entry]) => {
    visible = entry.isIntersecting;
  }).observe(canvas);

  function frame(now) {
    if (visible) draw(now / 1000);
    requestAnimationFrame(frame);
  }

  resize();
  window.addEventListener("resize", () => {
    resize();
    if (reduceMotion) draw(0);
  });
  if (reduceMotion) draw(0);
  else requestAnimationFrame(frame);
}

setupDownloads();
setupDither();
