const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const { getCurrentWindow } = window.__TAURI__.window;

// Sounds via Web Audio API
const audioCtx = new (window.AudioContext || window.webkitAudioContext)();

function playQuackAt(time, pitch) {
  const osc = audioCtx.createOscillator();
  const gain = audioCtx.createGain();
  const filter = audioCtx.createBiquadFilter();

  filter.type = "bandpass";
  filter.frequency.value = 800 * pitch;
  filter.Q.value = 3;

  osc.connect(filter);
  filter.connect(gain);
  gain.connect(audioCtx.destination);

  osc.type = "sawtooth";
  osc.frequency.setValueAtTime(680 * pitch, time);
  osc.frequency.exponentialRampToValueAtTime(280 * pitch, time + 0.08);

  gain.gain.setValueAtTime(0.13, time);
  gain.gain.linearRampToValueAtTime(0.13, time + 0.02);
  gain.gain.exponentialRampToValueAtTime(0.001, time + 0.12);

  osc.start(time);
  osc.stop(time + 0.12);
}

function playStartQuack() {
  const now = audioCtx.currentTime;
  playQuackAt(now, 1.1);
  playQuackAt(now + 0.14, 1.0);
}

function playStopQuack() {
  playQuackAt(audioCtx.currentTime, 0.85);
}

function playChirp() {
  const now = audioCtx.currentTime;
  const osc = audioCtx.createOscillator();
  const gain = audioCtx.createGain();
  osc.connect(gain);
  gain.connect(audioCtx.destination);
  osc.type = "sine";
  osc.frequency.setValueAtTime(600, now);
  osc.frequency.exponentialRampToValueAtTime(900, now + 0.06);
  gain.gain.setValueAtTime(0.08, now);
  gain.gain.exponentialRampToValueAtTime(0.001, now + 0.08);
  osc.start(now);
  osc.stop(now + 0.08);
}

const $ = (sel) => document.querySelector(sel);

// Audio level visualization
let currentAudioLevel = 0;
let vizAnimId = null;

function startAudioViz() {
  const canvas = $("#audio-viz");
  canvas.style.display = "block";
  const ctx = canvas.getContext("2d");
  const W = canvas.width;
  const H = canvas.height;

  function draw() {
    ctx.clearRect(0, 0, W, H);
    ctx.beginPath();
    ctx.strokeStyle = "#4ade80";
    ctx.lineWidth = 1.5;

    // Amplitude based on real audio level, clamped
    const amp = Math.min(currentAudioLevel * 250, H / 2 - 1);
    const t = performance.now() / 80;

    for (let x = 0; x < W; x++) {
      const y = H / 2 + Math.sin(x * 0.2 + t) * amp * (0.6 + 0.4 * Math.sin(x * 0.07 + t * 0.3));
      if (x === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.stroke();
    vizAnimId = requestAnimationFrame(draw);
  }
  draw();
}

function stopAudioViz() {
  if (vizAnimId) {
    cancelAnimationFrame(vizAnimId);
    vizAnimId = null;
  }
  $("#audio-viz").style.display = "none";
  currentAudioLevel = 0;
}

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

window.addEventListener("DOMContentLoaded", async () => {
  const config = await invoke("get_config");
  if (!config.has_api_key) {
    $("#setup").style.display = "block";
    $("#status-detail").textContent = "api key required";
  } else {
    $("#status-detail").textContent = `${config.provider}: ${config.api_key_preview}`;
    setTimeout(() => {
      $("#status-detail").textContent = "ready";
    }, 2000);
  }

  try {
    const history = await invoke("get_log_history", { limit: 50 });
    for (const entry of history.reverse()) {
      addLogEntry(entry.text, entry.ts);
    }
  } catch (e) {
    console.error("Failed to load history:", e);
  }

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
      playStartQuack();
      startAudioViz();
    } else {
      icon.className = "idle";
      playStopQuack();
      stopAudioViz();
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
    addLogEntry(event.payload);
    playChirp();
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
