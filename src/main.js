import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { save, open } from "@tauri-apps/plugin-dialog";
import "./styles.css";

// --- State ---
let profiles = [];
let activeProfileId = null;
let trafficStats = null;
let currentView = "list"; // "list" | "form" | "settings"
let editingProfile = null;
let ppsInHistory = new Array(60).fill(0); // 30s of history at 500ms interval
let ppsOutHistory = new Array(60).fill(0);

// --- DOM ---
const mainContent = document.getElementById("main-content");

// --- Theme ---
function loadTheme() {
  const saved = localStorage.getItem("bedrock-bridge-theme");
  if (saved) {
    document.documentElement.setAttribute("data-theme", saved);
  }
}
loadTheme();

function toggleTheme() {
  const current = document.documentElement.getAttribute("data-theme");
  const next = current === "light" ? "dark" : "light";
  document.documentElement.setAttribute("data-theme", next === "dark" ? "" : next);
  if (next === "dark") {
    document.documentElement.removeAttribute("data-theme");
    localStorage.setItem("bedrock-bridge-theme", "dark");
  } else {
    localStorage.setItem("bedrock-bridge-theme", next);
  }
}

function isLightTheme() {
  return document.documentElement.getAttribute("data-theme") === "light";
}

// --- Header ---
function updateHeader() {
  const btn = document.getElementById("header-settings");
  if (btn) {
    btn.textContent = currentView === "settings" ? "← Back" : "⚙️";
    btn.onclick = () => {
      currentView = currentView === "settings" ? "list" : "settings";
      render();
    };
  }
}

// --- API helpers ---
async function loadProfiles() {
  try {
    profiles = await invoke("list_profiles");
  } catch (e) {
    console.error("list_profiles failed:", e);
    profiles = [];
  }
  render();
}

async function addProfile(label, host, port) {
  try {
    const p = await invoke("add_profile", { label, host, port });
    profiles.push(p);
  } catch (e) {
    alert("Failed to add profile: " + e);
  }
  render();
}

async function deleteProfile(id) {
  try {
    if (activeProfileId === id) {
      if (!confirm("This profile is currently active. Stop proxy and delete?")) return;
      await invoke("stop_proxy");
      await invoke("deactivate_profile");
      activeProfileId = null;
    }
    await invoke("delete_profile", { id });
    profiles = profiles.filter((p) => p.id !== id);
  } catch (e) {
    alert("Failed to delete profile: " + e);
  }
  render();
}

async function toggleProfile(id) {
  try {
    if (activeProfileId === id) {
      await invoke("stop_proxy");
      await invoke("deactivate_profile");
      activeProfileId = null;
    } else {
      const p = profiles.find((p) => p.id === id);
      if (!p) return;
      if (activeProfileId) {
        const active = profiles.find((p) => p.id === activeProfileId);
        const msg = active
          ? `"${active.label}" is currently active. Switch to "${p.label}"?`
          : "A proxy is currently running. Switch?";
        if (!confirm(msg)) return;
        await invoke("stop_proxy");
        await invoke("deactivate_profile");
      }
      await invoke("activate_profile", { id });
      await invoke("start_proxy", {
        host: p.host,
        port: p.port,
        label: p.label,
      });
      activeProfileId = id;
    }
  } catch (e) {
    alert("Proxy error: " + e);
    activeProfileId = null;
  }
  render();
}

// --- Traffic stats listener ---
listen("traffic-stats", (event) => {
  trafficStats = event.payload;
  ppsInHistory.push(trafficStats.pps_in);
  ppsOutHistory.push(trafficStats.pps_out);
  if (ppsInHistory.length > 60) ppsInHistory.shift();
  if (ppsOutHistory.length > 60) ppsOutHistory.shift();
  updateTrafficDisplay();
});

// --- Proxy status listener ---
let proxyStatus = null;
listen("proxy-status", (event) => {
  proxyStatus = event.payload;
  updateTitle();
  // If failed, allow retry by toggling off
  if (proxyStatus.state === "failed") {
    activeProfileId = null;
  }
  render();
});

function updateTrafficDisplay() {
  const el = document.getElementById("traffic-display");
  if (!el || !trafficStats) return;

  el.innerHTML = `
    <div class="traffic-stats">
      <div class="stat">
        <div class="stat-value">${trafficStats.pps_in}</div>
        <div class="stat-label">PPS In</div>
      </div>
      <div class="stat">
        <div class="stat-value">${trafficStats.pps_out}</div>
        <div class="stat-label">PPS Out</div>
      </div>
      <div class="stat">
        <div class="stat-value">${formatBytes(trafficStats.bytes_in)}</div>
        <div class="stat-label">Total In</div>
      </div>
      <div class="stat">
        <div class="stat-value">${formatBytes(trafficStats.bytes_out)}</div>
        <div class="stat-label">Total Out</div>
      </div>
    </div>
    ${trafficStats.clients && trafficStats.clients.length > 0 ? `
    <div class="client-list">
      <div class="client-header">Connected Clients (${trafficStats.active_sessions})</div>
      ${trafficStats.clients.map(c => `<div class="client-entry">● ${esc(c)}</div>`).join("")}
    </div>` : ""}
    <div class="sparkline-section">
      <div class="sparkline-label">Packets/sec (30s)</div>
      <canvas id="sparkline" width="420" height="80"></canvas>
    </div>
  `;

  drawSparkline();
}

