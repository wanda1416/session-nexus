import { useEffect, useMemo, useState } from "react";
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

type AgentRun = { provider: string; ok: boolean; text: string };
type ProviderStatus = { id: string; installed: boolean };

export default function App() {
  const [groups, setGroups] = useState<Group[]>(seedGroups);
  const [people, setPeople] = useState(contacts);
  const [bridgeNote, setBridgeNote] = useState("");
  const [activeId, setActiveId] = useState("group-a");
  const [draft, setDraft] = useState("");
  const [creating, setCreating] = useState(false);
  const [mentionOpen, setMentionOpen] = useState(false);
  const active = groups.find((group) => group.id === activeId) ?? groups[0];

  useEffect(() => {
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
  }, []);

  const title = useMemo(() => active?.name ?? "SessionNexus", [active]);

  function send() {
    const text = draft.trim();
    if (!text || !active) return;
    const mentioned = parseMentions(text, active.memberIds);
    const targets = mentioned.length > 0 ? mentioned : active.memberIds;
    const task = text.replace(/@\S+/g, "").trim() || "处理群里的新消息";
    const prompt = `项目：${active.name}\n工作目录：${active.cwd || "未指定"}\n说明：${active.brief}\n任务：${task}\n只用文字回复结果，不要再派生子代理。`;
    const groupId = active.id;
    const cwd = active.cwd;
    const userMessage = { id: nextId("m"), kind: "user" as const, text };
    const works = targets.map((contactId) => ({
      id: nextId("work"),
      contactId,
      task,
      status: "Working" as const,
    }));
    const replies = works.flatMap((work) => {
      const name = contactById(work.contactId).name;
      return [
        {
          id: nextId("m"),
          kind: "room" as const,
          contactId: work.contactId,
          text: `${name} 的任务已交给本机 CLI。`,
        },
        {
          id: nextId("m"),
          kind: "work" as const,
          contactId: work.contactId,
          workId: work.id,
          text: "正在调用本机 CLI…",
        },
      ];
    });
    setGroups((current) =>
      current.map((group) =>
        group.id !== groupId
          ? group
          : {
              ...group,
              messages: [...group.messages, userMessage, ...replies],
              works: [...group.works, ...works],
            },
      ),
    );
    setDraft("");
    setMentionOpen(false);
    for (const work of works) {
      invoke<AgentRun>("run_agent", { provider: work.contactId, prompt, cwd: cwd || null })
        .then((result) => {
          setGroups((current) =>
            current.map((group) =>
              group.id !== groupId
                ? group
                : {
                    ...group,
                    works: group.works.map((item) =>
                      item.id === work.id ? { ...item, status: result.ok ? "Finished" : "Error" } : item,
                    ),
                    messages: group.messages.map((message) =>
                      message.workId === work.id ? { ...message, text: result.text } : message,
                    ),
                  },
            ),
          );
        })
        .catch((error: unknown) => {
          const detail = error instanceof Error ? error.message : String(error);
          setGroups((current) =>
            current.map((group) =>
              group.id !== groupId
                ? group
                : {
                    ...group,
                    works: group.works.map((item) => (item.id === work.id ? { ...item, status: "Error" } : item)),
                    messages: group.messages.map((message) =>
                      message.workId === work.id ? { ...message, text: detail } : message,
                    ),
                  },
            ),
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
      messages: [
        {
          id: nextId("m"),
          kind: "system",
          text: `已为 ${contact.name} 打开房间会话。`,
        },
      ],
    };
    setGroups((current) => [...current, group]);
    setActiveId(id);
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
              <li key={contact.id}>
                <button type="button" className="row" onClick={() => openDirect(contact.id)} disabled={!contact.installed}>
                  <Avatar id={contact.id} />
                  <span>
                    <b>{contact.name}</b>
                    <small>{contact.installed ? "可加入群" : "未安装"}</small>
                  </span>
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
        </header>
        <div className="messages">
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
        <form
          className="composer"
          onSubmit={(event) => {
            event.preventDefault();
            send();
          }}
        >
          {mentionOpen && active ? (
            <div className="mention">
              {active.memberIds.map((id) => (
                <button
                  key={id}
                  type="button"
                  onClick={() => {
                    setDraft((value) => `${value}@${contactById(id).name} `);
                    setMentionOpen(false);
                  }}
                >
                  {contactById(id).name}
                </button>
              ))}
            </div>
          ) : null}
          <textarea
            value={draft}
            placeholder={active ? "发给本群。用 @ 点名一个联系人" : ""}
            onChange={(event) => {
              const value = event.target.value;
              setDraft(value);
              setMentionOpen(value.endsWith("@"));
            }}
            onKeyDown={(event) => {
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
                  <small>房间会话已打开</small>
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
            setCreating(false);
          }}
        />
      ) : null}
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
