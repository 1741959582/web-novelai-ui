import { defineStore } from "pinia";
import { computed, ref, watch } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { generateTxt2img, readImageDataUrl } from "@/api/tauri";
import {
  llmCancel,
  llmChat,
  tavernExportFile,
  tavernRead,
  tavernRemove,
  tavernWrite,
  type CharPreset,
  type ChatFile,
  type ChatMessage,
  type ChatMeta,
  type ImageDraft,
  type LlmDelta,
  type LlmMessage,
  type ProviderConfig,
  type ProviderId,
  type StylePreset,
  type TavernCard,
  type TavernConfig,
  type TavernState,
} from "@/api/tavern";
import { useAppStore } from "@/stores/app";
import { isV4Plus, type CharCaption } from "@/types/nai";
import { emptyCard, exportCardJson, exportCardPng, parseCardFile, utf8ToBase64, type NaiStudioExt } from "@/utils/charaCard";

// ---------------------------------------------------------------------------
// 默认值
// ---------------------------------------------------------------------------

export const FOLLOW_STYLE = "__follow__";

export const DEFAULT_MAIN_PROMPT = `你正在进行一场沉浸式角色扮演。你扮演 {{char}}，用户扮演 {{user}}。
- 始终以 {{char}} 的身份、语气和视角回复，保持人设一致。
- 描写 {{char}} 的言行、神态、心理与周围环境，主动推动情节发展。
- 不要替 {{user}} 说话、行动或做决定。
- 回复使用中文，动作与描写可以用 *星号* 包裹。`;

export const DEFAULT_DIRECTOR_PROMPT = `你是 NovelAI 插图提示词导演。根据角色扮演对话的最新情节，为「最后一条消息描述的那一刻」设计一张插图，并写成 NovelAI / Danbooru 风格的英文标签。

只输出一个 JSON 对象，不要解释，不要代码块：
{"orientation":"portrait","scene":"...","negative":"","characters":[{"name":"...","tags":"...","x":0.5,"y":0.5}]}

字段规则：
- orientation：portrait（竖图）/ landscape（横图）/ square（方图），按构图选择。
- scene：人数标签（如 1girl, 1boy, 2girls, solo）、镜头与构图（close-up, cowboy shot, from above…）、地点与背景、时间、光线、氛围。不要写角色外貌。
- characters：只列出画面中出现的角色，最多 6 个。
  - name：必须从【可用角色】中原样选择；不在列表里的角色写 "other"。
  - tags：列表中的角色只写此刻的表情、动作、姿势、视线、服装状态等，不要重复发色瞳色等固定外貌（外貌由预设提供）；"other" 角色要写完整外貌 + 动作。
  - x、y：角色在画面中的大致中心位置（0~1），多人时请分开。
- negative：通常留空，只在需要排除特定元素时填写。
- 所有标签用英文逗号分隔，使用 Danbooru 常见写法，不要写句子。

【可用角色】
{{cast}}`;

function defaultProviders(): Record<ProviderId, ProviderConfig> {
  return {
    deepseek: {
      id: "deepseek",
      label: "DeepSeek",
      baseUrl: "https://api.deepseek.com",
      apiKey: "",
      model: "deepseek-chat",
      directorModel: "",
    },
    xai: {
      id: "xai",
      label: "xAI (Grok)",
      baseUrl: "https://api.x.ai/v1",
      apiKey: "",
      model: "grok-4",
      directorModel: "",
    },
    custom: {
      id: "custom",
      label: "自定义（OpenAI 兼容）",
      baseUrl: "",
      apiKey: "",
      model: "",
      directorModel: "",
    },
  };
}

function defaultConfig(): TavernConfig {
  return {
    provider: "deepseek",
    providers: defaultProviders(),
    temperature: 0.9,
    topP: 1,
    maxTokens: 1200,
    stream: true,
    contextMessages: 40,
    loreScanDepth: 4,
    userName: "User",
    userPersona: "",
    mainPrompt: DEFAULT_MAIN_PROMPT,
    directorPrompt: DEFAULT_DIRECTOR_PROMPT,
    showReasoning: false,
    imageMode: "confirm",
    sizeMode: "auto",
    extraNegative: "",
    directorContext: 6,
  };
}

