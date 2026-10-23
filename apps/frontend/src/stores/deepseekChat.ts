import { defineStore } from "pinia";
import { computed, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { llmCancel, llmChat, llmModels, tavernRead, tavernWrite, type LlmDelta, type LlmMessage } from "@/api/tavern";

const FILE = "deepseek_desk";
const BASE = "https://api.deepseek.com";

export interface DsMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
  reasoning: string;
  image?: string;
}

export interface DsChat {
  id: string;
  title: string;
  updatedAt: number;
  messages: DsMessage[];
}

export interface DsMemory {
  id: string;
  chatId: string;
  user: string;
  answer: string;
  at: number;
}

const FALLBACK_MODELS = ["deepseek-chat", "deepseek-reasoner"];

interface SavedDesk {
  apiKey?: string;
  think?: boolean;
  model?: string;
  memoryOn?: boolean;
  memories?: DsMemory[];
  followKeyword?: string;
  activeId?: string;
  chats?: DsChat[];
}

function uid() {
  return crypto.randomUUID();
}

function blankChat(): DsChat {
  return { id: uid(), title: "新对话", updatedAt: Date.now(), messages: [] };
}

function pairsFromChats(list: DsChat[]): DsMemory[] {
  const out: DsMemory[] = [];
  const ordered = [...list].sort((a, b) => a.updatedAt - b.updatedAt);
  for (const chat of ordered) {
    for (let i = 1; i < chat.messages.length; i += 1) {
      const prev = chat.messages[i - 1];
      const current = chat.messages[i];
      if (prev?.role === "user" && current?.role === "assistant" && prev.content.trim() && current.content.trim()) {
        out.push({ id: uid(), chatId: chat.id, user: prev.content, answer: current.content, at: chat.updatedAt });
      }
    }
  }
  return out.slice(-80);
}

function formatErr(error: unknown) {
  if (typeof error === "string") return error;
  if (error && typeof error === "object" && "message" in error) return String((error as { message: unknown }).message);
  return String(error);
}

