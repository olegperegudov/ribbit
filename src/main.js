const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWindow } = window.__TAURI__.window;

// Sounds — load real quack sample, play with Web Audio API for pitch control
const audioCtx = new (window.AudioContext || window.webkitAudioContext)();
let quackBuffer = null;

fetch("quack.ogg")
  .then(r => r.arrayBuffer())
  .then(buf => audioCtx.decodeAudioData(buf))
  .then(decoded => { quackBuffer = decoded; });

function playQuack(rate = 1.0, volume = 0.8) {
  if (!quackBuffer) return;
  const src = audioCtx.createBufferSource();
  const gain = audioCtx.createGain();
  src.buffer = quackBuffer;
  src.playbackRate.value = rate;
  gain.gain.value = volume;
  src.connect(gain);
  gain.connect(audioCtx.destination);
  src.start();
}

function playStartQuack() { playQuack(1.15, 0.8); }
function playStopQuack() { playQuack(0.85, 0.6); }

const $ = (sel) => document.querySelector(sel);

// --- Sparkline + Audio Viz (shared canvas) ---
const SPARK_DAYS = 30;
let sparkData = new Array(SPARK_DAYS).fill(0); // seconds per day
let sparkDates = []; // date strings for tooltip
let currentAudioLevel = 0;
let morphProgress = 0; // 0=sparkline, 1=waveform
let morphTarget = 0;
let sparkAnimId = null;
let isRecording = false;

function initSparkline(canvas) {
  const rect = canvas.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  canvas.width = rect.width * dpr;
  canvas.height = rect.height * dpr;
  canvas.getContext("2d").scale(dpr, dpr);
  canvas._w = rect.width;
  canvas._h = rect.height;
}

function loadSparkData(usageData) {
  // Fill 30-day array, zero-fill missing days
  const today = new Date();
  sparkData = [];
  sparkDates = [];

  for (let i = SPARK_DAYS - 1; i >= 0; i--) {
    const d = new Date(today);
    d.setDate(d.getDate() - i);
    const dateStr = d.toISOString().slice(0, 10);
    sparkDates.push(dateStr);
    const entry = usageData.find(e => e.date === dateStr);
    sparkData.push(entry ? entry.seconds : 0);
  }
}

function drawSparkline() {
  const canvas = $("#sparkline");
  if (!canvas._w) initSparkline(canvas);
  const ctx = canvas.getContext("2d");
  const W = canvas._w;
  const H = canvas._h;

  ctx.clearRect(0, 0, W, H);

  // Morph animation
  if (morphProgress !== morphTarget) {
    morphProgress += (morphTarget - morphProgress) * 0.12;
    if (Math.abs(morphProgress - morphTarget) < 0.01) morphProgress = morphTarget;
  }

  const maxVal = Math.max(...sparkData, 60); // min 1 min
  const t = performance.now() / 80;
  const amp = Math.min(currentAudioLevel * 250, H / 2 - 2);

  // Compute blended Y for each pixel
  const points = [];
  for (let x = 0; x < W; x++) {
    // Sparkline Y
    const dayFrac = (x / (W - 1)) * (SPARK_DAYS - 1);
    const i = Math.floor(dayFrac);
    const f = dayFrac - i;
    const val = sparkData[i] * (1 - f) + (sparkData[i + 1] ?? sparkData[i]) * f;
    const sy = H - 2 - (val / maxVal) * (H - 6);

    // Waveform Y
    const wy = H / 2 + Math.sin(x * 0.15 + t) * amp
      * (0.6 + 0.4 * Math.sin(x * 0.07 + t * 0.3));

    const y = sy * (1 - morphProgress) + wy * morphProgress;
    points.push(y);
  }

  // Fill
  ctx.beginPath();
  ctx.moveTo(0, H);
  points.forEach((y, x) => ctx.lineTo(x, y));
  ctx.lineTo(W, H);
  ctx.closePath();
  ctx.fillStyle = "rgba(74, 222, 128, 0.08)";
  ctx.fill();

  // Stroke
  ctx.beginPath();
  points.forEach((y, x) => x === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y));
  ctx.strokeStyle = "#4ade80";
  ctx.lineWidth = 1.5;
  ctx.stroke();

  // Keep animating if recording or morphing
  if (isRecording || morphProgress !== morphTarget) {
    sparkAnimId = requestAnimationFrame(drawSparkline);
  } else {
    sparkAnimId = null;
  }
}