function defaultState(): TavernState {
  return {
    version: 1,
    config: defaultConfig(),
    cards: [],
    charPresets: [],
    stylePresets: [],
    chats: [],
    activeCardId: "",
    activeChatId: "",
  };
}

function mergeState(raw: Partial<TavernState> | null): TavernState {
  const base = defaultState();
  if (!raw || typeof raw !== "object") return base;
  const cfg = { ...base.config, ...(raw.config ?? {}) } as TavernConfig;
  const providers = defaultProviders();
  for (const id of Object.keys(providers) as ProviderId[]) {
    providers[id] = { ...providers[id], ...(raw.config?.providers?.[id] ?? {}), id };
  }
  cfg.providers = providers;
  if (!providers[cfg.provider]) cfg.provider = "deepseek";
  return {
    ...base,
    ...raw,
    config: cfg,
    cards: (raw.cards ?? []).map((c) => ({ ...emptyCard(c.name), ...c })),
    charPresets: raw.charPresets ?? [],
    stylePresets: raw.stylePresets ?? [],
    chats: raw.chats ?? [],
  };
}

// ---------------------------------------------------------------------------
// 工具函数
// ---------------------------------------------------------------------------

const uid = () => crypto.randomUUID();

function formatErr(e: unknown) {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) return String((e as { message: unknown }).message);
  return String(e);
}

export function joinTags(...parts: Array<string | undefined | null>) {
  return parts
    .map((p) => (p ?? "").trim().replace(/^,+|,+$/g, "").trim())
    .filter(Boolean)
    .join(", ");
}

export function msgText(m: ChatMessage) {
  return m.swipes[m.swipeIdx] ?? "";
}

function newMessage(role: ChatMessage["role"], text = ""): ChatMessage {
  return {
    id: uid(),
    role,
    swipes: [text],
    reasoning: [""],
    swipeIdx: 0,
    images: [],
    draft: null,
    draftState: "",
    draftError: "",
    createdAt: Date.now(),
  };
}

function clamp01(v: unknown, fallback = 0.5) {
  const n = Number(v);
  if (!Number.isFinite(n)) return fallback;
  return Math.min(0.9, Math.max(0.1, n));
}

