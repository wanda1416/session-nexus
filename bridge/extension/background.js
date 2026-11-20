const SOCKET_URL = "ws://127.0.0.1:19877/extension";
const HEALTH_URL = "http://127.0.0.1:19877/health";

let socket = null;
let keepaliveTimer = null;
let reconnectTimer = null;
let nexusWindowId = null;

function setBadge(connected) {
  chrome.action.setBadgeText({ text: connected ? "ON" : "OFF" });
  chrome.action.setBadgeBackgroundColor({ color: connected ? "#16a34a" : "#64748b" });
}

function scheduleReconnect() {
  if (reconnectTimer) {
    return;
  }
  reconnectTimer = setTimeout(() => {
    reconnectTimer = null;
    connect();
  }, 3000);
}

async function connect() {
  if (socket && (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING)) {
    return;
  }
  setBadge(false);
  try {
    const res = await fetch(HEALTH_URL, { cache: "no-store" });
    if (!res.ok) {
      scheduleReconnect();
      return;
    }
  } catch {
    scheduleReconnect();
    return;
  }
  if (socket && (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING)) {
    return;
  }
  socket = new WebSocket(SOCKET_URL);
  socket.onopen = () => {
    setBadge(true);
    if (keepaliveTimer) {
      clearInterval(keepaliveTimer);
    }
    keepaliveTimer = setInterval(() => {
      if (socket && socket.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify({ type: "ping" }));
      }
    }, 20000);
  };
  socket.onerror = () => {};
  socket.onmessage = (event) => {
    let message;
    try {
      message = JSON.parse(event.data);
    } catch {
      return;
    }
    if (message.type === "pong" || message.id == null) {
      return;
    }
    handle(message).then(
      (result) => reply(message.id, { result }),
      (error) => reply(message.id, { error: String(error && error.message ? error.message : error) }),
    );
  };
  socket.onclose = () => {
    setBadge(false);
    if (keepaliveTimer) {
      clearInterval(keepaliveTimer);
      keepaliveTimer = null;
    }
    socket = null;
    scheduleReconnect();
  };
}

chrome.action.onClicked.addListener(() => {
  connect();
});

chrome.alarms.create("nexus-keepalive", { periodInMinutes: 0.4 });
chrome.alarms.onAlarm.addListener((alarm) => {
  if (alarm.name === "nexus-keepalive") {
    connect();
  }
});

function reply(id, body) {
  if (!socket || socket.readyState !== WebSocket.OPEN || id == null) {
    return;
  }
  socket.send(JSON.stringify({ id, ...body }));
}

async function handle(message) {
  const params = message.params || {};
  switch (message.method) {
    case "list_tabs":
      return listTabs();
    case "find_tab":
      return findTab(params.url || "", Boolean(params.nexusOnly));
    case "get_tab":
      return getTab(params.tabId);
    case "open_tab":
      return openTab(params.url || "", params.isolated !== false);
    case "snapshot":
      return run(params.tabId, snapshotPage);
    case "fill":
      return run(params.tabId, fillPage, [params.selector || "", params.value || ""]);
    case "click":
      return run(params.tabId, clickPage, [params.selector || ""]);
    case "press_key":
      return run(params.tabId, pressKeyPage, [
        params.selector || "",
        params.key || "Enter",
        Boolean(params.ctrlKey),
        Boolean(params.shiftKey),
      ]);
    case "read_text":
      return run(params.tabId, readTextPage, [params.selector || "", Boolean(params.lastOnly)]);
    case "exists":
      return run(params.tabId, existsPage, [params.selector || ""]);
    default:
      throw new Error("unknown tool " + message.method);
  }
}

async function findTab(url, nexusOnly) {
  const needle = String(url);
  const tabs = await listTabs();
  const matches = tabs.filter((tab) => {
    if (!tab.url.includes(needle)) return false;
    if (nexusOnly) {
      return nexusWindowId != null && tab.windowId === nexusWindowId;
    }
    return true;
  });
  const found = matches.find((tab) => tab.active) || matches[0];
  if (!found) {
    throw new Error("no open tab found");
  }
  return found;
}

function listTabs() {
  return chrome.tabs.query({}).then((tabs) =>
    tabs.map((tab) => ({
      tabId: tab.id,
      windowId: tab.windowId,
      url: tab.url || "",
      title: tab.title || "",
      active: Boolean(tab.active),
    })),
  );
}

async function getTab(tabId) {
  const id = Number(tabId);
  if (!Number.isInteger(id)) {
    throw new Error("tabId is required");
  }
  const tab = await chrome.tabs.get(id);
  return {
    tabId: tab.id,
    windowId: tab.windowId,
    url: tab.url || "",
    title: tab.title || "",
    active: Boolean(tab.active),
  };
}

