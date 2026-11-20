export type Contact = {
  id: string;
  name: string;
  installed: boolean;
};

export type WorkSession = {
  id: string;
  contactId: string;
  task: string;
  status: "Working" | "Finished";
};

export type ChatMessage = {
  id: string;
  kind: "user" | "room" | "work" | "system";
  contactId?: string;
  text: string;
  workId?: string;
};

export type Group = {
  id: string;
  name: string;
  cwd: string;
  brief: string;
  memberIds: string[];
  messages: ChatMessage[];
  works: WorkSession[];
};

export const contacts: Contact[] = [
  { id: "grok", name: "Grok", installed: true },
  { id: "qoder", name: "Qoder", installed: true },
  { id: "dsh", name: "DSH", installed: true },
  { id: "codex", name: "Codex", installed: true },
];

export function contactById(id: string): Contact {
  return contacts.find((item) => item.id === id) ?? { id, name: id, installed: false };
}

let seq = 10;
export function nextId(prefix: string): string {
  seq += 1;
  return `${prefix}-${seq}`;
}

export function seedGroups(): Group[] {
  return [
    {
      id: "group-a",
      name: "项目 A",
      cwd: "D:\\work\\project-a",
      brief: "做一个本地会话枢纽的桌面端，先把群聊和按群隔离的会话跑通。",
      memberIds: ["grok", "dsh", "qoder"],
      works: [
        {
          id: "work-1",
          contactId: "grok",
          task: "初始化仓库结构",
          status: "Working",
        },
      ],
      messages: [
        {
          id: "m1",
          kind: "system",
          text: "已为 Grok、DSH、Qoder 各开一个房间会话，并写入项目说明。",
        },
        {
          id: "m2",
          kind: "user",
          text: "@Grok 你负责初始化项目",
        },
        {
          id: "m3",
          kind: "room",
          contactId: "grok",
          text: "收到。我不开自己的子代理，交给 SessionNexus 新开一个工作会话来初始化仓库。",
        },
        {
          id: "m4",
          kind: "work",
          contactId: "grok",
          workId: "work-1",
          text: "工作会话已开始：初始化仓库结构",
        },
      ],
    },
  ];
}

export function parseMentions(text: string, memberIds: string[]): string[] {
  const found = new Set<string>();
  for (const id of memberIds) {
    const name = contactById(id).name;
    if (text.toLowerCase().includes("@" + name.toLowerCase()) || text.toLowerCase().includes("@" + id)) {
      found.add(id);
    }
  }
  return [...found];
}