function startRecordingViz() {
  isRecording = true;
  morphTarget = 1;
  if (!sparkAnimId) sparkAnimId = requestAnimationFrame(drawSparkline);
}

function stopRecordingViz() {
  isRecording = false;
  morphTarget = 0;
  currentAudioLevel = 0;
  // Keep animating until morph completes
  if (!sparkAnimId) sparkAnimId = requestAnimationFrame(drawSparkline);
}

// Tooltip
function setupSparkTooltip(canvas) {
  let tooltip = document.createElement("div");
  tooltip.id = "sparkline-tooltip";
  tooltip.style.display = "none";
  document.body.appendChild(tooltip);

  canvas.addEventListener("mousemove", (e) => {
    if (isRecording) { tooltip.style.display = "none"; return; }
    const rect = canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const dayIdx = Math.round((x / rect.width) * (SPARK_DAYS - 1));
    if (dayIdx < 0 || dayIdx >= SPARK_DAYS) return;

    const secs = sparkData[dayIdx];
    const date = sparkDates[dayIdx];
    const mins = (secs / 60).toFixed(1);
    tooltip.textContent = `${date}: ${mins} min`;
    tooltip.style.display = "block";
    tooltip.style.left = (e.clientX + 8) + "px";
    tooltip.style.top = (e.clientY - 24) + "px";
  });

  canvas.addEventListener("mouseleave", () => {
    tooltip.style.display = "none";
  });
}

// --- Utilities ---

