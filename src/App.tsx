import { type FormEvent, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import "./App.css";
import {
  type Group,
  contactById,
  contacts,
  nextId,
  parseMentions,
  seedGroups,
} from "./chat";

type AgentRun = {
  provider: string;
  ok: boolean;
  text: string;
  native_session_id?: string | null;
  dispatch_task?: string | null;
};
type ProviderStatus = { id: string; installed: boolean };
type ContactProfile = {
  contactId: string;
  model: string;
  apiKey: string;
  envText: string;
  persona: string;
};

export default function App() {
  const [groups, setGroups] = useState<Group[]>(seedGroups);
  const [people, setPeople] = useState(contacts);
  const [bridgeNote, setBridgeNote] = useState("");
  const [activeId, setActiveId] = useState("group-a");
  const [persisted, setPersisted] = useState(false);
  const [draft, setDraft] = useState("");
  const [creating, setCreating] = useState(false);
  const [editingContactId, setEditingContactId] = useState<string | null>(null);
  const [profiles, setProfiles] = useState<Record<string, ContactProfile>>({});
  const [mentionQuery, setMentionQuery] = useState<string | null>(null);
  const [mentionIndex, setMentionIndex] = useState(0);
  const [typingByGroup, setTypingByGroup] = useState<Record<string, { key: string; label: string }[]>>({});
  const active = groups.find((group) => group.id === activeId) ?? groups[0];

  function addTyping(groupId: string, key: string, label: string) {
    setTypingByGroup((current) => {
      const list = current[groupId] ?? [];
      return {
        ...current,
        [groupId]: [...list.filter((item) => item.key !== key), { key, label }],
      };
    });
  }

  function removeTyping(groupId: string, key: string) {
    setTypingByGroup((current) => {
      const list = current[groupId] ?? [];
      return {
        ...current,
        [groupId]: list.filter((item) => item.key !== key),
      };
    });
  }

  const mentionCandidates = useMemo(() => {
    if (mentionQuery === null || !active) return [];
    const q = mentionQuery.toLowerCase();
    const inGroup = active.memberIds.map((id) => ({
      contact: people.find((p) => p.id === id) ?? contactById(id),
      inGroup: true,
    }));
    const others = people
      .filter((p) => p.installed && !active.memberIds.includes(p.id))
      .map((contact) => ({ contact, inGroup: false }));
    return [...inGroup, ...others].filter(
      ({ contact }) =>
        !q || contact.name.toLowerCase().includes(q) || contact.id.toLowerCase().includes(q),
    );
  }, [mentionQuery, active, people]);

  function applyMention(contactId: string) {
    const name = contactById(contactId).name;
    setDraft((value) => {
      if (/@([^\s@]*)$/.test(value)) {
        return value.replace(/@([^\s@]*)$/, `@${name} `);
      }
      return `${value}${value.endsWith(" ") || !value ? "" : " "}@${name} `;
    });
    if (active && !active.memberIds.includes(contactId)) {
      setGroups((current) =>
        current.map((group) => {
          if (group.id !== active.id) return group;
          const next = { ...group, memberIds: [...group.memberIds, contactId] };
          persist(next);
          return next;
        }),
      );
    }
    setMentionQuery(null);
    setMentionIndex(0);
  }

  useEffect(() => {
    const refreshProviders = () => {
      invoke<ProviderStatus[]>("provider_status")
        .then((list) => {
          setPeople(
            contacts.map((contact) => ({
              ...contact,
              installed: list.find((item) => item.id === contact.id)?.installed ?? false,
            })),
          );
        })
        .catch(() => {
          setBridgeNote("当前窗口没有连上桌面后端。在项目根目录运行 pnpm tauri dev 后，发送才会调用本机 CLI。");
        });
    };
    refreshProviders();
    const timer = window.setInterval(refreshProviders, 3000);
    invoke<Group[]>("load_groups")
      .then((list) => {
        setPersisted(true);
        setGroups(list);
        setActiveId(list[0]?.id ?? "");
      })
      .catch(() => {
        setPersisted(false);
      });
    invoke<ContactProfile[]>("load_contact_profiles")
      .then((list) => {
        const map: Record<string, ContactProfile> = {};
        for (const item of list) {
          map[item.contactId] = item;
        }
        setProfiles(map);
      })
      .catch(() => {});
    return () => window.clearInterval(timer);
  }, []);

  function persist(group: Group) {
    if (!persisted) return;
    invoke("save_group", { group }).catch(() => {});
  }

  const title = useMemo(() => active?.name ?? "SessionNexus", [active]);

  function send() {
    const text = draft.trim();
    if (!text || !active) return;
    const allKnownIds = people.map((p) => p.id);
    const mentioned = parseMentions(text, allKnownIds);
    const targets = mentioned.length > 0 ? mentioned : active.memberIds;
    const groupId = active.id;
    const cwd = active.cwd;
    const groupName = active.name;
    const brief = active.brief;
    const rooms = active.roomSessions;
    const userMessage = { id: nextId("m"), kind: "user" as const, text };
    const pending = targets.map((contactId) => ({
      contactId,
      turnKey: nextId("turn"),
      nativeSessionId: rooms.find((room) => room.contactId === contactId)?.nativeSessionId,
    }));
    setGroups((current) =>
      current.map((group) => {
        if (group.id !== groupId) return group;
        const memberIds = [...new Set([...group.memberIds, ...targets])];
        const next = {
          ...group,
          memberIds,
          messages: [...group.messages, userMessage],
        };
        persist(next);
        return next;
      }),
    );
    setDraft("");
    setMentionQuery(null);
    for (const item of pending) {
      const contactName = contactById(item.contactId).name;
      const roomTypingKey = `room:${item.turnKey}`;
      addTyping(groupId, roomTypingKey, `${contactName} 正在输入…`);
      const roomPrompt = item.nativeSessionId
        ? text
        : [
            `你是本群里 ${contactName} 的主对话（房间会话）。`,
            `项目：${groupName}`,
            `工作目录：${cwd || "未指定"}`,
            `群说明：${brief}`,
            `用户消息：${text}`,
          ].join("\n");
      invoke<AgentRun>("run_agent", {
        provider: item.contactId,
        prompt: roomPrompt,
        cwd: cwd || null,
        nativeSessionId: item.nativeSessionId ?? null,
        role: "room",
      })
        .then((result) => {
          removeTyping(groupId, roomTypingKey);
          const task = result.dispatch_task?.trim();
          const workId = task ? nextId("work") : "";
          setGroups((current) =>
            current.map((group) => {
              if (group.id !== groupId) return group;
              const roomSessions = result.native_session_id
                ? [
                    ...group.roomSessions.filter((room) => room.contactId !== item.contactId),
                    { contactId: item.contactId, nativeSessionId: result.native_session_id },
                  ]
                : group.roomSessions;
              const next = {
                ...group,
                roomSessions,
                works: task
                  ? [
                      ...group.works,
                      {
                        id: workId,
                        contactId: item.contactId,
                        task,
                        status: "Working" as const,
                      },
                    ]
                  : group.works,
                messages: [
                  ...group.messages,
                  {
                    id: nextId("m"),
                    kind: "room" as const,
                    contactId: item.contactId,
                    text: result.text,
                  },
                ],
              };
              persist(next);
              return next;
            }),
          );
          if (!task) return;
          const workTypingKey = `work:${workId}`;
          addTyping(groupId, workTypingKey, `${contactName} 正在执行工作任务…`);
          const workPrompt = [
            `项目：${groupName}`,
            `工作目录：${cwd || "未指定"}`,
            `说明：${brief}`,
            `任务：${task}`,
            "只用文字回复结果，不要再派生子代理。",
          ].join("\n");
          invoke<AgentRun>("run_agent", {
            provider: item.contactId,
            prompt: workPrompt,
            cwd: cwd || null,
            nativeSessionId: null,
            role: "work",
          })
            .then((workResult) => {
              removeTyping(groupId, workTypingKey);
              setGroups((current) =>
                current.map((group) => {
                  if (group.id !== groupId) return group;
                  const next = {
                    ...group,
                    works: group.works.map((work) =>
                      work.id === workId
                        ? { ...work, status: workResult.ok ? ("Finished" as const) : ("Error" as const) }
                        : work,
                    ),
                    messages: [
                      ...group.messages,
                      {
                        id: nextId("m"),
                        kind: "work" as const,
                        contactId: item.contactId,
                        workId,
                        text: workResult.text,
                      },
                    ],
                  };
                  persist(next);
                  return next;
                }),
              );
            })
            .catch((error: unknown) => {
              removeTyping(groupId, workTypingKey);
              const detail = error instanceof Error ? error.message : String(error);
              setGroups((current) =>
                current.map((group) => {
                  if (group.id !== groupId) return group;
                  const next = {
                    ...group,
                    works: group.works.map((work) =>
                      work.id === workId ? { ...work, status: "Error" as const } : work,
                    ),
                    messages: [
                      ...group.messages,
                      {
                        id: nextId("m"),
                        kind: "work" as const,
                        contactId: item.contactId,
                        workId,
                        text: detail,
                      },
                    ],
                  };
                  persist(next);
                  return next;
                }),
              );
            });
        })
        .catch((error: unknown) => {
          removeTyping(groupId, roomTypingKey);
          const detail = error instanceof Error ? error.message : String(error);
          setGroups((current) =>
            current.map((group) => {
              if (group.id !== groupId) return group;
              const next = {
                ...group,
                messages: [
                  ...group.messages,
                  {
                    id: nextId("m"),
                    kind: "room" as const,
                    contactId: item.contactId,
                    text: detail,
                  },
                ],
              };
              persist(next);
              return next;
            }),
          );
        });
    }
  }

  function openDirect(contactId: string) {
    const existing = groups.find(
      (group) => group.memberIds.length === 1 && group.memberIds[0] === contactId,
    );
    if (existing) {
      setActiveId(existing.id);
      return;
    }
    const contact = contactById(contactId);
    const id = nextId("dm");
    const group: Group = {
      id,
      name: contact.name,
      cwd: "",
      brief: `与 ${contact.name} 的单聊。`,
      memberIds: [contactId],
      works: [],
      roomSessions: [],
      messages: [],
    };
    setBridgeNote(`已为 ${contact.name} 打开房间会话。`);
    setGroups((current) => [...current, group]);
    setActiveId(id);
    persist(group);
  }

  return (
    <div className="app">
      <aside className="rail">
        <header className="brand">
          <strong>SessionNexus</strong>
          <button type="button" onClick={() => setCreating(true)}>
            新建群
          </button>
        </header>
        <section>
          <h2>联系人</h2>
          <ul>
            {people.map((contact) => (
              <li key={contact.id} className="contact-item">
                <button type="button" className="row" onClick={() => openDirect(contact.id)} disabled={!contact.installed}>
                  <Avatar id={contact.id} />
                  <span>
                    <b>{contact.name}</b>
                    <small>
                      {profiles[contact.id]?.model
                        ? profiles[contact.id].model
                        : contact.installed
                          ? "可加入群"
                          : "未安装"}
                    </small>
                  </span>
                </button>
                <button
                  type="button"
                  className="icon-btn"
                  title={`${contact.name} 资料卡设置`}
                  onClick={() => setEditingContactId(contact.id)}
                >
                  设置
                </button>
              </li>
            ))}
          </ul>
        </section>
        <section>
          <h2>群聊</h2>
          <ul>
            {groups.map((group) => (
              <li key={group.id}>
                <button
                  type="button"
                  className={group.id === active?.id ? "row active" : "row"}
                  onClick={() => setActiveId(group.id)}
                >
                  <span className="group-mark">{group.memberIds.length === 1 ? "单" : "群"}</span>
                  <span>
                    <b>{group.name}</b>
                    <small>{group.memberIds.map((id) => contactById(id).name).join("、")}</small>
                  </span>
                </button>
              </li>
            ))}
          </ul>
        </section>
      </aside>

      <main className="thread">
        <header className="thread-head">
          <div>
            <h1>{title}</h1>
            <p>{active?.cwd || "未设置工作目录"}</p>
            {bridgeNote ? <p>{bridgeNote}</p> : null}
          </div>
          {active && active.memberIds.length === 1 ? (
            <button type="button" onClick={() => setEditingContactId(active.memberIds[0])}>
              资料卡设置
            </button>
          ) : null}
        </header>
        <div className="messages">
          {!active ? <p className="brief">还没有群。点左上角「新建群」，拉进联系人后再发消息。</p> : null}
          {active?.brief ? <p className="brief">{active.brief}</p> : null}
          {active?.messages.map((message) => (
            <article key={message.id} className={`bubble ${message.kind}`}>
              {message.contactId ? (
                <Avatar id={message.contactId} />
              ) : message.kind === "user" ? (
                <span className="avatar me">我</span>
              ) : (
                <span className="avatar sys">系</span>
              )}
              <div>
                <header>
                  {message.kind === "user"
                    ? "我"
                    : message.kind === "system"
                      ? "SessionNexus"
                      : contactById(message.contactId ?? "").name}
                  {message.kind === "room" ? <em>房间会话</em> : null}
                  {message.kind === "work" ? <em>工作会话</em> : null}
                </header>
                <p>{message.text}</p>
              </div>
            </article>
          ))}
        </div>
        {active && (typingByGroup[active.id]?.length ?? 0) > 0 ? (
          <div className="typing-bar" role="status" aria-live="polite">
            <span className="typing-dot" />
            <span>{typingByGroup[active.id].map((item) => item.label).join(" · ")}</span>
          </div>
        ) : null}
        <form
          className="composer"
          onSubmit={(event) => {
            event.preventDefault();
            send();
          }}
        >
          {mentionCandidates.length > 0 ? (
            <div className="mention">
              {mentionCandidates.map(({ contact, inGroup }, idx) => (
                <button
                  key={contact.id}
                  type="button"
                  className={idx === mentionIndex ? "active" : ""}
                  onClick={() => applyMention(contact.id)}
                >
                  <Avatar id={contact.id} />
                  <span>{contact.name}</span>
                  <small>{inGroup ? "群成员" : "点名并加入"}</small>
                </button>
              ))}
            </div>
          ) : null}
          <button
            type="button"
            className="at-btn"
            title="点名联系人 (@)"
            onClick={() => {
              setDraft((value) => (value.endsWith("@") ? value : `${value}${value && !value.endsWith(" ") ? " " : ""}@`));
              setMentionQuery("");
              setMentionIndex(0);
            }}
          >
            @
          </button>
          <textarea
            value={draft}
            placeholder={active ? "发给本群。输入 @ 或点击左侧 @ 点名联系人" : ""}
            onChange={(event) => {
              const value = event.target.value;
              setDraft(value);
              const match = value.match(/@([^\s@]*)$/);
              if (match) {
                setMentionQuery(match[1]);
                setMentionIndex(0);
              } else {
                setMentionQuery(null);
              }
            }}
            onKeyDown={(event) => {
              if (mentionCandidates.length > 0) {
                if (event.key === "ArrowDown") {
                  event.preventDefault();
                  setMentionIndex((idx) => (idx + 1) % mentionCandidates.length);
                  return;
                }
                if (event.key === "ArrowUp") {
                  event.preventDefault();
                  setMentionIndex((idx) => (idx - 1 + mentionCandidates.length) % mentionCandidates.length);
                  return;
                }
                if (event.key === "Enter" || event.key === "Tab") {
                  event.preventDefault();
                  const picked = mentionCandidates[mentionIndex] ?? mentionCandidates[0];
                  if (picked) {
                    applyMention(picked.contact.id);
                  }
                  return;
                }
                if (event.key === "Escape") {
                  event.preventDefault();
                  setMentionQuery(null);
                  return;
                }
              }
              if (event.key === "Enter" && !event.shiftKey) {
                event.preventDefault();
                send();
              }
            }}
          />
          <button type="submit">发送</button>
        </form>
      </main>

      <aside className="side">
        <section>
          <h2>成员</h2>
          <ul className="plain">
            {active?.memberIds.map((id) => (
              <li key={id}>
                <Avatar id={id} />
                <span>
                  <b>{contactById(id).name}</b>
                  <small>
                    {active.roomSessions.find((room) => room.contactId === id)
                      ? "房间会话可继续"
                      : "发送后建立房间会话"}
                  </small>
                </span>
              </li>
            ))}
          </ul>
        </section>
        <section>
          <h2>工作会话</h2>
          <ul className="plain">
            {active?.works.length ? (
              active.works.map((work) => (
                <li key={work.id}>
                  <Avatar id={work.contactId} />
                  <span>
                    <b>{work.task}</b>
                    <small>
                      {contactById(work.contactId).name} · {work.status}
                    </small>
                  </span>
                </li>
              ))
            ) : (
              <li className="empty">这个群还没有开工</li>
            )}
          </ul>
        </section>
      </aside>

      {creating ? (
        <CreateGroup
          people={people}
          onCancel={() => setCreating(false)}
          onCreate={(group) => {
            setGroups((current) => [...current, group]);
            setActiveId(group.id);
            persist(group);
            setCreating(false);
          }}
        />
      ) : null}
      {editingContactId ? (
        <ContactProfileModal
          contactId={editingContactId}
          initial={
            profiles[editingContactId] ?? {
              contactId: editingContactId,
              model: "",
              apiKey: "",
              envText: "",
              persona: "",
            }
          }
          onCancel={() => setEditingContactId(null)}
          onSave={(profile) => {
            setProfiles((current) => ({ ...current, [profile.contactId]: profile }));
            setEditingContactId(null);
            invoke("save_contact_profile", { profile })
              .then(() => setBridgeNote(`已保存 ${contactById(profile.contactId).name} 的资料卡设置。`))
              .catch((error: unknown) => {
                const detail = error instanceof Error ? error.message : String(error);
                setBridgeNote(`保存资料卡失败：${detail}`);
              });
          }}
        />
      ) : null}
    </div>
  );
}

const API_KEY_HINTS: Record<string, string> = {
  dsh: "DEEPSEEK_API_KEY",
  codex: "OPENAI_API_KEY",
  grok: "XAI_API_KEY",
};

function ContactProfileModal({
  contactId,
  initial,
  onCancel,
  onSave,
}: {
  contactId: string;
  initial: ContactProfile;
  onCancel: () => void;
  onSave: (profile: ContactProfile) => void;
}) {
  const [model, setModel] = useState(initial.model);
  const [apiKey, setApiKey] = useState(initial.apiKey);
  const [envText, setEnvText] = useState(initial.envText);
  const [persona, setPersona] = useState(initial.persona);
  const contact = contactById(contactId);
  const keyHint = API_KEY_HINTS[contactId];

  function submit(event: FormEvent) {
    event.preventDefault();
    onSave({
      contactId,
      model: model.trim(),
      apiKey: apiKey.trim(),
      envText,
      persona,
    });
  }

  return (
    <div className="modal">
      <form className="sheet" onSubmit={submit}>
        <h3>{contact.name} · 资料卡设置</h3>
        <label>
          模型名（Model）
          <input
            value={model}
            onChange={(event) => setModel(event.target.value)}
            placeholder="留空则使用该 Agent 默认模型"
          />
        </label>
        {keyHint ? (
          <label>
            API Key（注入为 {keyHint}）
            <input
              type="password"
              value={apiKey}
              onChange={(event) => setApiKey(event.target.value)}
              placeholder={`输入 ${keyHint}，留空沿用系统环境`}
            />
          </label>
        ) : null}
        <label>
          附加环境变量（每行 KEY=VALUE）
          <textarea
            rows={2}
            value={envText}
            onChange={(event) => setEnvText(event.target.value)}
            placeholder="例如 DEEPSEEK_API_KEY=sk-..."
          />
        </label>
        <label>
          人设 / 初始指令（新建会话首轮自动附带）
          <textarea
            rows={3}
            value={persona}
            onChange={(event) => setPersona(event.target.value)}
            placeholder="例如：你是一个严谨的代码审查助手，回答保持简洁。"
          />
        </label>
        <div className="actions">
          <button type="button" onClick={onCancel}>
            取消
          </button>
          <button type="submit">保存</button>
        </div>
      </form>
    </div>
  );
}

function CreateGroup({
  people,
  onCancel,
  onCreate,
}: {
  people: typeof contacts;
  onCancel: () => void;
  onCreate: (group: Group) => void;
}) {
  const [name, setName] = useState("项目 B");
  const [cwd, setCwd] = useState("D:\\work\\project-b");
  const [brief, setBrief] = useState("");
  const [picked, setPicked] = useState<string[]>(["grok", "qoder"]);

  return (
    <div className="modal">
      <form
        onSubmit={(event) => {
          event.preventDefault();
          const memberIds = picked.filter((id) => people.find((contact) => contact.id === id)?.installed);
          if (!name.trim() || memberIds.length === 0) return;
          onCreate({
            id: nextId("group"),
            name: name.trim(),
            cwd: cwd.trim(),
            brief: brief.trim() || "未填写项目说明。",
            memberIds,
            works: [],
            roomSessions: [],
            messages: [
              {
                id: nextId("m"),
                kind: "system",
                text: `已为 ${memberIds.map((id) => contactById(id).name).join("、")} 各开一个房间会话，并写入项目说明。`,
              },
            ],
          });
        }}
      >
        <h2>新建群</h2>
        <label>
          名称
          <input value={name} onChange={(event) => setName(event.target.value)} />
        </label>
        <label>
          工作目录
          <input value={cwd} onChange={(event) => setCwd(event.target.value)} />
        </label>
        <label>
          项目说明
          <textarea value={brief} onChange={(event) => setBrief(event.target.value)} />
        </label>
        <fieldset>
          <legend>拉入联系人</legend>
          {people.map((contact) => (
            <label key={contact.id}>
              <input
                type="checkbox"
                checked={picked.includes(contact.id)}
                disabled={!contact.installed}
                onChange={(event) => {
                  setPicked((current) =>
                    event.target.checked
                      ? [...current, contact.id]
                      : current.filter((id) => id !== contact.id),
                  );
                }}
              />
              {contact.name}
            </label>
          ))}
        </fieldset>
        <div className="actions">
          <button type="button" onClick={onCancel}>
            取消
          </button>
          <button type="submit">创建</button>
        </div>
      </form>
    </div>
  );
}

function Avatar({ id }: { id: string }) {
  return <span className={`avatar a-${id}`}>{contactById(id).name.slice(0, 1)}</span>;
}