function formatBytes(b) {
  if (b < 1024) return b + " B";
  if (b < 1048576) return (b / 1024).toFixed(1) + " KB";
  return (b / 1048576).toFixed(1) + " MB";
}

// --- Render ---
function render() {
  updateTitle();
  updateHeader();
  if (currentView === "settings") {
    renderSettings();
  } else if (currentView === "form") {
    renderForm();
  } else {
    renderList();
  }
}

function renderList() {
  const profileCards = profiles
    .map(
      (p) => `
    <div class="profile-card">
      <div class="profile-info">
        <div class="profile-label">${esc(p.label)}</div>
        <div class="profile-host">${esc(p.host)}:${p.port}</div>
      </div>
      <label class="toggle">
        <input type="checkbox" ${activeProfileId === p.id ? "checked" : ""}
          onchange="window.__toggle('${p.id}')" />
        <span class="slider"></span>
      </label>
      <button class="btn btn-ghost" onclick="window.__edit('${p.id}')">✏️</button>
      <button class="btn btn-ghost" onclick="window.__delete('${p.id}')">🗑️</button>
    </div>
  `
    )
    .join("");

  const statusHtml = proxyStatus && proxyStatus.state !== "running" && activeProfileId
    ? `<div class="status-banner status-${proxyStatus.state}">${esc(proxyStatus.message)}</div>`
    : "";

  const trafficHtml = activeProfileId
    ? `<div id="traffic-display"></div>`
    : "";

  mainContent.innerHTML = `
    ${statusHtml}
    ${profileCards}
    <button class="add-btn" onclick="window.__add()">+ Add Server Profile</button>
    ${trafficHtml}
  `;

  if (activeProfileId) updateTrafficDisplay();
}