export const useDeepseekStore = defineStore("deepseek", () => {
  const loaded = ref(false);
  const apiKey = ref("");
  const model = ref(FALLBACK_MODELS[0]);
  const models = ref<string[]>([...FALLBACK_MODELS]);
  const memoryOn = ref(true);
  const memories = ref<DsMemory[]>([]);
  const followKeyword = ref("");
  const chats = ref<DsChat[]>([]);
  const activeId = ref("");
  const busy = ref(false);
  const error = ref("");
  let requestId = "";
  let streamMsg: DsMessage | null = null;
  let unlisten: UnlistenFn | undefined;
  let saveTimer = 0;

  const active = computed(() => chats.value.find((chat) => chat.id === activeId.value) ?? null);

  function scheduleSave() {
    if (!loaded.value) return;
    window.clearTimeout(saveTimer);
    saveTimer = window.setTimeout(() => {
      const payload: SavedDesk = {
        apiKey: apiKey.value.trim(),
        model: model.value,
        memoryOn: memoryOn.value,
        memories: memories.value,
        followKeyword: followKeyword.value,
        chats: chats.value.map((chat) => ({
          ...chat,
          messages: chat.messages.map(({ image: _image, ...message }) => message),
        })),
        activeId: activeId.value,
      };
      void tavernWrite(FILE, payload).catch((err) => {
        error.value = `保存对话失败：${formatErr(err)}`;
      });
    }, 400);
  }

  watch([apiKey, model, memoryOn, memories, followKeyword, chats, activeId], scheduleSave, { deep: true });

  async function load() {
    if (loaded.value) return;
    let hadMemories = true;
    try {
      const raw = await tavernRead<SavedDesk | null>(FILE);
      if (raw && typeof raw === "object") {
        apiKey.value = raw.apiKey ?? "";
        model.value = raw.model || (raw.think ? "deepseek-reasoner" : FALLBACK_MODELS[0]);
        memoryOn.value = raw.memoryOn !== false;
        hadMemories = Array.isArray(raw.memories);
        memories.value = hadMemories ? raw.memories ?? [] : [];
        followKeyword.value = raw.followKeyword ?? "";
        chats.value = Array.isArray(raw.chats) ? raw.chats : [];
        activeId.value = raw.activeId || chats.value[0]?.id || "";
      }
    } catch (err) {
      error.value = `读取对话失败：${formatErr(err)}`;
    }
    if (!chats.value.length) {
      const chat = blankChat();
      chats.value = [chat];
      activeId.value = chat.id;
    } else if (!chats.value.some((chat) => chat.id === activeId.value)) {
      activeId.value = chats.value[0].id;
    }
    unlisten = await listen<LlmDelta>("tavern-llm-delta", (event) => {
      const payload = event.payload;
      if (!streamMsg || payload.requestId !== requestId) return;
      if (payload.content) streamMsg.content += payload.content;
      if (payload.reasoning) streamMsg.reasoning += payload.reasoning;
    });
    loaded.value = true;
    if (!hadMemories) memories.value = pairsFromChats(chats.value);
    void refreshModels();
  }

  async function refreshModels() {
    const key = apiKey.value.trim();
    if (!key) {
      models.value = [...FALLBACK_MODELS];
      return;
    }
    try {
      const ids = await llmModels(BASE, key);
      const next = ids.map((id) => id.trim()).filter(Boolean);
      models.value = next.length ? Array.from(new Set([...next, ...FALLBACK_MODELS])) : [...FALLBACK_MODELS];
    } catch {
      models.value = [...FALLBACK_MODELS];
    }
    if (!models.value.includes(model.value)) model.value = models.value[0];
  }

  let modelTimer = 0;
  watch(apiKey, () => {
    if (!loaded.value) return;
    window.clearTimeout(modelTimer);
    modelTimer = window.setTimeout(() => void refreshModels(), 600);
  });

  function newChat() {
    const chat = blankChat();
    chats.value.unshift(chat);
    activeId.value = chat.id;
    error.value = "";
  }

  function openChat(id: string) {
    activeId.value = id;
    error.value = "";
  }

  function removeChat(id: string) {
    if (busy.value) return;
    chats.value = chats.value.filter((chat) => chat.id !== id);
    if (!chats.value.length) {
      const chat = blankChat();
      chats.value = [chat];
      activeId.value = chat.id;
      return;
    }
    if (activeId.value === id) activeId.value = chats.value[0].id;
  }

  function remember(chat: DsChat, assistantId: string) {
    if (!memoryOn.value) return;
    const index = chat.messages.findIndex((message) => message.id === assistantId);
    const answer = chat.messages[index];
    const prompt = index > 0 ? chat.messages[index - 1] : undefined;
    if (!answer || answer.role !== "assistant" || !answer.content.trim()) return;
    if (!prompt || prompt.role !== "user" || !prompt.content.trim()) return;
    const exists = memories.value.some((item) => item.chatId === chat.id && item.user === prompt.content && item.answer === answer.content);
    if (exists) return;
    memories.value = [...memories.value, { id: uid(), chatId: chat.id, user: prompt.content, answer: answer.content, at: Date.now() }].slice(-80);
  }

  function forget(id: string) {
    memories.value = memories.value.filter((item) => item.id !== id);
  }

  function historyMessages(chat: DsChat): LlmMessage[] {
    const messages: LlmMessage[] = [];
    if (memoryOn.value) {
      const prior = memories.value.filter((item) => item.chatId !== chat.id && item.answer.trim()).slice(-24);
      if (prior.length) {
        const body = prior.map((item, index) => `${index + 1}. 用户：${item.user}\n回答：${item.answer}`).join("\n\n");
        messages.push({
          role: "system",
          content: `以下是你被调教之后给出的回答。新的对话要延续这些回答里的要求、口吻和结论，不要推翻。\n\n${body}`,
        });
      }
    }
    for (const message of chat.messages) {
      if (message.role === "assistant" && !message.content.trim()) continue;
      if (message.image) {
        messages.push({
          role: message.role,
          content: [
            { type: "text", text: message.content },
            { type: "image_url", image_url: { url: message.image } },
          ],
        });
      } else {
        messages.push({ role: message.role, content: message.content });
      }
    }
    return messages;
  }

  const followTwists = ["接下一瞬间", "镜头稍近一点", "动作再往前一拍", "光线和表情稍微变一点", "构图不动，细节再推进一步"];

  async function continueFromImage(dataUrl: string) {
    await load();
    const keyword = followKeyword.value.trim();
    if (!keyword) {
      error.value = "先在设置 → DeepSeek 里填写后续关键词";
      return;
    }
    const twist = followTwists[Math.floor(Math.random() * followTwists.length)];
    const text = `参考这张图。下面这段关键词是底，请按画面小幅增改，写成可以接着画的后续，不要整段推翻。\n这次侧重：${twist}\n\n${keyword}`;
    await send(text, dataUrl);
  }

  async function send(text: string, image?: string) {
    const typed = text.trim();
    const content = typed || (image ? "看这张图，按画面内容回答。" : "");
    if (!content || busy.value) return;
    if (!apiKey.value.trim()) {
      error.value = "先填写 DeepSeek API Key";
      return;
    }
    const chat = active.value ?? blankChat();
    if (!chats.value.some((item) => item.id === chat.id)) chats.value.unshift(chat);
    activeId.value = chat.id;
    error.value = "";
    if (chat.title === "新对话") chat.title = typed.replace(/\s+/g, " ").slice(0, 18) || (image ? "看图" : "新对话");
    chat.updatedAt = Date.now();
    chat.messages.push({ id: uid(), role: "user", content, reasoning: "", image });
    const assistant: DsMessage = { id: uid(), role: "assistant", content: "", reasoning: "" };
    chat.messages.push(assistant);
    streamMsg = chat.messages[chat.messages.length - 1];
    requestId = uid();
    busy.value = true;
    try {
      const result = await llmChat({
        requestId,
        baseUrl: BASE,
        apiKey: apiKey.value.trim(),
        model: model.value,
        messages: historyMessages(chat),
        stream: true,
      });
      if (streamMsg) {
        if (result.content) streamMsg.content = result.content;
        if (result.reasoning) streamMsg.reasoning = result.reasoning;
      }
      if (result.finishReason === "cancelled") error.value = "";
      else if (!streamMsg?.content.trim()) error.value = "没有收到回复，可以再发一次";
      else remember(chat, assistant.id);
    } catch (err) {
      error.value = formatErr(err);
      if (streamMsg && !streamMsg.content && !streamMsg.reasoning) {
        const index = chat.messages.findIndex((message) => message.id === assistant.id);
        if (index >= 0) chat.messages.splice(index, 1);
      }
    } finally {
      streamMsg = null;
      busy.value = false;
      chat.updatedAt = Date.now();
    }
  }

  function stop() {
    if (requestId) void llmCancel(requestId);
  }

  return {
    loaded,
    apiKey,
    model,
    models,
    memoryOn,
    memories,
    followKeyword,
    forget,
    chats,
    activeId,
    active,
    busy,
    error,
    load,
    refreshModels,
    newChat,
    openChat,
    removeChat,
    send,
    continueFromImage,
    stop,
  };
});
