const SOCKET_URL = "ws://127.0.0.1:19877/extension";

let socket = null;

function connect() {
  if (socket && (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING)) {
    return;
  }
  socket = new WebSocket(SOCKET_URL);
  socket.onmessage = (event) => {
    let message;
    try {
      message = JSON.parse(event.data);
    } catch {
      return;
    }
    handle(message).then(
      (result) => reply(message.id, { result }),
      (error) => reply(message.id, { error: String(error && error.message ? error.message : error) }),
    );
  };
  socket.onclose = () => {
    socket = null;
    setTimeout(connect, 2000);
  };
}

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
      return findTab(params.url || "");
    case "snapshot":
      return run(params.tabId, snapshotPage);
    case "fill":
      return run(params.tabId, fillPage, [params.selector || "", params.value || ""]);
    case "click":
      return run(params.tabId, clickPage, [params.selector || ""]);
    default:
      throw new Error("unknown tool " + message.method);
  }
}

function listTabs() {
  return chrome.tabs.query({}).then((tabs) =>
    tabs.map((tab) => ({
      tabId: tab.id,
      url: tab.url || "",
      title: tab.title || "",
      active: Boolean(tab.active),
    })),
  );
}

async function findTab(url) {
  const needle = String(url);
  const tabs = await listTabs();
  const found = tabs.find((tab) => tab.url.includes(needle));
  if (!found) {
    throw new Error("no open tab found");
  }
  return found;
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
      if (!first || first.result == null) {
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
  el.click();
  return { ok: true, tag: el.tagName.toLowerCase() };
}

connect();
