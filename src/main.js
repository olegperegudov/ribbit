const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const statusIcon = () => document.getElementById("status-icon");
const statusText = () => document.getElementById("status-text");
const lastTranscription = () => document.getElementById("last-transcription");
const setupDiv = () => document.getElementById("setup");

window.addEventListener("DOMContentLoaded", async () => {
  // Check if API key is configured
  const config = await invoke("get_config");
  if (!config.has_api_key) {
    setupDiv().style.display = "block";
  }

  // API key form
  document.getElementById("key-form")?.addEventListener("submit", async (e) => {
    e.preventDefault();
    const key = document.getElementById("api-key-input").value.trim();
    if (key) {
      await invoke("set_api_key", { key });
      setupDiv().style.display = "none";
      statusText().textContent = "Ready. Press Ctrl+Alt+Space to dictate.";
    }
  });

  // Listen for recording status changes
  await listen("recording-status", (event) => {
    const icon = statusIcon();
    if (event.payload) {
      icon.className = "recording";
      statusText().textContent = "Recording... Release keys to stop.";
    } else {
      icon.className = "idle";
      statusText().textContent = "Ready. Press Ctrl+Alt+Space to dictate.";
    }
  });

  // Listen for transcribing status
  await listen("transcribing", (event) => {
    if (event.payload) {
      statusIcon().className = "transcribing";
      statusText().textContent = "Transcribing...";
    }
  });

  // Listen for transcription results
  await listen("transcription", (event) => {
    const text = event.payload;
    const div = lastTranscription();
    const entry = document.createElement("div");
    entry.className = "entry";
    const time = new Date().toLocaleTimeString();
    entry.textContent = `[${time}] ${text}`;
    div.insertBefore(entry, div.firstChild);

    // Keep only last 20 entries
    while (div.children.length > 20) {
      div.removeChild(div.lastChild);
    }
  });

  // Listen for errors
  await listen("error", (event) => {
    statusText().textContent = event.payload;
    statusIcon().className = "idle";
    setTimeout(() => {
      statusText().textContent = "Ready. Press Ctrl+Alt+Space to dictate.";
    }, 5000);
  });
});