function drawSparkline() {
  const canvas = document.getElementById("sparkline");
  if (!canvas) return;
  const ctx = canvas.getContext("2d");
  const w = canvas.width;
  const h = canvas.height;

  ctx.clearRect(0, 0, w, h);

  const maxVal = Math.max(
    1,
    ...ppsInHistory,
    ...ppsOutHistory
  );

  const drawLine = (data, color) => {
    ctx.beginPath();
    ctx.strokeStyle = color;
    ctx.lineWidth = 1.5;
    for (let i = 0; i < data.length; i++) {
      const x = (i / (data.length - 1)) * w;
      const y = h - (data[i] / maxVal) * (h - 4) - 2;
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.stroke();
  };

  drawLine(ppsInHistory, "#4ade80");
  drawLine(ppsOutHistory, "#60a5fa");

  // Legend
  ctx.font = "10px sans-serif";
  ctx.fillStyle = "#4ade80";
  ctx.fillText("● In", w - 70, 12);
  ctx.fillStyle = "#60a5fa";
  ctx.fillText("● Out", w - 30, 12);
}

function renderForm() {
  const p = editingProfile;
  mainContent.innerHTML = `
    <div class="form-group">
      <label>Label</label>
      <input id="f-label" type="text" placeholder="My Bedrock Server" value="${p ? esc(p.label) : ""}" />
    </div>
    <div class="form-group">
      <label>Remote Host</label>
      <input id="f-host" type="text" placeholder="192.168.1.100" value="${p ? esc(p.host) : ""}" />
    </div>
    <div class="form-group">
      <label>Remote Port</label>
      <input id="f-port" type="number" placeholder="19132" value="${p ? p.port : "19132"}" />
    </div>
    <div class="form-actions">
      <button class="btn btn-ghost" onclick="window.__cancel()">Cancel</button>
      <button class="btn btn-primary" onclick="window.__save()">Save</button>
    </div>
  `;

  // Keyboard shortcuts for form
  const handleKey = (e) => {
    if (e.key === "Escape") { window.__cancel(); }
    if (e.key === "Enter" && e.target.tagName !== "TEXTAREA") { window.__save(); }
  };
  mainContent.removeEventListener("keydown", window.__formKeyHandler);
  window.__formKeyHandler = handleKey;
  mainContent.addEventListener("keydown", handleKey);
  document.getElementById("f-label").focus();
}

function renderSettings() {
  const autostartChecked = ""; // loaded async below
  mainContent.innerHTML = `
    <div class="settings-section">
      <div class="settings-item">
        <div>
          <div class="settings-item-label">Light Theme</div>
          <div class="settings-item-desc">Switch to light color scheme</div>
        </div>
        <label class="toggle">
          <input type="checkbox" ${isLightTheme() ? "checked" : ""}
            onchange="window.__toggleTheme()" />
          <span class="slider"></span>
        </label>
      </div>
      <div class="settings-item">
        <div>
          <div class="settings-item-label">Start on Login</div>
          <div class="settings-item-desc">Launch Bedrock Bridge automatically</div>
        </div>
        <label class="toggle">
          <input type="checkbox" id="autostart-toggle" ${autostartChecked}
            onchange="window.__toggleAutostart(this.checked)" />
          <span class="slider"></span>
        </label>
      </div>
      <div class="settings-item">
        <div>
          <div class="settings-item-label">Export Profiles</div>
          <div class="settings-item-desc">Save profiles to a JSON file</div>
        </div>
        <button class="btn btn-ghost" onclick="window.__export()">📦</button>
      </div>
      <div class="settings-item">
        <div>
          <div class="settings-item-label">Import Profiles</div>
          <div class="settings-item-desc">Load profiles from a JSON file</div>
        </div>
        <button class="btn btn-ghost" onclick="window.__import()">📂</button>
      </div>
    </div>
  `;
  // Load autostart state async
  invoke("is_autostart_enabled").then((enabled) => {
    const el = document.getElementById("autostart-toggle");
    if (el) el.checked = enabled;
  }).catch(() => {});
}
window.__toggle = (id) => toggleProfile(id);
window.__add = () => {
  editingProfile = null;
  currentView = "form";
  render();
};
window.__edit = (id) => {
  editingProfile = profiles.find((p) => p.id === id);
  currentView = "form";
  render();
};
window.__delete = (id) => {
  if (confirm("Delete this profile?")) deleteProfile(id);
};
window.__cancel = () => {
  editingProfile = null;
  currentView = "list";
  render();
};
window.__toggleTheme = () => {
  toggleTheme();
  render();
};
window.__toggleAutostart = async (enabled) => {
  try {
    await invoke("set_autostart", { enable: enabled });
  } catch (e) {
    alert("Failed to set autostart: " + e);
  }
};
window.__save = async () => {
  const label = document.getElementById("f-label").value.trim();
  const host = document.getElementById("f-host").value.trim();
  const port = parseInt(document.getElementById("f-port").value, 10);

  if (!label || !host || isNaN(port)) {
    alert("Please fill in all fields.");
    return;
  }

  // Check for duplicate label (excluding current profile being edited)
  const duplicate = profiles.find(
    (p) => p.label.toLowerCase() === label.toLowerCase() && (!editingProfile || p.id !== editingProfile.id)
  );
  if (duplicate) {
    alert(`A profile named "${label}" already exists.`);
    return;
  }

  try {
    if (editingProfile) {
      await invoke("update_profile", { id: editingProfile.id, label, host, port });
      // If the active profile was edited, restart proxy with new config
      if (activeProfileId === editingProfile.id) {
        await invoke("stop_proxy");
        await invoke("start_proxy", { host, port, label });
      }
    } else {
      await invoke("add_profile", { label, host, port });
    }
  } catch (e) {
    alert("Save failed: " + e);
  }

  editingProfile = null;
  currentView = "list";
  await loadProfiles();
};

window.__export = async () => {
  try {
    const json = await invoke("export_profiles");
    const path = await save({
      defaultPath: "bedrock-bridge-profiles.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (path) {
      const { writeTextFile } = await import("@tauri-apps/plugin-fs");
      await writeTextFile(path, json);
    }
  } catch (e) {
    alert("Export failed: " + e);
  }
};

window.__import = async () => {
  try {
    const path = await open({
      filters: [{ name: "JSON", extensions: ["json"] }],
      multiple: false,
    });
    if (path) {
      const { readTextFile } = await import("@tauri-apps/plugin-fs");
      const json = await readTextFile(path);
      const count = await invoke("import_profiles", { json });
      alert(`Imported ${count} profile(s).`);
      await loadProfiles();
    }
  } catch (e) {
    alert("Import failed: " + e);
  }
};

function updateTitle() {
  const p = profiles.find((p) => p.id === activeProfileId);
  if (proxyStatus && proxyStatus.state === "retrying") {
    document.title = `⛏ Bedrock Bridge ⚡ Retrying...`;
  } else if (proxyStatus && proxyStatus.state === "failed") {
    document.title = `⛏ Bedrock Bridge ✗ Connection Failed`;
  } else if (p) {
    document.title = `⛏ Bedrock Bridge → ${p.label} (${p.host}:${p.port})`;
  } else {
    document.title = "⛏ Bedrock Bridge";
  }
}

function esc(s) {
  const d = document.createElement("div");
  d.textContent = s;
  return d.innerHTML;
}

// --- Init ---
mainContent.innerHTML = "<p>Loading profiles...</p>";
loadProfiles();