function extractJson(text: string) {
  const cleaned = text.replace(/```(?:json)?/gi, "").trim();
  const start = cleaned.indexOf("{");
  const end = cleaned.lastIndexOf("}");
  if (start < 0 || end <= start) throw new Error("AI 没有返回 JSON，可以点「重新构思」再试");
  return JSON.parse(cleaned.slice(start, end + 1));
}

const SIZE_BY_ORIENTATION = {
  portrait: { width: 832, height: 1216 },
  landscape: { width: 1216, height: 832 },
  square: { width: 1024, height: 1024 },
} as const;

function debounce(fn: () => void, ms: number) {
  let t: number | undefined;
  const run = () => {
    if (t) window.clearTimeout(t);
    t = window.setTimeout(() => {
      t = undefined;
      fn();
    }, ms);
  };
  run.flush = () => {
    if (t) {
      window.clearTimeout(t);
      t = undefined;
      fn();
    }
  };
  return run;
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

export const useTavernStore = defineStore("tavern", () => {
  const app = useAppStore();

  const loaded = ref(false);
  const state = ref<TavernState>(defaultState());
  const chat = ref<ChatFile | null>(null);
  const busy = ref(false);
  const error = ref("");
  const notice = ref("");
  const imageUrls = ref<Record<string, string>>({});

  let requestId = "";
  let streamTarget: ChatMessage | null = null;
  let unlisten: UnlistenFn | null = null;
  let suppressSave = false;

  // ---------------- computed ----------------
  const config = computed(() => state.value.config);
  const provider = computed(() => state.value.config.providers[state.value.config.provider]);
  const activeCard = computed(() => state.value.cards.find((c) => c.id === state.value.activeCardId) ?? null);
  const cardChats = computed(() =>
    state.value.chats
      .filter((c) => c.cardId === state.value.activeCardId)
      .sort((a, b) => b.updatedAt - a.updatedAt),
  );
  const cast = computed<CharPreset[]>(() => {
    const ids = chat.value?.castIds ?? [];
    return ids.map((id) => state.value.charPresets.find((p) => p.id === id)).filter((p): p is CharPreset => Boolean(p));
  });
  const activeStyle = computed<StylePreset | null>(() => {
    const id = chat.value?.styleId ?? "";
    if (id === FOLLOW_STYLE) {
      return { id: FOLLOW_STYLE, name: "跟随生成页", prompt: app.params.stylePrompt, negative: "" };
    }
    return state.value.stylePresets.find((s) => s.id === id) ?? null;
  });

  // ---------------- 持久化 ----------------
  const saveState = debounce(() => {
    void tavernWrite("state", state.value).catch((e) => (error.value = `保存失败：${formatErr(e)}`));
  }, 600);

  const saveChat = debounce(() => {
    const c = chat.value;
    if (!c) return;
    void tavernWrite(`chat_${c.id}`, c).catch((e) => (error.value = `保存聊天失败：${formatErr(e)}`));
    const meta = state.value.chats.find((m) => m.id === c.id);
    if (meta) {
      const firstUser = c.messages.find((m) => m.role === "user");
      const title = firstUser ? msgText(firstUser).replace(/\s+/g, " ").slice(0, 24) : meta.title;
      if (meta.count !== c.messages.length || meta.title !== title) {
        meta.count = c.messages.length;
        meta.title = title || meta.title;
        meta.updatedAt = Date.now();
      }
    }
  }, 800);

  watch(state, () => { if (loaded.value && !suppressSave) saveState(); }, { deep: true });
  watch(chat, () => { if (loaded.value && !suppressSave) saveChat(); }, { deep: true });

  function flush() {
    saveState.flush();
    saveChat.flush();
  }

  async function load() {
    if (loaded.value) return;
    try {
      const raw = await tavernRead<Partial<TavernState>>("state");
      suppressSave = true;
      state.value = mergeState(raw);
      unlisten = await listen<LlmDelta>("tavern-llm-delta", (event) => {
        const p = event.payload;
        if (!streamTarget || p.requestId !== requestId) return;
        const i = streamTarget.swipeIdx;
        if (p.content) streamTarget.swipes[i] = (streamTarget.swipes[i] ?? "") + p.content;
        if (p.reasoning) streamTarget.reasoning[i] = (streamTarget.reasoning[i] ?? "") + p.reasoning;
      });
      if (state.value.activeCardId && activeCard.value) {
        await openChat(state.value.activeChatId);
      }
    } catch (e) {
      error.value = `读取酒馆数据失败：${formatErr(e)}`;
    } finally {
      suppressSave = false;
      loaded.value = true;
    }
  }

  function dispose() {
    flush();
    unlisten?.();
    unlisten = null;
    loaded.value = false;
  }

  // ---------------- 宏与提示词 ----------------
  function fill(text: string, card = activeCard.value) {
    const user = state.value.config.userName || "User";
    const char = card?.name || "Assistant";
    return (text || "")
      .replace(/\{\{char\}\}|<BOT>/gi, char)
      .replace(/\{\{user\}\}|<USER>/gi, user);
  }

  function activeLore(history: ChatMessage[]) {
    const card = activeCard.value;
    if (!card?.lore.length) return [];
    const depth = Math.max(1, state.value.config.loreScanDepth);
    const scan = history.slice(-depth).map(msgText).join("\n").toLowerCase();
    return card.lore.filter(
      (e) => e.enabled && e.content.trim() && (e.constant || e.keys.some((k) => k && scan.includes(k.toLowerCase()))),
    );
  }

  function buildMessages(history: ChatMessage[]): LlmMessage[] {
    const card = activeCard.value;
    const cfg = state.value.config;
    const user = cfg.userName || "User";
    const sys: string[] = [fill(cfg.mainPrompt)];
    if (card) {
      if (card.systemPrompt.trim()) sys.push(fill(card.systemPrompt));
      if (card.description.trim()) sys.push(`【${card.name} 的设定】\n${fill(card.description)}`);
      if (card.personality.trim()) sys.push(`【${card.name} 的性格】\n${fill(card.personality)}`);
      if (card.scenario.trim()) sys.push(`【场景】\n${fill(card.scenario)}`);
    }
    if (cfg.userPersona.trim()) sys.push(`【${user} 的设定】\n${fill(cfg.userPersona)}`);
    const lore = activeLore(history);
    if (lore.length) sys.push(`【世界书】\n${lore.map((e) => fill(e.content)).join("\n\n")}`);
    if (card?.mesExample.trim()) {
      const ex = fill(card.mesExample).replace(/<START>/gi, "---").trim();
      sys.push(`【对话示例】（仅用于参考语气和格式，并非已发生的剧情）\n${ex}`);
    }

    const out: LlmMessage[] = [{ role: "system", content: sys.join("\n\n") }];
    const recent = history.slice(-Math.max(2, cfg.contextMessages));
    for (const m of recent) {
      const content = msgText(m).trim();
      if (!content) continue;
      const last = out[out.length - 1];
      // 连续同角色消息合并（部分推理模型不接受）
      if (last.role === m.role) last.content += `\n\n${content}`;
      else out.push({ role: m.role, content });
    }
    // 部分模型要求 system 之后第一条必须是 user
    if (out.length === 1 || out[1].role !== "user") {
      out.splice(1, 0, { role: "user", content: "[开始新的对话]" });
    }
    const post = card?.postHistoryInstructions.trim();
    if (post) {
      const last = out[out.length - 1];
      if (last.role === "user") last.content += `\n\n（${fill(post)}）`;
      else out.push({ role: "system", content: fill(post) });
    }
    return out;
  }

  function cleanReply(text: string) {
    const card = activeCard.value;
    let t = text.replace(/^\s+/, "");
    if (card?.name) {
      const prefix = new RegExp(`^${card.name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}\\s*[:：]\\s*`);
      t = t.replace(prefix, "");
    }
    return t.trimEnd();
  }

  // ---------------- 角色卡 ----------------
  function selectCard(id: string) {
    if (state.value.activeCardId === id && chat.value?.cardId === id) return;
    flush();
    state.value.activeCardId = id;
    const latest = cardChats.value[0];
    void openChat(latest?.id ?? "");
  }

  function addCard(card = emptyCard()) {
    state.value.cards.unshift(card);
    return state.value.cards[0];
  }

  function touchCard(card: TavernCard) {
    card.updatedAt = Date.now();
  }

  async function deleteCard(id: string) {
    const chats = state.value.chats.filter((c) => c.cardId === id);
    for (const c of chats) await tavernRemove(`chat_${c.id}`).catch(() => undefined);
    state.value.chats = state.value.chats.filter((c) => c.cardId !== id);
    state.value.cards = state.value.cards.filter((c) => c.id !== id);
    if (state.value.activeCardId === id) {
      state.value.activeCardId = "";
      state.value.activeChatId = "";
      chat.value = null;
    }
  }

  function ensureCharPreset(p: Pick<CharPreset, "name" | "note" | "prompt" | "negative">) {
    const hit = state.value.charPresets.find((x) => x.name === p.name && x.prompt === p.prompt);
    if (hit) return hit.id;
    const preset: CharPreset = { id: uid(), name: p.name || "角色", note: p.note || "", prompt: p.prompt || "", negative: p.negative || "" };
    state.value.charPresets.push(preset);
    return preset.id;
  }

  function ensureStylePreset(s: Pick<StylePreset, "name" | "prompt" | "negative">) {
    const hit = state.value.stylePresets.find((x) => x.prompt === s.prompt && x.negative === (s.negative || ""));
    if (hit) return hit.id;
    const preset: StylePreset = { id: uid(), name: s.name || "画风", prompt: s.prompt || "", negative: s.negative || "" };
    state.value.stylePresets.push(preset);
    return preset.id;
  }

  async function importCardFiles(files: FileList | File[]) {
    let ok = 0;
    const errors: string[] = [];
    let lastId = "";
    for (const file of Array.from(files)) {
      try {
        const { card, nai } = await parseCardFile(file);
        if (nai?.characterPrompts?.length) card.charPresetIds = nai.characterPrompts.map(ensureCharPreset);
        if (nai?.style?.prompt) card.stylePresetId = ensureStylePreset(nai.style);
        addCard(card);
        lastId = card.id;
        ok += 1;
      } catch (e) {
        errors.push(`${file.name}：${formatErr(e)}`);
      }
    }
    if (lastId) selectCard(lastId);
    notice.value = ok ? `已导入 ${ok} 张角色卡` : "";
    error.value = errors.join("；");
  }

  function naiExtFor(card: TavernCard): NaiStudioExt {
    const chars = card.charPresetIds
      .map((id) => state.value.charPresets.find((p) => p.id === id))
      .filter((p): p is CharPreset => Boolean(p))
      .map(({ name, note, prompt, negative }) => ({ name, note, prompt, negative }));
    const style = state.value.stylePresets.find((s) => s.id === card.stylePresetId);
    return {
      characterPrompts: chars,
      style: style ? { name: style.name, prompt: style.prompt, negative: style.negative } : null,
    };
  }

  async function exportCard(card: TavernCard, format: "png" | "json") {
    try {
      const nai = naiExtFor(card);
      const safe = card.name.replace(/[\\/:*?"<>|]/g, "_") || "character";
      const data = format === "png" ? await exportCardPng(card, nai) : exportCardJson(card, nai);
      const saved = await tavernExportFile(`${safe}.${format}`, data);
      if (saved) notice.value = `已导出到 ${saved}`;
    } catch (e) {
      error.value = `导出失败：${formatErr(e)}`;
    }
  }

  // ---------------- 预设导入导出 ----------------
  async function exportPresets() {
    const data = {
      type: "nai-studio-tavern-presets",
      charPresets: state.value.charPresets,
      stylePresets: state.value.stylePresets,
    };
    try {
      const saved = await tavernExportFile("tavern-presets.json", utf8ToBase64(JSON.stringify(data, null, 2)));
      if (saved) notice.value = `已导出到 ${saved}`;
    } catch (e) {
      error.value = `导出失败：${formatErr(e)}`;
    }
  }

  async function importPresets(file: File) {
    try {
      const data = JSON.parse(await file.text());
      let n = 0;
      for (const p of data.charPresets ?? []) {
        ensureCharPreset(p);
        n++;
      }
      for (const s of data.stylePresets ?? []) {
        ensureStylePreset(s);
        n++;
      }
      notice.value = `已导入 ${n} 个预设`;
    } catch (e) {
      error.value = `导入失败：${formatErr(e)}`;
    }
  }

  function importCharsFromGeneratePage() {
    const list = app.characters.filter((c) => c.prompt.trim());
    if (!list.length) {
      error.value = "生成页没有填写角色提示词";
      return;
    }
    list.forEach((c) =>
      ensureCharPreset({ name: `角色 ${state.value.charPresets.length + 1}`, note: "", prompt: c.prompt, negative: c.negativePrompt }),
    );
    notice.value = `已从生成页导入 ${list.length} 个角色提示词，记得改名`;
  }

  function importStyleFromGeneratePage() {
    const prompt = app.params.stylePrompt.trim() || app.params.positivePrompt.trim();
    if (!prompt) {
      error.value = "生成页没有画风/正面提示词";
      return;
    }
    ensureStylePreset({ name: `画风 ${state.value.stylePresets.length + 1}`, prompt, negative: "" });
    notice.value = "已从生成页导入画风";
  }

  // ---------------- 聊天 ----------------
  async function openChat(id: string) {
    const card = activeCard.value;
    if (!card) return;
    if (!id) {
      await newChat();
      return;
    }
    try {
      const file = await tavernRead<ChatFile>(`chat_${id}`);
      if (!file || file.cardId !== card.id) {
        if (!file) state.value.chats = state.value.chats.filter((c) => c.id !== id);
        const other = state.value.chats.filter((c) => c.cardId === card.id && c.id !== id).sort((a, b) => b.updatedAt - a.updatedAt)[0];
        if (other) return openChat(other.id);
        await newChat();
        return;
      }
      suppressSave = true;
      chat.value = file;
      state.value.activeChatId = id;
      void preloadImages();
    } catch (e) {
      error.value = formatErr(e);
    } finally {
      suppressSave = false;
    }
  }

  async function newChat() {
    const card = activeCard.value;
    if (!card) return;
    flush();
    const id = uid();
    const messages: ChatMessage[] = [];
    const greetings = [card.firstMes, ...card.alternateGreetings].map((g) => fill(g, card)).filter((g) => g.trim());
    if (greetings.length) {
      const m = newMessage("assistant");
      m.swipes = greetings;
      m.reasoning = greetings.map(() => "");
      messages.push(m);
    }
    const file: ChatFile = {
      id,
      cardId: card.id,
      messages,
      castIds: [...card.charPresetIds],
      styleId: card.stylePresetId,
    };
    const meta: ChatMeta = {
      id,
      cardId: card.id,
      title: new Date().toLocaleString(),
      count: messages.length,
      updatedAt: Date.now(),
    };
    state.value.chats.push(meta);
    chat.value = file;
    state.value.activeChatId = id;
    await tavernWrite(`chat_${id}`, file).catch(() => undefined);
  }

  /** 卡片编辑后：如果当前对话还没开始，按最新的开场白 / 出场角色 / 画风重置 */
  function refreshEmptyChat() {
    const card = activeCard.value;
    const c = chat.value;
    if (!card || !c || c.cardId !== card.id) return;
    if (c.messages.some((m) => m.role === "user")) return;
    const greetings = [card.firstMes, ...card.alternateGreetings].map((g) => fill(g, card)).filter((g) => g.trim());
    if (greetings.length) {
      const m = newMessage("assistant");
      m.swipes = greetings;
      m.reasoning = greetings.map(() => "");
      c.messages = [m];
    } else {
      c.messages = [];
    }
    c.castIds = [...card.charPresetIds];
    c.styleId = card.stylePresetId;
  }

  async function deleteChat(id: string) {
    await tavernRemove(`chat_${id}`).catch(() => undefined);
    state.value.chats = state.value.chats.filter((c) => c.id !== id);
    if (chat.value?.id === id) {
      chat.value = null;
      const next = cardChats.value[0];
      await openChat(next?.id ?? "");
    }
  }

  function lastMessage() {
    const list = chat.value?.messages ?? [];
    return list[list.length - 1] ?? null;
  }

  function pushMessage(m: ChatMessage) {
    const list = chat.value!.messages;
    list.push(m);
    return list[list.length - 1]; // 取回响应式代理
  }

  async function runReply(target: ChatMessage, history: ChatMessage[]) {
    const p = provider.value;
    const cfg = state.value.config;
    busy.value = true;
    error.value = "";
    requestId = uid();
    streamTarget = target;
    const i = target.swipeIdx;
    target.swipes[i] = "";
    target.reasoning[i] = "";
    let ok = false;
    try {
      const res = await llmChat({
        requestId,
        baseUrl: p.baseUrl,
        apiKey: p.apiKey,
        model: p.model,
        messages: buildMessages(history),
        temperature: cfg.temperature,
        topP: cfg.topP,
        maxTokens: cfg.maxTokens,
        stream: cfg.stream,
      });
      target.swipes[i] = cleanReply(res.content || target.swipes[i] || "");
      if (res.reasoning) target.reasoning[i] = res.reasoning;
      if (res.finishReason === "cancelled") notice.value = "已停止";
      else if (!target.swipes[i].trim()) error.value = "AI 返回了空内容，可以重新生成";
      else ok = true;
      if (res.finishReason === "length") notice.value = "回复被长度上限截断，可在「接口」里调大最大长度";
    } catch (e) {
      error.value = formatErr(e);
      if (!target.swipes[i]) target.swipes[i] = "";
    } finally {
      streamTarget = null;
      busy.value = false;
    }
    if (ok && cfg.imageMode !== "manual") {
      await planImage(target);
      if (cfg.imageMode === "auto" && target.draftState === "ready") await drawImage(target);
    }
  }

  async function send(text: string): Promise<void> {
    if (busy.value || !chat.value || !activeCard.value) return;
    const t = text.trim();
    if (t) pushMessage(newMessage("user", t));
    const last = lastMessage();
    if (!last || last.role !== "user") {
      // 没有新输入且最后是 AI：视为重新生成
      if (last?.role === "assistant") return regenerate();
      return;
    }
    const history = [...chat.value.messages];
    const target = pushMessage(newMessage("assistant"));
    await runReply(target, history);
  }

  async function regenerate(): Promise<void> {
    if (busy.value || !chat.value) return;
    const last = lastMessage();
    if (!last) return;
    if (last.role === "user") return send("");
    last.swipes.push("");
    last.reasoning.push("");
    last.swipeIdx = last.swipes.length - 1;
    last.draft = null;
    last.draftState = "";
    await runReply(last, chat.value.messages.slice(0, -1));
  }

  async function swipe(m: ChatMessage, dir: -1 | 1) {
    if (busy.value) return;
    if (dir < 0) {
      if (m.swipeIdx > 0) m.swipeIdx--;
      return;
    }
    if (m.swipeIdx < m.swipes.length - 1) {
      m.swipeIdx++;
      return;
    }
    if (m === lastMessage() && m.role === "assistant") await regenerate();
  }

  function editMessage(m: ChatMessage, text: string) {
    m.swipes[m.swipeIdx] = text;
  }

  function deleteMessage(m: ChatMessage) {
    if (!chat.value) return;
    chat.value.messages = chat.value.messages.filter((x) => x.id !== m.id);
  }

  function stop() {
    if (requestId) void llmCancel(requestId);
  }

  // ---------------- 生图 ----------------
  function presetById(id: string) {
    return state.value.charPresets.find((p) => p.id === id);
  }

  function matchPreset(name: string) {
    const n = name.trim().toLowerCase();
    if (!n || n === "other") return undefined;
    return (
      cast.value.find((p) => p.name.trim().toLowerCase() === n) ??
      cast.value.find((p) => p.note.toLowerCase().split(/[,，/、\s]+/).includes(n)) ??
      cast.value.find((p) => n.includes(p.name.trim().toLowerCase()) || p.name.trim().toLowerCase().includes(n))
    );
  }

  async function planImage(m: ChatMessage) {
    if (!chat.value || !activeCard.value) return;
    const cfg = state.value.config;
    const p = provider.value;
    const card = activeCard.value;
    const list = chat.value.messages;
    const idx = list.findIndex((x) => x.id === m.id);
    if (idx < 0) return;
    m.draftState = "loading";
    m.draftError = "";
    try {
      const ctx = list.slice(Math.max(0, idx + 1 - Math.max(1, cfg.directorContext)), idx + 1);
      const user = cfg.userName || "User";
      const transcript = ctx
        .map((x) => `${x.role === "user" ? user : card.name}：${msgText(x)}`)
        .join("\n\n");
      const castText = cast.value.length
        ? cast.value.map((c) => `- ${c.name}${c.note ? `（${c.note}）` : ""}`).join("\n")
        : "（没有预设，所有角色都写 other 并写完整外貌）";
      const res = await llmChat({
        requestId: uid(),
        baseUrl: p.baseUrl,
        apiKey: p.apiKey,
        model: p.directorModel.trim() || p.model,
        stream: false,
        temperature: 0.5,
        messages: [
          { role: "system", content: cfg.directorPrompt.replace("{{cast}}", castText) },
          {
            role: "user",
            content: `主角：${card.name}；用户扮演：${user}\n\n【对话】\n${transcript}\n\n请为最后一条消息的画面输出 JSON。`,
          },
        ],
      });
      const json = extractJson(res.content);
      const orientation = ["portrait", "landscape", "square"].includes(json.orientation) ? json.orientation : "portrait";
      const chars = (Array.isArray(json.characters) ? json.characters : []).slice(0, 6).map((c: any) => {
        const preset = matchPreset(String(c?.name ?? ""));
        return {
          presetId: preset?.id ?? "",
          name: preset?.name ?? String(c?.name ?? "other"),
          tags: String(c?.tags ?? ""),
          x: clamp01(c?.x),
          y: clamp01(c?.y),
        };
      });
      m.draft = {
        orientation,
        scene: String(json.scene ?? ""),
        negative: String(json.negative ?? ""),
        chars,
      } as ImageDraft;
      m.draftState = "ready";
    } catch (e) {
      m.draftState = "error";
      m.draftError = formatErr(e);
    }
  }

  function emptyDraft(): ImageDraft {
    return {
      orientation: "portrait",
      scene: "",
      negative: "",
      chars: cast.value.map((c, i, arr) => ({
        presetId: c.id,
        name: c.name,
        tags: "",
        x: arr.length > 1 ? clamp01((i + 1) / (arr.length + 1)) : 0.5,
        y: 0.5,
      })),
    };
  }

  function manualDraft(m: ChatMessage) {
    m.draft = emptyDraft();
    m.draftState = "ready";
    m.draftError = "";
  }

  function buildNaiRequest(draft: ImageDraft) {
    const p = app.params;
    const cfg = state.value.config;
    const style = activeStyle.value;
    const v4 = isV4Plus(p.model);
    const chars = draft.chars.filter((c) => c.tags.trim() || c.presetId);
    const multi = chars.length > 1;
    const captions: CharCaption[] = chars.map((c) => {
      const preset = c.presetId ? presetById(c.presetId) : undefined;
      return {
        id: uid(),
        prompt: joinTags(preset?.prompt, c.tags),
        negativePrompt: preset?.negative ?? "",
        useCoords: multi,
        x: clamp01(c.x),
        y: clamp01(c.y),
        enabled: true,
      };
    });
    const positive = v4 ? draft.scene : joinTags(draft.scene, ...captions.map((c) => c.prompt));
    const size = cfg.sizeMode === "follow" ? { width: p.width, height: p.height } : SIZE_BY_ORIENTATION[draft.orientation];
    return {
      ...p,
      stylePrompt: style?.prompt ?? "",
      positivePrompt: positive,
      negativePrompt: joinTags(style?.negative, cfg.extraNegative, draft.negative),
      width: size.width,
      height: size.height,
      seed: 0,
      seedMode: "random" as const,
      fileNamePrefix: p.fileNamePrefix || "tavern",
      charCaptions: v4 ? captions : [],
      vibeImages: [],
      preciseReferences: [],
    };
  }

  async function drawImage(m: ChatMessage) {
    if (!m.draft) return;
    if (!app.hasToken) {
      m.draftState = "error";
      m.draftError = "请先在「设置」里填写 NovelAI Token";
      return;
    }
    if (app.busy) {
      m.draftState = "error";
      m.draftError = "生成页正在出图，请稍后再点生成";
      return;
    }
    m.draftState = "generating";
    m.draftError = "";
    app.busy = true;
    app.status = "酒馆生图中…";
    try {
      const res = await generateTxt2img(buildNaiRequest(m.draft));
      app.status = res.message;
      if (res.account) app.account = res.account;
      if (!res.items.length) throw new Error(res.message || "没有返回图片");
      app.history = [...res.items, ...app.history];
      for (const item of res.items) {
        m.images.push({ id: item.id, path: item.path, seed: res.actualSeed, createdAt: Date.now() });
        void ensureImage(item.path);
      }
      m.draftState = "ready";
    } catch (e) {
      m.draftState = "error";
      m.draftError = formatErr(e);
    } finally {
      app.busy = false;
    }
  }

  function removeImage(m: ChatMessage, id: string) {
    m.images = m.images.filter((i) => i.id !== id);
  }

  async function ensureImage(path: string) {
    if (!path || imageUrls.value[path]) return;
    try {
      imageUrls.value = { ...imageUrls.value, [path]: await readImageDataUrl(path) };
    } catch {
      imageUrls.value = { ...imageUrls.value, [path]: "" };
    }
  }

  async function preloadImages() {
    const paths = (chat.value?.messages ?? []).flatMap((m) => m.images.map((i) => i.path));
    for (const p of paths) await ensureImage(p);
  }

  return {
    loaded,
    state,
    chat,
    busy,
    error,
    notice,
    imageUrls,
    config,
    provider,
    activeCard,
    cardChats,
    cast,
    activeStyle,
    load,
    dispose,
    flush,
    fill,
    selectCard,
    addCard,
    touchCard,
    deleteCard,
    importCardFiles,
    exportCard,
    exportPresets,
    importPresets,
    importCharsFromGeneratePage,
    importStyleFromGeneratePage,
    openChat,
    newChat,
    refreshEmptyChat,
    deleteChat,
    lastMessage,
    send,
    regenerate,
    swipe,
    editMessage,
    deleteMessage,
    stop,
    planImage,
    manualDraft,
    drawImage,
    removeImage,
    ensureImage,
    buildNaiRequest,
  };
});