function formatTime(isoString) {
  const d = new Date(isoString);
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

function formatDate(isoString) {
  const d = new Date(isoString);
  const today = new Date();
  const yesterday = new Date(today);
  yesterday.setDate(yesterday.getDate() - 1);

  if (d.toDateString() === today.toDateString()) return "TODAY";
  if (d.toDateString() === yesterday.toDateString()) return "YESTERDAY";
  return d.toLocaleDateString([], { day: "numeric", month: "short", year: "numeric" }).toUpperCase();
}

function addLogEntry(text, ts) {
  const log = $("#log-entries");
  const time = ts ? formatTime(ts) : new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  const dateLabel = ts ? formatDate(ts) : "TODAY";

  const lastDateSep = log.querySelector(".date-sep:first-child");
  if (!lastDateSep || lastDateSep.textContent !== dateLabel) {
    const sep = document.createElement("div");
    sep.className = "date-sep";
    sep.textContent = dateLabel;
    log.insertBefore(sep, log.firstChild);
  }

  const entry = document.createElement("div");
  entry.className = "log-entry";
  entry.innerHTML = `<span class="log-time">${time}</span><span class="log-text">${escapeHtml(text)}</span>`;

  const firstSep = log.querySelector(".date-sep");
  if (firstSep && firstSep.nextSibling) {
    log.insertBefore(entry, firstSep.nextSibling);
  } else {
    log.appendChild(entry);
  }
}

function escapeHtml(text) {
  const div = document.createElement("div");
  div.textContent = text;
  return div.innerHTML;
}

// Show one panel at a time (log, settings, debug)
function showPanel(name) {
  $("#log-entries").style.display = name === "log" ? "" : "none";
  $("#settings-panel").style.display = name === "settings" ? "flex" : "none";
  $("#debug-panel").style.display = name === "debug" ? "flex" : "none";
}

// --- Main ---

window.addEventListener("DOMContentLoaded", async () => {
  const config = await invoke("get_config");
  if (!config.has_api_key) {
    $("#setup").style.display = "block";
    $("#status-detail").textContent = "api key required";
  }

  // Open Groq console link in default browser
  $("#groq-link")?.addEventListener("click", async (e) => {
    e.preventDefault();
    const { openUrl } = window.__TAURI__.opener;
    await openUrl("https://console.groq.com/keys");
  });

  if (config.has_api_key) {
    $("#status-detail").textContent = `${config.provider}: ${config.api_key_preview}`;
    setTimeout(() => {
      $("#status-detail").textContent = "ready";
    }, 2000);
  }

  // Load history
  try {
    const history = await invoke("get_log_history", { limit: 50 });
    for (const entry of history.reverse()) {
      const dur = entry.duration ? ` (${Number(entry.duration).toFixed(1)}s)` : "";
      addLogEntry(entry.text + dur, entry.ts);
    }
  } catch (e) {
    console.error("Failed to load history:", e);
  }

  // Init sparkline
  const sparkCanvas = $("#sparkline");
  initSparkline(sparkCanvas);
  setupSparkTooltip(sparkCanvas);

  try {
    const usage = await invoke("get_usage_stats");
    loadSparkData(usage);
  } catch (e) {
    console.error("Failed to load usage:", e);
  }
  drawSparkline();

  // Resize handler
  window.addEventListener("resize", () => {
    initSparkline(sparkCanvas);
    drawSparkline();
  });

  $("#key-form")?.addEventListener("submit", async (e) => {
    e.preventDefault();
    const key = $("#api-key-input").value.trim();
    if (key) {
      await invoke("set_api_key", { key });
      $("#setup").style.display = "none";
      $("#status-detail").textContent = "key saved. ready";
    }
  });

  await listen("recording-status", (event) => {
    const icon = $("#status-icon");
    if (event.payload) {
      icon.className = "recording";
      $("#status-detail").textContent = "";
      playStartQuack();
      startRecordingViz();
    } else {
      icon.className = "idle";
      playStopQuack();
      stopRecordingViz();
    }
  });

  await listen("audio-level", (event) => {
    currentAudioLevel = event.payload;
  });

  await listen("status-detail", (event) => {
    $("#status-detail").textContent = event.payload.toLowerCase();
  });

  let ribbitInterval = null;
  await listen("transcribing", (event) => {
    if (event.payload) {
      $("#status-icon").className = "transcribing";
      let dots = 0;
      ribbitInterval = setInterval(() => {
        dots = (dots + 1) % 4;
        const base = $("#status-detail").textContent.split("...")[0].split("..")[0].split(".")[0];
        if (base.includes("ribbit")) {
          $("#status-detail").textContent = "ribbiting" + ".".repeat(dots + 1);
        }
      }, 400);
    } else {
      if (ribbitInterval) { clearInterval(ribbitInterval); ribbitInterval = null; }
      $("#status-icon").className = "idle";
    }
  });

  await listen("transcription", (event) => {
    const { text, duration } = event.payload;
    const dur = duration ? ` (${Number(duration).toFixed(1)}s)` : "";
    addLogEntry(text + dur);
    playQuack(1.3, 0.4);

    // Update today's sparkline data point
    sparkData[SPARK_DAYS - 1] += duration || 0;
    if (!sparkAnimId) drawSparkline();
  });

  await listen("error", (event) => {
    $("#status-detail").textContent = event.payload.toLowerCase();
    $("#status-detail").classList.add("error");
    setTimeout(() => {
      $("#status-detail").classList.remove("error");
      $("#status-detail").textContent = "ready";
    }, 8000);
  });

  // Window controls
  $("#win-min").addEventListener("click", () => getCurrentWindow().minimize());
  $("#win-close").addEventListener("click", () => getCurrentWindow().hide());

  // Settings panel (gear toggles it)
  $("#settings-btn").addEventListener("click", () => {
    const visible = $("#settings-panel").style.display !== "none";
    showPanel(visible ? "log" : "settings");
  });

  // Always on top toggle
  $("#always-on-top").addEventListener("change", async (e) => {
    await invoke("set_always_on_top", { value: e.target.checked });
  });

  // Debug log (from settings)
  $("#debug-btn").addEventListener("click", async () => {
    const log = await invoke("get_debug_log");
    $("#debug-content").textContent = log;
    showPanel("debug");
    $("#debug-content").scrollTop = $("#debug-content").scrollHeight;
  });
  $("#debug-close").addEventListener("click", () => showPanel("log"));
});