async function ensureNexusWindow(initialUrl) {
  if (nexusWindowId != null) {
    try {
      const existing = await chrome.windows.get(nexusWindowId);
      if (existing && existing.id != null) {
        return { windowId: existing.id, createdTab: null };
      }
    } catch {
      nexusWindowId = null;
    }
  }
  const win = await chrome.windows.create({
    url: initialUrl,
    state: "minimized",
    type: "normal",
  });
  nexusWindowId = win.id;
  const tab = win.tabs && win.tabs[0];
  return { windowId: win.id, createdTab: tab || null };
}

async function openTab(url, isolated) {
  if (!url) {
    throw new Error("url is required");
  }
  if (isolated) {
    const { windowId, createdTab } = await ensureNexusWindow(url);
    const tab = createdTab || (await chrome.tabs.create({ windowId, url, active: false }));
    return {
      tabId: tab.id,
      windowId: tab.windowId,
      url: tab.url || url,
      title: tab.title || "",
      active: Boolean(tab.active),
    };
  }
  const tab = await chrome.tabs.create({ url, active: true });
  return {
    tabId: tab.id,
    windowId: tab.windowId,
    url: tab.url || url,
    title: tab.title || "",
    active: Boolean(tab.active),
  };
}

function run(tabId, func, args) {
  const id = Number(tabId);
  if (!Number.isInteger(id)) {
    return Promise.reject(new Error("tabId is required"));
  }
  return chrome.scripting
    .executeScript({ target: { tabId: id }, func, args: args || [] })
    .then((injected) => {
      const first = injected && injected[0];
      if (!first || first.result === undefined) {
        throw new Error("page returned nothing");
      }
      return first.result;
    });
}

function elementFor(selector) {
  if (selector.startsWith("@e")) {
    return document.querySelector('[data-nexus-ref="' + selector.slice(1) + '"]');
  }
  return document.querySelector(selector);
}

function snapshotPage() {
  const nodes = Array.from(
    document.querySelectorAll("a, button, input, textarea, select, [contenteditable='true']"),
  ).slice(0, 80);
  const items = nodes.map((el, index) => {
    const ref = "e" + index;
    el.setAttribute("data-nexus-ref", ref);
    const text = (el.innerText || el.value || el.getAttribute("aria-label") || "").trim().slice(0, 120);
    return { ref: "@" + ref, tag: el.tagName.toLowerCase(), text };
  });
  const body = document.body && document.body.innerText ? document.body.innerText.slice(0, 4000) : "";
  return { url: location.href, title: document.title, text: body, items };
}

function fillPage(selector, value) {
  const el = elementFor(selector);
  if (!el) {
    throw new Error("element not found");
  }
  el.focus();
  if (el.isContentEditable) {
    try {
      const selection = window.getSelection();
      const range = document.createRange();
      range.selectNodeContents(el);
      selection.removeAllRanges();
      selection.addRange(range);
      if (document.execCommand("insertText", false, value)) {
        return { ok: true, mode: "execCommand" };
      }
    } catch {
      // Fall through to direct textContent + InputEvent.
    }
    el.textContent = value;
    el.dispatchEvent(new InputEvent("input", { bubbles: true, data: value, inputType: "insertText" }));
    return { ok: true, mode: "contenteditable" };
  }
  const proto = el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
  const setter = Object.getOwnPropertyDescriptor(proto, "value")?.set;
  if (setter) {
    setter.call(el, value);
  } else {
    el.value = value;
  }
  el.dispatchEvent(new Event("input", { bubbles: true }));
  el.dispatchEvent(new Event("change", { bubbles: true }));
  return { ok: true, mode: "value" };
}

function clickPage(selector) {
  const el = elementFor(selector);
  if (!el) {
    throw new Error("element not found");
  }
  if (el.disabled || el.getAttribute("aria-disabled") === "true") {
    throw new Error("element is disabled");
  }
  el.click();
  return { ok: true, tag: el.tagName.toLowerCase() };
}

function pressKeyPage(selector, key, ctrlKey, shiftKey) {
  const el = selector ? elementFor(selector) : document.activeElement;
  if (!el) {
    throw new Error("element not found");
  }
  el.focus();
  const init = {
    key,
    code: key === "Enter" ? "Enter" : key,
    keyCode: key === "Enter" ? 13 : 0,
    which: key === "Enter" ? 13 : 0,
    ctrlKey,
    shiftKey,
    bubbles: true,
    cancelable: true,
  };
  el.dispatchEvent(new KeyboardEvent("keydown", init));
  el.dispatchEvent(new KeyboardEvent("keypress", init));
  el.dispatchEvent(new KeyboardEvent("keyup", init));
  return { ok: true, key };
}

function readTextPage(selector, lastOnly) {
  const nodes = Array.from(document.querySelectorAll(selector));
  if (nodes.length === 0) {
    return { count: 0, text: "" };
  }
  const text = lastOnly
    ? (nodes[nodes.length - 1].innerText || "").trim()
    : nodes.map((n) => (n.innerText || "").trim()).filter(Boolean).join("\n\n");
  return { count: nodes.length, text };
}

function existsPage(selector) {
  const nodes = document.querySelectorAll(selector);
  return { exists: nodes.length > 0, count: nodes.length };
}

connect();
