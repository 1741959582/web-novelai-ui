import { defineStore } from "pinia";
import { computed, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { llmCancel, llmChat, tavernRead, tavernWrite, type LlmDelta, type LlmMessage, type LlmUsage } from "@/api/tavern";
import { useDeepseekStore } from "@/stores/deepseekChat";
import { splitPromptTags } from "@/utils/promptWeight";

const FILE = "nai_agent";
const BASE = "https://api.deepseek.com";

export interface AgentMessage {
  id: string;
  role: "user" | "assistant";
  content: string;
  reasoning: string;
  image?: string;
}

export type AgentGlyph = "eye" | "image" | "sparkle" | "layers";
export type CacheWarming = "off" | "streaming" | "idle";

export interface AgentQuick {
  id: string;
  glyph: AgentGlyph;
  title: string;
  hint: string;
  prompt: string;
  send?: boolean;
}

export interface AgentMcpServer {
  id: string;
  name: string;
  json: string;
  enabled: boolean;
}

export interface AgentUsage {
  input: number;
  output: number;
  cacheRead: number;
  cacheWrite: number;
  lastPrompt: number;
  turns: number;
}

export interface AgentChat {
  id: string;
  title: string;
  updatedAt: number;
  messages: AgentMessage[];
  usage?: AgentUsage;
}

/** 没有模型回传用量时，用字数估算 token。中文大约一字一 token。 */
export function estimateTokens(text: string) {
  let units = 0;
  for (let i = 0; i < text.length; i += 1) {
    units += text.charCodeAt(i) > 127 ? 1 : 0.3;
  }
  return Math.ceil(units);
}

export interface AgentPatch {
  positive?: string;
  negative?: string;
}

interface SavedAgent {
  activeId?: string;
  chats?: AgentChat[];
  modelId?: string;
  permission?: "auto" | "bypass";
  instruction?: string;
  prefix?: string;
  quick?: AgentQuick[];
  enterToSend?: boolean;
  maxRequests?: number;
  skipPaid?: boolean;
  nextSuggestion?: boolean;
  cacheWarming?: CacheWarming;
  mcpServers?: AgentMcpServer[];
}

export const DEFAULT_QUICK: AgentQuick[] = [
  {
    id: "builtin-review",
    glyph: "eye",
    title: "检查草稿",
    hint: "找冲突、冗余和模型不认的写法",
    prompt: "检查当前草稿和参数，找出冲突、冗余和当前模型不认的写法，先不要修改。",
  },
  {
    id: "builtin-analyze",
    glyph: "image",
    title: "分析参考图",
    hint: "拆出画风、构图和可用标签",
    prompt: "分析我附上的参考图：画风、构图、光影，以及能直接用的标签，先不要生成。",
  },
  {
    id: "builtin-variants",
    glyph: "sparkle",
    title: "批量变体",
    hint: "改一处，出几张对比",
    prompt: "基于当前草稿，只改一个方向做几组变体对比。先告诉我打算怎么改和费用，等我确认再生成。",
  },
  {
    id: "builtin-blocks",
    glyph: "layers",
    title: "整理积木",
    hint: "把常用片段收进积木库",
    prompt: "看看当前草稿里哪些片段适合做成可复用的积木，列出建议，先不要修改。",
  },
];

export const AGENT_GLYPHS: AgentGlyph[] = ["eye", "image", "sparkle", "layers"];

export const AGENT_BUILTIN_PROMPT = `你是 NAI Agent，嵌在 NovelAI 生图页里，专门整理 Danbooru 英文标签。
标签用逗号分隔。权重大于 1 写成 1.2::tag::，小于 1 写成 0.8::tag::。不要写出 1 倍权重。
先用简短中文说明这些标签适合放进主提示词还是哪个角色。
需要给出可用标签时，在回复最后单独给一个语言标记为 nai 的代码块，内容是 JSON，只放建议写入的标签：
{"positive":"tag1, tag2","negative":"tag3"}
不要在 JSON 里写整段现有提示词，也不要写解释。用户会自己勾选标签，并决定写进主提示词还是某个 Character。
如果适合给下一句建议，再跟一个 next 代码块，里面只有一句用户可能会接着说的话。`;

export function composeAgentSystem(opts: {
  prefix: string;
  instruction: string;
  scene: string;
  skipPaid: boolean;
  bypass: boolean;
}) {
  const paid = opts.skipPaid && opts.bypass
    ? "用户已在完全权限下关闭付费确认。你仍然不能自己调用生图接口；要生图时给出标签和费用说明，让用户按生成按钮。"
    : "花 Anlas 前先说明费用并等待确认。不要主动要求生图。";
  return [opts.prefix.trim(), AGENT_BUILTIN_PROMPT, opts.instruction.trim(), paid, `当前画面：\n${opts.scene}`]
    .filter(Boolean)
    .join("\n\n");
}

function uid() {
  return crypto.randomUUID();
}

function blankChat(): AgentChat {
  return { id: uid(), title: "新对话", updatedAt: Date.now(), messages: [] };
}

function formatErr(error: unknown) {
  if (typeof error === "string") return error;
  if (error && typeof error === "object" && "message" in error) return String((error as { message: unknown }).message);
  return String(error);
}

export function agentVisibleText(content: string) {
  return content
    .replace(/```nai[\s\S]*?```/gi, "")
    .replace(/```nai[\s\S]*$/i, "")
    .replace(/```next[\s\S]*?```/gi, "")
    .replace(/```next[\s\S]*$/i, "")
    .trim();
}

export function agentNext(content: string) {
  const re = /```next\s*([\s\S]*?)```/gi;
  let match: RegExpExecArray | null;
  let raw = "";
  while ((match = re.exec(content))) raw = match[1];
  return raw.replace(/\s+/g, " ").trim().slice(0, 180);
}

export interface AgentTag {
  key: string;
  text: string;
  side: "positive" | "negative";
}

export function agentTags(content: string): AgentTag[] {
  const patch = agentPatch(content);
  if (!patch) return [];
  const tags: AgentTag[] = [];
  splitPromptTags(patch.positive || "").forEach((text, index) => {
    tags.push({ key: `p:${index}:${text}`, text, side: "positive" });
  });
  splitPromptTags(patch.negative || "").forEach((text, index) => {
    tags.push({ key: `n:${index}:${text}`, text, side: "negative" });
  });
  return tags;
}

export function agentPatch(content: string): AgentPatch | null {
  const re = /```nai\s*([\s\S]*?)```/gi;
  let match: RegExpExecArray | null;
  let raw: string | null = null;
  while ((match = re.exec(content))) raw = match[1];
  if (!raw) return null;
  try {
    const data = JSON.parse(raw) as { positive?: unknown; negative?: unknown };
    const patch: AgentPatch = {};
    if (typeof data.positive === "string") patch.positive = data.positive.trim();
    if (typeof data.negative === "string") patch.negative = data.negative.trim();
    if (patch.positive == null && patch.negative == null) return null;
    return patch;
  } catch {
    return null;
  }
}

function clampRequests(value: unknown) {
  const n = typeof value === "number" ? value : 32;
  if (!Number.isFinite(n)) return 32;
  return Math.max(1, Math.min(200, Math.round(n)));
}

function normalizeQuick(item: AgentQuick): AgentQuick | null {
  if (!item || typeof item !== "object" || typeof item.prompt !== "string" || !item.prompt.trim()) return null;
  const glyph = AGENT_GLYPHS.includes(item.glyph) ? item.glyph : "sparkle";
  return {
    id: item.id || uid(),
    glyph,
    title: (item.title || "快捷信息").slice(0, 40),
    hint: item.hint || "",
    prompt: item.prompt,
    send: item.send === true,
  };
}

function validMcp(item: AgentMcpServer): item is AgentMcpServer {
  return Boolean(item && typeof item.id === "string" && typeof item.name === "string" && typeof item.json === "string");
}

export const useNaiAgentStore = defineStore("naiAgent", () => {
  const deepseek = useDeepseekStore();
  const loaded = ref(false);
  const chats = ref<AgentChat[]>([]);
  const activeId = ref("");
  const modelId = ref("");
  const permission = ref<"auto" | "bypass">("auto");
  const instruction = ref("");
  const prefix = ref("");
  const quick = ref<AgentQuick[]>(DEFAULT_QUICK.map((item) => ({ ...item })));
  const enterToSend = ref(true);
  const maxRequests = ref(32);
  const skipPaid = ref(false);
  const nextSuggestion = ref(true);
  const cacheWarming = ref<CacheWarming>("streaming");
  const mcpServers = ref<AgentMcpServer[]>([]);
  const busy = ref(false);
  const error = ref("");
  let requestId = "";
  let streamMsg: AgentMessage | null = null;
  let unlisten: UnlistenFn | undefined;
  let saveTimer = 0;

  const active = computed(() => chats.value.find((chat) => chat.id === activeId.value) ?? null);
  const hasKey = computed(() => Boolean(deepseek.apiKey.trim()));

  function scheduleSave() {
    if (!loaded.value) return;
    window.clearTimeout(saveTimer);
    saveTimer = window.setTimeout(() => {
      const payload: SavedAgent = {
        activeId: activeId.value,
        chats: chats.value,
        modelId: modelId.value,
        permission: permission.value,
        instruction: instruction.value,
        prefix: prefix.value,
        quick: quick.value,
        enterToSend: enterToSend.value,
        maxRequests: maxRequests.value,
        skipPaid: skipPaid.value,
        nextSuggestion: nextSuggestion.value,
        cacheWarming: cacheWarming.value,
        mcpServers: mcpServers.value,
      };
      void tavernWrite(FILE, payload).catch((err) => {
        error.value = `保存对话失败：${formatErr(err)}`;
      });
    }, 400);
  }

  watch(
    [chats, activeId, modelId, permission, instruction, prefix, quick, enterToSend, maxRequests, skipPaid, nextSuggestion, cacheWarming, mcpServers],
    scheduleSave,
    { deep: true },
  );

  async function load() {
    if (loaded.value) return;
    await deepseek.load();
    try {
      const raw = await tavernRead<SavedAgent | null>(FILE);
      if (raw && typeof raw === "object") {
        chats.value = Array.isArray(raw.chats) ? raw.chats : [];
        activeId.value = raw.activeId || chats.value[0]?.id || "";
        modelId.value = raw.modelId || "";
        permission.value = raw.permission === "bypass" ? "bypass" : "auto";
        instruction.value = raw.instruction || "";
        prefix.value = raw.prefix || "";
        enterToSend.value = raw.enterToSend !== false;
        maxRequests.value = clampRequests(raw.maxRequests);
        skipPaid.value = raw.skipPaid === true;
        nextSuggestion.value = raw.nextSuggestion !== false;
        cacheWarming.value = raw.cacheWarming === "off" || raw.cacheWarming === "idle" ? raw.cacheWarming : "streaming";
        mcpServers.value = Array.isArray(raw.mcpServers) ? raw.mcpServers.filter(validMcp) : [];
        const savedQuick = Array.isArray(raw.quick) ? raw.quick.map(normalizeQuick).filter((item) => item != null) : [];
        if (savedQuick.length) quick.value = savedQuick;
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
    try {
      unlisten = await listen<LlmDelta>("tavern-llm-delta", (event) => {
        const payload = event.payload;
        if (!streamMsg || payload.requestId !== requestId) return;
        if (payload.content) streamMsg.content += payload.content;
        if (payload.reasoning) streamMsg.reasoning += payload.reasoning;
      });
    } catch {
      /* 浏览器预览没有 Tauri 事件 */
    }
    loaded.value = true;
  }

  function newChat() {
    if (busy.value) return;
    const chat = blankChat();
    chats.value.unshift(chat);
    activeId.value = chat.id;
    error.value = "";
  }

  function selectChat(id: string) {
    if (busy.value) return;
    activeId.value = id;
    error.value = "";
  }

  function renameChat(id: string, title: string) {
    const name = title.trim();
    if (!name) return;
    const chat = chats.value.find((item) => item.id === id);
    if (chat) chat.title = name.slice(0, 40);
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

  function restoreQuick() {
    quick.value = DEFAULT_QUICK.map((item) => ({ ...item }));
  }

  function recordUsage(chat: AgentChat, usage: LlmUsage) {
    const prompt = usage.promptTokens || 0;
    const completion = usage.completionTokens || 0;
    if (!prompt && !completion) return;
    if (!chat.usage) {
      let input = 0;
      let output = 0;
      for (const message of chat.messages.slice(0, -2)) {
        const tokens = estimateTokens(`${message.content}\n${message.reasoning}`) + (message.image ? 1200 : 0);
        if (message.role === "user") input += tokens;
        else output += tokens;
      }
      chat.usage = { input, output, cacheRead: 0, cacheWrite: 0, lastPrompt: 0, turns: 0 };
    }
    chat.usage.input += prompt;
    chat.usage.output += completion;
    chat.usage.cacheRead += usage.cacheReadTokens || 0;
    chat.usage.cacheWrite += usage.cacheWriteTokens || 0;
    chat.usage.lastPrompt = prompt;
    chat.usage.turns += 1;
  }

  async function send(text: string, scene: string, image?: string) {
    const typed = text.trim();
    if (!typed || busy.value) return;
    await load();
    if (!modelId.value) return;
    if (!deepseek.apiKey.trim()) {
      error.value = "先到 DeepSeek 页填写 API Key，NAI Agent 用同一把钥匙。";
      return;
    }
    const chat = active.value ?? blankChat();
    if (!chats.value.some((item) => item.id === chat.id)) chats.value.unshift(chat);
    activeId.value = chat.id;
    error.value = "";
    if (chat.title === "新对话") chat.title = typed.replace(/\s+/g, " ").slice(0, 18);
    chat.updatedAt = Date.now();
    chat.messages.push({ id: uid(), role: "user", content: typed, reasoning: "", image });
    const assistant: AgentMessage = { id: uid(), role: "assistant", content: "", reasoning: "" };
    chat.messages.push(assistant);
    streamMsg = chat.messages[chat.messages.length - 1];
    requestId = uid();
    busy.value = true;
    const cap = clampRequests(maxRequests.value) * 2;
    const history = chat.messages
      .filter((message) => message.role === "user" || message.content.trim() || message.reasoning.trim())
      .slice(-cap);
    const messages: LlmMessage[] = [
      {
        role: "system",
        content: composeAgentSystem({
          prefix: prefix.value,
          instruction: instruction.value,
          scene,
          skipPaid: skipPaid.value,
          bypass: permission.value === "bypass",
        }),
      },
      ...history
        .map((message) => (
          message.image
            ? {
                role: message.role,
                content: [
                  { type: "text" as const, text: message.content },
                  { type: "image_url" as const, image_url: { url: message.image } },
                ],
              }
            : { role: message.role, content: message.content }
        )),
    ];
    try {
      const result = await llmChat({
        requestId,
        baseUrl: BASE,
        apiKey: deepseek.apiKey.trim(),
        model: modelId.value,
        messages,
        stream: true,
      });
      if (streamMsg) {
        if (result.content) streamMsg.content = result.content;
        if (result.reasoning) streamMsg.reasoning = result.reasoning;
      }
      if (result.usage) recordUsage(chat, result.usage);
      if (result.finishReason === "cancelled") error.value = "";
      else if (!streamMsg?.content.trim()) error.value = "没有收到回复，可以再发一次";
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
    chats,
    activeId,
    active,
    modelId,
    permission,
    instruction,
    prefix,
    quick,
    enterToSend,
    maxRequests,
    skipPaid,
    nextSuggestion,
    cacheWarming,
    mcpServers,
    busy,
    error,
    hasKey,
    models: computed(() => deepseek.models),
    load,
    newChat,
    selectChat,
    renameChat,
    removeChat,
    restoreQuick,
    send,
    stop,
  };
});
