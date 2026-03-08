const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// Frog sounds via Web Audio API
const audioCtx = new (window.AudioContext || window.webkitAudioContext)();

function playRibbit() {
  const now = audioCtx.currentTime;
  [0, 0.12].forEach((offset, i) => {
    const osc = audioCtx.createOscillator();
    const gain = audioCtx.createGain();
    const lfo = audioCtx.createOscillator();
    const lfoGain = audioCtx.createGain();
    lfo.frequency.value = 30;
    lfoGain.gain.value = 40;
    lfo.connect(lfoGain);
    lfoGain.connect(osc.frequency);
    osc.connect(gain);
    gain.connect(audioCtx.destination);
    osc.type = "triangle";
    const t = now + offset;
    osc.frequency.setValueAtTime(480 - i * 40, t);
    osc.frequency.exponentialRampToValueAtTime(200, t + 0.09);
    gain.gain.setValueAtTime(0.12, t);
    gain.gain.exponentialRampToValueAtTime(0.001, t + 0.11);
    osc.start(t);
    osc.stop(t + 0.11);
    lfo.start(t);
    lfo.stop(t + 0.11);
  });
}

function playCroak() {
  const now = audioCtx.currentTime;
  const osc = audioCtx.createOscillator();
  const gain = audioCtx.createGain();
  const lfo = audioCtx.createOscillator();
  const lfoGain = audioCtx.createGain();
  lfo.frequency.value = 25;
  lfoGain.gain.value = 30;
  lfo.connect(lfoGain);
  lfoGain.connect(osc.frequency);
  osc.connect(gain);
  gain.connect(audioCtx.destination);
  osc.type = "triangle";
  osc.frequency.setValueAtTime(320, now);
  osc.frequency.exponentialRampToValueAtTime(140, now + 0.15);
  gain.gain.setValueAtTime(0.12, now);
  gain.gain.exponentialRampToValueAtTime(0.001, now + 0.18);
  osc.start(now);
  osc.stop(now + 0.18);
  lfo.start(now);
  lfo.stop(now + 0.18);
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

window.addEventListener("DOMContentLoaded", async () => {
  const config = await invoke("get_config");
  if (!config.has_api_key) {
    $("#setup").style.display = "block";
    $("#status-detail").textContent = "api key required";
  } else {
    $("#status-detail").textContent = `key: ${config.api_key_preview}`;
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
      playRibbit();
    } else {
      icon.className = "idle";
      playCroak();
    }
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

  $("#debug-btn").addEventListener("click", async () => {
    const panel = $("#debug-panel");
    if (panel.style.display === "none") {
      const log = await invoke("get_debug_log");
      $("#debug-content").textContent = log;
      panel.style.display = "flex";
      $("#log-entries").style.display = "none";
      $("#debug-content").scrollTop = $("#debug-content").scrollHeight;
    } else {
      panel.style.display = "none";
      $("#log-entries").style.display = "";
    }
  });

  $("#debug-close").addEventListener("click", () => {
    $("#debug-panel").style.display = "none";
    $("#log-entries").style.display = "";
  });
});
