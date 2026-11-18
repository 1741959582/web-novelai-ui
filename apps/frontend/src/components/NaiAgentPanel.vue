<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { RouterLink } from "vue-router";
import NaiIcon from "@/components/NaiIcon.vue";
import NaiAgentMark from "@/components/NaiAgentMark.vue";
import NaiAgentSettings from "@/components/NaiAgentSettings.vue";
import { useAppStore } from "@/stores/app";
import { agentNext, agentTags, agentVisibleText, composeAgentSystem, estimateTokens, useNaiAgentStore, type AgentQuick, type AgentTag } from "@/stores/naiAgent";
import { renderMarkdown } from "@/utils/markdown";
import { splitPromptTags } from "@/utils/promptWeight";
import { readImageDataUrl } from "@/api/tauri";
import type { HistoryItem } from "@/types/nai";
import { appendPromptChunk } from "@/utils/textUtils";

const emit = defineEmits<{ close: [] }>();

type Tab = "chat" | "shelf" | "memory";
type Pop = "" | "chats" | "more" | "attach" | "mention" | "mode" | "model" | "usage" | "quick";

const app = useAppStore();
const store = useNaiAgentStore();
const tab = ref<Tab>("chat");
const pop = ref<Pop>("");
const historyOpen = ref(false);
const historyQuery = ref("");
const draft = ref("");
const pendingImage = ref("");
const armedChatId = ref("");
const renameValue = ref("");
const renaming = ref(false);
const applied = ref("");
const scroller = ref<HTMLElement | null>(null);
const box = ref<HTMLTextAreaElement | null>(null);
const filePick = ref<HTMLInputElement | null>(null);
const draftPick = ref<HTMLInputElement | null>(null);
const draftMenu = ref(false);
const historyPick = ref(false);
const historyThumbs = ref<Record<string, string>>({});
const variantBase = ref("");
const variantNote = ref("");
const picked = ref<Record<string, boolean>>({});
const dest = ref("main:positive");
const writeMode = ref<"append" | "replace">("append");
const wrote = ref("");

interface PromptEdit {
  id: string;
  target: string;
  label: string;
  before: string;
  after: string;
}
const edits = ref<PromptEdit[]>([]);
const editListOpen = ref(false);
const reviewOpen = ref(false);
const reviewId = ref("");

function scrollToEnd() {
  nextTick(() => {
    requestAnimationFrame(() => {
      if (scroller.value) scroller.value.scrollTop = scroller.value.scrollHeight;
    });
  });
}

onMounted(async () => {
  await store.load();
  scrollToEnd();
});

const hour = new Date().getHours();
const greet = hour < 12 ? "上午好，今天画什么？" : hour < 18 ? "下午好，今天画什么？" : "晚上好，今天画什么？";

const number = computed(() => {
  const index = store.chats.findIndex((chat) => chat.id === store.activeId);
  const value = index < 0 ? 1 : store.chats.length - index;
  return String(value).padStart(2, "0");
});

const promptLine = computed(() => app.params.positivePrompt.replace(/\s+/g, " ").trim());
const referenceCount = computed(() => (app.i2iImage ? 1 : 0) + app.vibeImages.length + app.preciseReferences.length);
const thumb = computed(() => app.previewUrl || app.i2iImage || app.insetReference?.previewUrl || "");

const tags = computed(() => {
  const messages = store.active?.messages ?? [];
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    if (messages[i].role !== "assistant") continue;
    return agentTags(messages[i].content);
  }
  return [] as AgentTag[];
});
const positiveTags = computed(() => tags.value.filter((tag) => tag.side === "positive"));
const negativeTags = computed(() => tags.value.filter((tag) => tag.side === "negative"));
const tagKey = computed(() => tags.value.map((tag) => tag.key).join("|"));
const welcomeQuick = computed(() => store.quick.filter((item) => item.prompt.trim()).slice(0, 6));
const suggestion = computed(() => {
  if (!store.nextSuggestion || store.busy) return "";
  const messages = store.active?.messages ?? [];
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    if (messages[i].role !== "assistant") continue;
    return agentNext(messages[i].content);
  }
  return "";
});

const shelfImages = computed(() => {
  const images: { id: string; url: string; label: string }[] = [];
  for (const message of store.active?.messages ?? []) {
    if (message.image) images.push({ id: message.id, url: message.image, label: message.role === "user" ? "附件" : "产物" });
  }
  return images;
});

function contextWindow(model: string) {
  const id = model.toLowerCase();
  if (id.includes("1m") || id.includes("1000k")) return 1_000_000;
  return 128_000;
}

function formatTokens(value: number) {
  if (value >= 10000) {
    const scaled = value / 1000;
    return `${scaled >= 100 ? Math.round(scaled) : scaled.toFixed(1)}k`;
  }
  return Math.round(value).toLocaleString("zh-CN");
}

const usageView = computed(() => {
  const chat = store.active;
  const messages = chat?.messages ?? [];
  const windowSize = contextWindow(store.modelId);
  const recorded = chat?.usage;
  const assistantTurns = messages.filter((message) => message.role === "assistant" && (message.content.trim() || message.reasoning.trim())).length;
  const measured = !!recorded && recorded.turns > 0 && recorded.turns >= assistantTurns;
  let input = 0;
  let output = 0;
  let cacheRead = 0;
  let cacheWrite = 0;
  let context = 0;
  if (measured && recorded) {
    input = recorded.input;
    output = recorded.output;
    cacheRead = recorded.cacheRead;
    cacheWrite = recorded.cacheWrite;
    context = recorded.lastPrompt;
  } else {
    for (const message of messages) {
      const tokens = estimateTokens(`${message.content}\n${message.reasoning}`) + (message.image ? 1200 : 0);
      if (message.role === "user") input += tokens;
      else output += tokens;
    }
    const system = estimateTokens(composeAgentSystem({
      prefix: store.prefix,
      instruction: store.instruction,
      scene: scene(),
      skipPaid: store.skipPaid,
      bypass: store.permission === "bypass",
    }));
    const recent = messages.slice(-Math.max(1, store.maxRequests) * 2);
    context = system + recent.reduce((sum, message) => sum + estimateTokens(`${message.content}\n${message.reasoning}`) + (message.image ? 1200 : 0), 0);
  }
  const fraction = windowSize > 0 ? Math.min(1, context / windowSize) : 0;
  const percentValue = fraction * 100;
  const percent = messages.length
    ? `${percentValue >= 10 ? Math.round(percentValue) : Math.max(percentValue > 0 ? 0.1 : 0, Number(percentValue.toFixed(1)))}%`
    : "—";
  return {
    empty: messages.length === 0,
    measured,
    percent,
    context,
    window: windowSize,
    left: Math.max(0, windowSize - context),
    input,
    output,
    cacheRead,
    cacheWrite,
    total: input + output,
    user: messages.filter((message) => message.role === "user").length,
    assistant: assistantTurns,
  };
});

const historyItems = computed(() => {
  const query = historyQuery.value.trim().toLowerCase();
  return (store.active?.messages ?? []).filter((message) => {
    const text = `${message.content} ${message.reasoning}`.toLowerCase();
    return !query || text.includes(query);
  });
});

const tail = computed(() => {
  const messages = store.active?.messages;
  const last = messages?.length ? messages[messages.length - 1] : undefined;
  return last ? `${last.content}\n${last.reasoning}` : "";
});

watch(tail, scrollToEnd);
watch(() => store.activeId, scrollToEnd);
watch([tab, historyOpen], () => {
  if (tab.value === "chat" && !historyOpen.value) scrollToEnd();
});

watch(tagKey, () => {
  const next: Record<string, boolean> = {};
  for (const tag of tags.value) next[tag.key] = true;
  picked.value = next;
  wrote.value = "";
});

watch(() => store.busy, (busy, was) => {
  if (busy || !was || store.permission !== "bypass" || !tags.value.length) return;
  const key = `${store.activeId}|${tagKey.value}|${dest.value}`;
  if (key === applied.value) return;
  writeSelected(true);
});

function scene() {
  const params = app.params;
  const characters = app.characters
    .map((item, index) => `${index + 1}. ${item.enabled ? "" : "（关闭）"}${item.prompt.trim() || "（空）"}`)
    .join("\n");
  return [
    `模型：${params.model}`,
    `质量词预设：${params.qualityPreset}`,
    `尺寸：${params.width}×${params.height}`,
    `基础提示词：${params.positivePrompt.trim() || "（空）"}`,
    `负面：${params.negativePrompt.trim() || "（空）"}`,
    `角色：\n${characters || "（无）"}`,
    `参考图：${referenceCount.value}`,
    variantBase.value
      ? `变体底图：用户选了一张自己的图当底图。后续变体要沿用图里的构图和姿势，只改用户点名的部分。${variantNote.value}`
      : "",
  ].filter(Boolean).join("\n");
}

function html(content: string) {
  return renderMarkdown(agentVisibleText(content));
}

function toggle(name: Pop) {
  pop.value = pop.value === name ? "" : name;
  armedChatId.value = "";
  renaming.value = false;
}

function fill(prompt: string) {
  draft.value = prompt;
  tab.value = "chat";
  historyOpen.value = false;
  nextTick(() => box.value?.focus());
}

function charLabel(index: number) {
  const item = app.characters[index];
  const head = item?.prompt.split(",")[0]?.trim().slice(0, 18);
  return head ? `Character ${index + 1} · ${head}` : `Character ${index + 1}`;
}

function destLabel(value: string) {
  if (value === "main:positive") return "主提示词";
  if (value === "main:negative") return "负面提示词";
  const [, id, field] = value.split(":");
  const index = app.characters.findIndex((item) => item.id === id);
  const name = index >= 0 ? charLabel(index) : "Character";
  return field === "negative" ? `${name} · 负面` : name;
}

function promptField(target: string) {
  if (target === "main:positive") {
    return {
      get: () => app.params.positivePrompt,
      set: (value: string) => { app.params.positivePrompt = value; },
    };
  }
  if (target === "main:negative") {
    return {
      get: () => app.params.negativePrompt,
      set: (value: string) => { app.params.negativePrompt = value; },
    };
  }
  const [, id, field] = target.split(":");
  const character = app.characters.find((item) => item.id === id);
  if (!character) return null;
  const key = field === "negative" ? "negativePrompt" : "prompt";
  return {
    get: () => character[key],
    set: (value: string) => app.updateCharacter(id, { [key]: value }),
  };
}

function closeReviewIfEmpty() {
  if (edits.value.length) {
    if (!edits.value.some((edit) => edit.id === reviewId.value)) reviewId.value = edits.value[0].id;
    return;
  }
  editListOpen.value = false;
  reviewOpen.value = false;
  reviewId.value = "";
}

function undoEdit(id: string) {
  const edit = edits.value.find((item) => item.id === id);
  if (!edit) return;
  const field = promptField(edit.target);
  if (field) field.set(edit.before);
  edits.value = edits.value.filter((item) => item.id !== id);
  closeReviewIfEmpty();
  app.status = `已撤销${edit.label}`;
}

function keepEdit(id: string) {
  const edit = edits.value.find((item) => item.id === id);
  edits.value = edits.value.filter((item) => item.id !== id);
  closeReviewIfEmpty();
  if (edit) app.status = `已保留${edit.label}`;
}

function undoAll() {
  for (const edit of [...edits.value]) {
    const field = promptField(edit.target);
    if (field) field.set(edit.before);
  }
  edits.value = [];
  closeReviewIfEmpty();
  app.status = "已撤销全部标签改动";
}

function keepAll() {
  edits.value = [];
  closeReviewIfEmpty();
  app.status = "已保留全部标签改动";
}

function openReview(id = reviewId.value || edits.value[0]?.id || "") {
  if (!id) return;
  reviewId.value = id;
  reviewOpen.value = true;
  editListOpen.value = false;
}

const activeEdit = computed(() => edits.value.find((edit) => edit.id === reviewId.value) ?? edits.value[0]);
const activeDiff = computed(() => {
  const edit = activeEdit.value;
  if (!edit) return { removed: [] as string[], added: [] as string[], before: [] as string[], after: [] as string[] };
  const before = splitPromptTags(edit.before);
  const after = splitPromptTags(edit.after);
  const beforeSet = new Set(before);
  const afterSet = new Set(after);
  return {
    before,
    after,
    removed: before.filter((tag) => !afterSet.has(tag)),
    added: after.filter((tag) => !beforeSet.has(tag)),
  };
});

function writeSelected(auto = false) {
  const chosen = tags.value.filter((tag) => picked.value[tag.key] !== false).map((tag) => tag.text);
  if (!chosen.length) return;
  const text = chosen.join(", ");
  const replace = !auto && writeMode.value === "replace";
  const place = (current: string) => (replace ? text : appendPromptChunk(current, text));
  const target = dest.value;
  const field = promptField(target);
  if (!field) return;
  const current = field.get();
  const after = place(current);
  if (after === current) return;
  field.set(after);
  const existing = edits.value.find((edit) => edit.target === target);
  if (existing && after === existing.before) {
    edits.value = edits.value.filter((edit) => edit.id !== existing.id);
    closeReviewIfEmpty();
  } else if (existing) existing.after = after;
  else {
    edits.value = [...edits.value, {
      id: crypto.randomUUID(),
      target,
      label: destLabel(target),
      before: current,
      after,
    }];
  }
  reviewId.value = edits.value.find((edit) => edit.target === target)?.id || reviewId.value;
  applied.value = `${store.activeId}|${tagKey.value}|${target}`;
  wrote.value = destLabel(target);
  app.status = `NAI Agent 已写入${wrote.value}，可对比后保留或撤销`;
}

async function submit(text = draft.value) {
  const next = text.trim();
  if (!next || store.busy) return;
  if (!store.modelId) {
    pop.value = "model";
    return;
  }
  const seen = (store.active?.messages ?? []).some((message) => message.image && message.image === variantBase.value);
  const image = pendingImage.value || (seen ? "" : variantBase.value);
  draft.value = "";
  pendingImage.value = "";
  pop.value = "";
  tab.value = "chat";
  historyOpen.value = false;
  await store.send(next, scene(), image);
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Tab" && suggestion.value && !event.shiftKey) {
    event.preventDefault();
    draft.value = suggestion.value;
    return;
  }
  if (event.isComposing) return;
  const sendKey = store.enterToSend
    ? event.key === "Enter" && !event.shiftKey
    : event.key === "Enter" && (event.ctrlKey || event.metaKey);
  if (!sendKey) return;
  event.preventDefault();
  void submit();
}

function useQuick(item: AgentQuick) {
  if (item.send) void submit(item.prompt);
  else fill(item.prompt);
}

function insertMention(token: string) {
  const prefix = draft.value && !/\s$/.test(draft.value) ? " " : "";
  draft.value = `${draft.value}${prefix}${token} `;
  pop.value = "";
  box.value?.focus();
}

function useCanvas() {
  const url = app.previewUrl || app.i2iImage;
  if (!url) {
    store.error = "画布上还没有图片";
    pop.value = "";
    return;
  }
  pendingImage.value = url;
  pop.value = "";
}

async function pasteImage() {
  pop.value = "";
  try {
    const items = await navigator.clipboard.read();
    for (const item of items) {
      const type = item.types.find((name) => name.startsWith("image/"));
      if (!type) continue;
      const blob = await item.getType(type);
      pendingImage.value = await blobToDataUrl(blob);
      return;
    }
    store.error = "剪贴板里没有图片";
  } catch {
    store.error = "读取剪贴板失败";
  }
}

const recentHistory = computed(() =>
  [...app.history].sort((a, b) => (a.createdAt < b.createdAt ? 1 : -1)).slice(0, 30),
);

function applyDraftImage(url: string, note: string) {
  app.useImageAs("i2i", url);
  app.previewUrl = url;
  variantBase.value = url;
  variantNote.value = note;
  app.status = "已设为变体底图，会参考这张图的构图和姿势";
}

function onDraftFile(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = "";
  if (!file) return;
  const reader = new FileReader();
  reader.onload = () => {
    const url = String(reader.result || "");
    if (!url) return;
    applyDraftImage(url, "来自本地文件。");
  };
  reader.readAsDataURL(file);
}

async function openHistoryPick() {
  draftMenu.value = false;
  historyPick.value = true;
  const updates: Record<string, string> = {};
  for (const item of recentHistory.value) {
    if (historyThumbs.value[item.id]) continue;
    try {
      updates[item.id] = await readImageDataUrl(item.path);
    } catch {
      /* 单张读失败就跳过 */
    }
  }
  if (Object.keys(updates).length) historyThumbs.value = { ...historyThumbs.value, ...updates };
}

async function useHistoryItem(item: HistoryItem) {
  let url = historyThumbs.value[item.id];
  if (!url) {
    try {
      url = await readImageDataUrl(item.path);
    } catch {
      store.error = "这张生图记录读不出来";
      return;
    }
  }
  const prompt = item.prompt.replace(/\s+/g, " ").trim().slice(0, 180);
  applyDraftImage(url, prompt ? `来自生图记录。原提示词：${prompt}` : "来自生图记录。");
  historyPick.value = false;
}

function onFile(event: Event) {
  const file = (event.target as HTMLInputElement).files?.[0];
  if (!file) return;
  const reader = new FileReader();
  reader.onload = () => {
    pendingImage.value = String(reader.result || "");
  };
  reader.readAsDataURL(file);
  pop.value = "";
}

function blobToDataUrl(blob: Blob) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result || ""));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(blob);
  });
}

function startRename() {
  renaming.value = true;
  renameValue.value = store.active?.title || "";
}

function removeChatById(id: string) {
  if (store.busy || !id) return;
  if (armedChatId.value !== id) {
    armedChatId.value = id;
    return;
  }
  store.removeChat(id);
  armedChatId.value = "";
  pop.value = "";
}

function saveRename() {
  if (store.active) store.renameChat(store.active.id, renameValue.value);
  renaming.value = false;
  pop.value = "";
}

function openQuickEdit() {
  pop.value = "quick";
}

const modeLabel = computed(() => (store.permission === "bypass" ? "完全权限" : "自动"));
const modelLabel = computed(() => store.modelId || "未选择模型");
const thinkOpen = ref<Record<string, boolean>>({});

function thinkingShown(id: string, index: number) {
  if (id in thinkOpen.value) return thinkOpen.value[id];
  const messages = store.active?.messages ?? [];
  return store.busy && index === messages.length - 1;
}

function toggleThink(id: string, index: number) {
  thinkOpen.value = { ...thinkOpen.value, [id]: !thinkingShown(id, index) };
}
</script>

<template>
  <aside class="agent" @pointerdown="pop = pop === 'quick' ? pop : ''; editListOpen = false; draftMenu = false">
    <header @pointerdown.stop>
      <NaiAgentMark :expanded="true" :button="false" />
      <button type="button" class="who" :class="{ on: pop === 'chats' }" :title="store.active?.title || '新聊天'" @click="toggle('chats')">
        <small>工作区 1 / {{ number }}</small>
        <b><span>{{ store.active?.title || "新聊天" }}</span><i>▾</i></b>
      </button>
      <div v-if="pop === 'chats'" class="pop chats">
        <div v-for="chat in store.chats" :key="chat.id" class="chat-row">
          <button type="button" :class="{ on: chat.id === store.activeId }" :title="chat.title" @click="store.selectChat(chat.id); pop = ''">
            <b>{{ chat.title }}</b>
          </button>
          <button type="button" class="chat-delete" :title="armedChatId === chat.id ? '再点一次删除' : '删除这个聊天'" :disabled="store.busy" @click.stop="removeChatById(chat.id)">
            <NaiIcon name="trash" :size="13" />
          </button>
        </div>
      </div>
      <button type="button" title="新聊天" :disabled="store.busy" @click="store.newChat()">
        <NaiIcon name="plus" :size="15" />
      </button>
      <button type="button" class="icon-danger" :class="{ on: armedChatId === store.activeId }" :title="armedChatId === store.activeId ? '再点一次删除' : '删除这个聊天'" :disabled="store.busy" @click="removeChatById(store.activeId)">
        <NaiIcon name="trash" :size="15" />
      </button>
      <button type="button" title="更多" :class="{ on: pop === 'more' }" @click="toggle('more')">
        <NaiIcon name="dots" :size="15" />
      </button>
      <div v-if="pop === 'more'" class="pop more">
        <form v-if="renaming" @submit.prevent="saveRename">
          <input v-model="renameValue" @keydown.esc="renaming = false" />
          <button type="submit">保存</button>
        </form>
        <template v-else>
          <button type="button" @click="startRename">重命名聊天</button>
          <RouterLink to="/settings?tab=agent" @click="pop = ''">Agent 设置</RouterLink>
          <button type="button" class="danger" @click="removeChatById(store.activeId)">
            {{ armedChatId === store.activeId ? "再点一次删除" : "删除对话" }}
          </button>
        </template>
      </div>
      <button type="button" title="收起" @click="emit('close')">
        <NaiIcon name="close" :size="15" />
      </button>
    </header>

    <nav>
      <button type="button" class="tab" :class="{ on: tab === 'chat' && !historyOpen }" @click="tab = 'chat'; historyOpen = false">对话</button>
      <button type="button" class="tab" :class="{ on: tab === 'shelf' }" @click="tab = 'shelf'; historyOpen = false">
        产物<small v-if="shelfImages.length">{{ shelfImages.length }}</small>
      </button>
      <button type="button" class="tab" :class="{ on: tab === 'memory' }" @click="tab = 'memory'; historyOpen = false">指令</button>
      <span class="grow" />
      <div class="nav-tools">
        <button type="button" class="history" :class="{ on: historyOpen }" title="对话历史" @click="historyOpen = !historyOpen; tab = 'chat'">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.7"><circle cx="12" cy="12" r="8" /><path d="M12 8v5l3 2" /></svg>
          历史
        </button>
        <button type="button" class="usage" :class="{ on: pop === 'usage' }" title="上下文与用量" @pointerdown.stop @click="toggle('usage')">{{ usageView.percent }}</button>
      </div>
      <div v-if="pop === 'usage'" class="pop usage-pop" @pointerdown.stop>
        <b>上下文与用量</b>
        <template v-if="usageView.empty">
          <p>这个聊天还没有发言。</p>
        </template>
        <template v-else>
          <p class="usage-kicker">当前上下文 · {{ usageView.measured ? "模型返回" : "按字数估算" }}</p>
          <div class="usage-head">
            <strong>{{ usageView.percent }}</strong>
            <span>{{ formatTokens(usageView.context) }} / {{ formatTokens(usageView.window) }}</span>
          </div>
          <div class="usage-meter"><i :style="{ width: usageView.percent }" /></div>
          <p class="usage-remain">剩余 {{ formatTokens(usageView.left) }}</p>
          <p class="usage-kicker">本聊天累计</p>
          <div class="usage-row"><span>输入</span><b>{{ formatTokens(usageView.input) }}</b></div>
          <div class="usage-row"><span>输出</span><b>{{ formatTokens(usageView.output) }}</b></div>
          <div v-if="usageView.cacheRead" class="usage-row"><span>缓存读取</span><b>{{ formatTokens(usageView.cacheRead) }}</b></div>
          <div class="usage-row"><span>累计 tokens</span><b>{{ formatTokens(usageView.total) }}</b></div>
          <div class="usage-row"><span>消息</span><b>{{ usageView.user }} / {{ usageView.assistant }}</b></div>
          <p class="usage-note">问过话就会记上这些 token。费用以 DeepSeek 账单为准。</p>
        </template>
      </div>
    </nav>

    <div ref="scroller" class="body">
      <div v-if="historyOpen" class="history-view">
        <input v-model="historyQuery" placeholder="搜索发言和卡片…" />
        <p v-if="!historyItems.length" class="empty-copy">没有匹配的条目</p>
        <button v-for="message in historyItems" :key="message.id" type="button" class="hist" @click="historyOpen = false">
          <b>{{ message.role === "user" ? "我的发言" : "Agent" }}</b>
          <span>{{ agentVisibleText(message.content) || message.reasoning || "（空）" }}</span>
        </button>
      </div>

      <div v-else-if="tab === 'shelf'" class="shelf">
        <template v-if="shelfImages.length">
          <figure v-for="image in shelfImages" :key="image.id">
            <img :src="image.url" alt="" />
            <figcaption>{{ image.label }}</figcaption>
          </figure>
        </template>
        <div v-else class="empty-block">
          <NaiIcon name="image" :size="22" />
          <b>这个聊天还没有产物</b>
          <p>Agent 生成的图、你附上的图和它写的文件都会收在这里。</p>
        </div>
      </div>

      <div v-else-if="tab === 'memory'" class="memory">
        <p class="hint">这些内容每次都会交给 Agent。顺序是内置提示词前、内置系统提示词、全局指令。改动从下一条消息开始生效。</p>
        <section>
          <header><b>内置提示词前</b></header>
          <textarea v-model="store.prefix" rows="4" placeholder="还没写" />
        </section>
        <section>
          <header><b>全局指令</b></header>
          <textarea v-model="store.instruction" rows="8" placeholder="还没写" />
        </section>
        <RouterLink class="settings-link" to="/settings?tab=agent">打开完整设置</RouterLink>
      </div>

      <template v-else-if="!store.active?.messages.length">
        <h2>{{ greet }}</h2>
        <p class="lead">Agent 会先看草稿再动手；花 Anlas 前一定先问你。</p>
        <article class="draft">
          <button type="button" class="thumb" title="更换草稿图" @pointerdown.stop @click="draftMenu = !draftMenu">
            <img v-if="thumb" :src="thumb" alt="" />
            <NaiIcon v-else name="image" :size="18" />
            <span class="swap">更换</span>
          </button>
          <div v-if="draftMenu" class="pop draft-menu" @pointerdown.stop>
            <button type="button" @click="openHistoryPick">从生图记录</button>
            <button type="button" @click="draftMenu = false; draftPick?.click()">从文件选择</button>
          </div>
          <input ref="draftPick" type="file" accept="image/*" hidden @change="onDraftFile" />
          <div>
            <small>当前草稿</small>
            <b>{{ app.params.model }} · {{ app.params.width }}×{{ app.params.height }}</b>
            <em v-if="promptLine">{{ promptLine }}</em>
            <span v-if="variantBase">变体底图 · 参考构图和姿势</span>
            <span v-else>角色 {{ app.characters.length }} · 0 个积木 · 参考图 {{ referenceCount }}</span>
          </div>
        </article>
        <div class="quick-head">
          <small>快捷信息</small>
          <button type="button" title="编辑快捷信息" @click="openQuickEdit">
            <NaiIcon name="sliders" :size="14" />
          </button>
        </div>
        <div v-if="welcomeQuick.length" class="tiles">
          <button v-for="item in welcomeQuick" :key="item.id" type="button" @click="useQuick(item)">
            <span class="well"><NaiIcon :name="item.glyph" :size="16" /></span>
            <b>{{ item.title }}</b>
            <em>{{ item.hint }}</em>
            <small v-if="item.send">直接发送</small>
          </button>
        </div>
      </template>

      <template v-else>
        <article v-for="(message, index) in store.active?.messages" :key="message.id" class="msg" :class="message.role">
          <img v-if="message.image" :src="message.image" alt="" />
          <div v-if="message.reasoning" class="think">
            <button type="button" @click="toggleThink(message.id, index)">
              <svg viewBox="0 0 16 16" width="12" height="12" :class="{ open: thinkingShown(message.id, index) }"><path d="M6 4l4 4-4 4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" /></svg>
              {{ store.busy && index === (store.active?.messages.length ?? 0) - 1 ? "思考中" : "已思考" }}
            </button>
            <p v-if="thinkingShown(message.id, index)">{{ message.reasoning }}</p>
          </div>
          <div v-if="agentVisibleText(message.content)" class="md" v-html="html(message.content)" />
          <p v-else-if="message.role === 'assistant' && store.busy && !message.reasoning" class="wait">思考中…</p>
        </article>
        <section v-if="tags.length" class="tags">
          <header>
            <b>得到的标签</b>
            <span v-if="wrote">已写入{{ wrote }}</span>
          </header>
          <div v-if="positiveTags.length" class="group">
            <small>正向</small>
            <div class="chips">
              <button
                v-for="tag in positiveTags"
                :key="tag.key"
                type="button"
                class="chip"
                :class="{ on: picked[tag.key] !== false }"
                @click="picked[tag.key] = picked[tag.key] === false"
              >{{ tag.text }}</button>
            </div>
          </div>
          <div v-if="negativeTags.length" class="group">
            <small>负向</small>
            <div class="chips">
              <button
                v-for="tag in negativeTags"
                :key="tag.key"
                type="button"
                class="chip"
                :class="{ on: picked[tag.key] !== false }"
                @click="picked[tag.key] = picked[tag.key] === false"
              >{{ tag.text }}</button>
            </div>
          </div>
          <div class="tag-actions">
            <select v-model="dest">
              <optgroup label="主提示词">
                <option value="main:positive">主提示词</option>
                <option value="main:negative">负面提示词</option>
              </optgroup>
              <optgroup v-if="app.characters.length" label="Character">
                <option v-for="(character, index) in app.characters" :key="`${character.id}:prompt`" :value="`char:${character.id}:prompt`">
                  {{ charLabel(index) }}
                </option>
                <option v-for="(character, index) in app.characters" :key="`${character.id}:negative`" :value="`char:${character.id}:negative`">
                  {{ charLabel(index) }} · 负面
                </option>
              </optgroup>
            </select>
            <select v-model="writeMode">
              <option value="append">插入末尾</option>
              <option value="replace">替换整段</option>
            </select>
            <button type="button" @click="writeSelected()">写入所选</button>
          </div>
        </section>
      </template>
      <p v-if="store.error" class="err">{{ store.error }}</p>
      <p v-if="store.loaded && !store.hasKey" class="err">
        还没有 API Key。
        <RouterLink to="/deepseek">去 DeepSeek 页填写</RouterLink>
      </p>
    </div>

    <div v-if="edits.length" class="review-dock" @pointerdown.stop>
      <div v-if="reviewOpen && activeEdit" class="review-diff">
        <header>
          <b>{{ activeEdit.label }}</b>
          <span>新增 {{ activeDiff.added.length }} · 删除 {{ activeDiff.removed.length }}</span>
          <button type="button" @click="reviewOpen = false">关闭</button>
        </header>
        <p>改之前</p>
        <div class="diff-line">
          <span v-if="!activeDiff.before.length" class="diff-empty">（空）</span>
          <span v-for="(tag, index) in activeDiff.before" :key="`b-${index}`" class="diff-chip" :class="{ removed: activeDiff.removed.includes(tag) }">{{ tag }}</span>
        </div>
        <p>改之后</p>
        <div class="diff-line">
          <span v-if="!activeDiff.after.length" class="diff-empty">（空）</span>
          <span v-for="(tag, index) in activeDiff.after" :key="`a-${index}`" class="diff-chip" :class="{ added: activeDiff.added.includes(tag) }">{{ tag }}</span>
        </div>
        <div class="review-one">
          <button type="button" @click="undoEdit(activeEdit.id)">Undo</button>
          <button type="button" class="keep" @click="keepEdit(activeEdit.id)">Keep</button>
        </div>
      </div>
      <div class="review-bar">
        <button type="button" class="review-count" @click="editListOpen = !editListOpen">
          <svg viewBox="0 0 16 16" width="12" height="12" :class="{ open: editListOpen }"><path d="M6 4l4 4-4 4" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" /></svg>
          {{ edits.length }} 处改动
        </button>
        <span class="grow" />
        <button type="button" @click="undoAll">Undo All</button>
        <button type="button" @click="keepAll">Keep All</button>
        <button type="button" class="review-go" @click="openReview()">Review</button>
      </div>
      <div v-if="editListOpen" class="review-list">
        <div v-for="edit in edits" :key="edit.id" class="review-row">
          <button type="button" class="review-name" @click="openReview(edit.id)">{{ edit.label }}</button>
          <button type="button" title="Undo" @click="undoEdit(edit.id)">✕</button>
          <button type="button" title="Keep" @click="keepEdit(edit.id)">✓</button>
        </div>
      </div>
    </div>

    <form v-if="tab === 'chat' && !historyOpen" class="composer" @submit.prevent="submit()" @pointerdown.stop>
      <img v-if="pendingImage" class="pending" :src="pendingImage" alt="" />
      <button v-if="suggestion" type="button" class="suggest" @click="draft = suggestion">
        <small>Tab</small>{{ suggestion }}
      </button>
      <textarea
        ref="box"
        v-model="draft"
        rows="2"
        :placeholder="store.enterToSend ? '告诉 Agent 要做什么 · @ 引用 · / 技能' : 'Enter 换行 · Ctrl+Enter 发送'"
        @keydown="onKeydown"
      />
      <p v-if="!store.modelId" class="warn">先选一个能用工具的模型</p>
      <div class="bar">
        <button type="button" title="添加图片" :class="{ on: pop === 'attach' }" @click="toggle('attach')">
          <NaiIcon name="clip" :size="14" />
        </button>
        <div v-if="pop === 'attach'" class="pop up">
          <button type="button" @click="filePick?.click()">从文件选择…</button>
          <button type="button" @click="useCanvas">附上画布当前图片</button>
          <button type="button" @click="pasteImage">粘贴剪贴板图片</button>
        </div>
        <input ref="filePick" type="file" accept="image/*" hidden @change="onFile" />
        <button type="button" title="引用" :class="{ on: pop === 'mention' }" @click="toggle('mention')">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="1.8"><circle cx="12" cy="12" r="4" /><path d="M16 12v1.5a2.5 2.5 0 0 0 5 0V12a9 9 0 1 0-3.5 7.1" /></svg>
        </button>
        <div v-if="pop === 'mention'" class="pop up">
          <button type="button" @click="insertMention('@草稿')">草稿</button>
          <button v-if="thumb" type="button" @click="insertMention('@图片')">图片</button>
          <button type="button" @click="insertMention('@角色')">角色</button>
        </div>
        <button type="button" class="pill" :class="{ warn: store.permission === 'bypass', on: pop === 'mode' }" @click="toggle('mode')">
          {{ modeLabel }} <i>▾</i>
        </button>
        <div v-if="pop === 'mode'" class="pop up mode">
          <button type="button" @click="store.permission = 'auto'; pop = ''">
            <b>自动</b><span>工作区内的常规修改直接执行；花钱仍要确认</span>
          </button>
          <button type="button" @click="store.permission = 'bypass'; pop = ''">
            <b>完全权限</b><span>免费修改直接写入；付费生成仍会先问你</span>
          </button>
        </div>
        <span class="grow" />
        <button type="button" class="pill model" :class="{ on: pop === 'model', missing: !store.modelId }" @click="toggle('model')">
          {{ modelLabel }} <i>▾</i>
        </button>
        <div v-if="pop === 'model'" class="pop up model-pop">
          <button type="button" @click="store.modelId = ''; pop = ''">未选择模型</button>
          <button v-for="id in store.models" :key="id" type="button" :class="{ on: id === store.modelId }" @click="store.modelId = id; pop = ''">{{ id }}</button>
        </div>
        <button v-if="store.busy" type="button" class="send" @click="store.stop()">停止</button>
        <button v-else class="send" type="submit" :disabled="!draft.trim()">发送</button>
      </div>
    </form>

    <div v-if="historyPick" class="sheet" @pointerdown.stop>
      <header>
        <b>选一张生过的图</b>
        <button type="button" @click="historyPick = false">关闭</button>
      </header>
      <div class="sheet-body">
        <p class="hint">作为后续变体的底图。Agent 会看构图和姿势，生成时也用它做图生图。</p>
        <p v-if="!recentHistory.length" class="hint">还没有生图记录。</p>
        <div v-else class="record-grid">
          <button v-for="item in recentHistory" :key="item.id" type="button" :title="item.prompt" @click="useHistoryItem(item)">
            <img v-if="historyThumbs[item.id]" :src="historyThumbs[item.id]" alt="" />
            <span>{{ item.width }}×{{ item.height }}</span>
          </button>
        </div>
      </div>
    </div>

    <div v-if="pop === 'quick'" class="sheet" @pointerdown.stop>
      <header>
        <b>快捷信息</b>
        <button type="button" @click="pop = ''">关闭</button>
      </header>
      <div class="sheet-body">
        <NaiAgentSettings part="quick" />
      </div>
    </div>
  </aside>
</template>

<style scoped>
.agent {
  position: relative;
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background: #12142a;
  border-left: 1px solid rgba(255, 255, 255, 0.08);
  color: #fff;
}
header {
  height: 46px;
  flex: none;
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 0 8px 0 12px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}
header > button.who {
  width: auto;
  height: auto;
  min-width: 0;
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  justify-content: center;
  padding: 3px 6px;
  border: 0;
  border-radius: 7px;
  background: transparent;
  text-align: left;
  overflow: hidden;
}
.who.on, .who:hover { background: rgba(255, 255, 255, 0.06); }
.who small, .draft small, .quick-head small {
  color: rgba(255, 255, 255, 0.42);
  font-size: 10px;
  letter-spacing: 0.08em;
}
.who b {
  display: flex;
  align-items: center;
  gap: 4px;
  width: 100%;
  min-width: 0;
  color: #fff;
  font-size: 13px;
  font-weight: 600;
}
.who b span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.who b i { flex: none; }
header > button.icon-danger { color: #ff8d8d; }
header > button.icon-danger.on { background: rgba(255, 141, 141, 0.16); }
.chat-row { display: flex; align-items: center; gap: 2px; }
.chat-row > button:first-child { flex: 1; min-width: 0; width: auto; }
.chat-row b {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-weight: 500;
}
.chat-delete {
  width: 28px !important;
  flex: none;
  align-items: center !important;
  justify-content: center;
  color: #ff8d8d !important;
}
.who i, .pill i { font-style: normal; color: rgba(255, 255, 255, 0.45); }
header > button {
  width: 28px;
  height: 28px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: rgba(255, 255, 255, 0.78);
  display: grid;
  place-items: center;
}
header > button.on, header > button:hover { background: rgba(255, 255, 255, 0.06); color: #fff; }
header > button:disabled { opacity: 0.4; }
nav {
  position: relative;
  height: 36px;
  flex: none;
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 0 10px 0 8px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}
nav .tab {
  position: relative;
  height: 100%;
  border: 0;
  background: none;
  color: rgba(255, 255, 255, 0.45);
  font-size: 13px;
  padding: 0 8px;
}
nav .tab.on { color: #fff; }
nav .tab.on::after {
  content: "";
  position: absolute;
  left: 8px;
  right: 8px;
  bottom: 0;
  height: 2px;
  border-radius: 2px 2px 0 0;
  background: #fff;
}
nav small {
  margin-left: 4px;
  color: rgba(255, 255, 255, 0.45);
  font-size: 11px;
}
.nav-tools {
  display: flex;
  align-items: center;
  gap: 2px;
  height: 28px;
}
.history {
  height: 28px;
  padding: 0 8px;
  border: 0;
  border-radius: 7px;
  background: transparent;
  color: rgba(255, 255, 255, 0.62);
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 12px;
  font-weight: 500;
}
.history svg { display: block; flex: none; }
.history.on, .history:hover { background: rgba(255, 255, 255, 0.06); color: #fff; }
.usage {
  min-width: 28px;
  height: 28px;
  padding: 0 7px;
  border: 1px solid rgba(255, 255, 255, 0.16);
  border-radius: 999px;
  background: transparent;
  color: rgba(255, 255, 255, 0.72);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  line-height: 1;
  display: grid;
  place-items: center;
}
.usage.on, .usage:hover { border-color: rgba(255, 255, 255, 0.32); color: #fff; }
.grow { flex: 1; }
.body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 22px 16px;
}
h2 { margin: 0; font-size: 18px; font-weight: 600; color: #fff; }
.lead, .hint, .empty-block p { margin: 4px 0 0; color: rgba(255, 255, 255, 0.55); font-size: 12px; line-height: 1.5; }
.draft {
  position: relative;
  display: flex;
  gap: 12px;
  margin-top: 18px;
  padding: 10px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  background: #1a1e38;
}
.draft-menu { top: 96px; left: 10px; }
.record-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px; margin-top: 12px; }
.record-grid button {
  position: relative;
  aspect-ratio: 2 / 3;
  padding: 0;
  border: 0;
  border-radius: 8px;
  overflow: hidden;
  background: #12142a;
}
.record-grid img { width: 100%; height: 100%; object-fit: cover; }
.record-grid span {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  padding: 3px 4px;
  background: rgba(0, 0, 0, 0.55);
  color: #fff;
  font-size: 10px;
  white-space: nowrap;
}
.thumb {
  position: relative;
  width: 54px;
  height: 80px;
  flex: none;
  padding: 0;
  border: 0;
  border-radius: 8px;
  background: #12142a;
  display: grid;
  place-items: center;
  overflow: hidden;
  color: rgba(255, 255, 255, 0.35);
  cursor: pointer;
}
.thumb .swap {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  height: 18px;
  display: grid;
  place-items: center;
  background: rgba(0, 0, 0, 0.55);
  color: #fff;
  font-size: 10px;
  line-height: 1;
}
.thumb img, .msg img, .pending, .shelf img { width: 100%; height: 100%; object-fit: cover; }
.draft div { min-width: 0; display: flex; flex-direction: column; gap: 4px; }
.draft b, .tiles b { font-size: 13px; font-weight: 500; color: #fff; }
.draft em, .draft span, .tiles em { font-style: normal; color: rgba(255, 255, 255, 0.55); font-size: 12px; }
.draft em, .draft span { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.draft span { color: rgba(255, 255, 255, 0.38); font-size: 11px; }
.quick-head { display: flex; align-items: center; margin: 18px 0 8px; }
.quick-head button, .sheet header button, .sheet footer button {
  margin-left: auto;
  width: 28px;
  height: 28px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: rgba(255, 255, 255, 0.7);
}
.tiles { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
.tiles button {
  min-height: 124px;
  padding: 14px 14px 12px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 16px;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.045), rgba(255, 255, 255, 0.01)),
    #1a1e38;
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.05);
  text-align: left;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 6px;
  transition: border-color 0.15s ease, transform 0.15s ease, background 0.15s ease;
}
.tiles button:hover {
  border-color: rgba(245, 243, 194, 0.45);
  background: #202544;
  transform: translateY(-1px);
}
.tiles b { font-size: 14px; font-weight: 650; letter-spacing: 0.01em; }
.tiles em {
  color: rgba(255, 255, 255, 0.5);
  font-size: 12px;
  font-style: normal;
  line-height: 1.45;
}
.tiles small { margin-top: auto; color: #e39b4a; font-size: 10px; letter-spacing: 0.04em; }
.well {
  width: 34px;
  height: 34px;
  margin-bottom: 4px;
  border-radius: 11px;
  display: grid;
  place-items: center;
}
.tiles button:nth-child(4n + 1) .well { background: rgba(94, 168, 255, 0.16); color: #9ec4ff; }
.tiles button:nth-child(4n + 2) .well { background: rgba(245, 243, 194, 0.14); color: #f5f3c2; }
.tiles button:nth-child(4n + 3) .well { background: rgba(186, 154, 255, 0.16); color: #d4c4ff; }
.tiles button:nth-child(4n) .well { background: rgba(120, 210, 170, 0.14); color: #b7ecd2; }
.msg { margin: 0 0 18px; }
.msg.user {
  margin-left: 28px;
  padding: 8px 10px;
  border-radius: 10px;
  background: rgba(255, 255, 255, 0.05);
}
.msg img { width: 120px; height: 80px; object-fit: cover; border-radius: 8px; margin-bottom: 8px; }
.think { margin: 0 0 8px; }
.think button {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  height: 22px;
  padding: 0;
  border: 0;
  background: transparent;
  color: rgba(255, 255, 255, 0.45);
  font-size: 12px;
  font-weight: 500;
}
.think button:hover { color: rgba(255, 255, 255, 0.78); }
.think svg { transition: transform 0.15s ease; }
.think svg.open { transform: rotate(90deg); }
.think p {
  margin: 6px 0 0;
  padding-left: 10px;
  border-left: 1px solid rgba(255, 255, 255, 0.12);
  color: rgba(255, 255, 255, 0.42);
  font-size: 12px;
  line-height: 1.55;
  white-space: pre-wrap;
}
.wait { margin: 0; color: rgba(255, 255, 255, 0.45); font-size: 12px; }
.md :deep(p) { margin: 0 0 8px; font-size: 13px; line-height: 1.6; font-weight: 450; }
.md :deep(p:last-child) { margin-bottom: 0; }
.tags {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-top: 8px;
  padding: 12px;
  border: 1px solid rgba(245, 243, 194, 0.18);
  border-radius: 14px;
  background: rgba(245, 243, 194, 0.06);
}
.tags header { display: flex; align-items: center; justify-content: space-between; height: auto; padding: 0; border: 0; }
.tags header b { font-size: 13px; }
.tags header span, .tags small { color: rgba(255, 255, 255, 0.5); font-size: 11px; }
.group { display: flex; flex-direction: column; gap: 6px; }
.chips { display: flex; flex-wrap: wrap; gap: 6px; }
.chip {
  max-width: 100%;
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 999px;
  background: transparent;
  color: rgba(255, 255, 255, 0.55);
  padding: 4px 9px;
  font-size: 12px;
  font-weight: 500;
}
.chip.on {
  border-color: rgba(245, 243, 194, 0.45);
  background: rgba(245, 243, 194, 0.16);
  color: #f5f3c2;
}
.tag-actions { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; }
.tag-actions select {
  min-width: 0;
  flex: 1;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  background: #12142a;
  color: #fff;
  padding: 6px 8px;
  font-size: 12px;
}
.tag-actions button, .send {
  border: 0;
  border-radius: 999px;
  background: #f5f3c2;
  color: #12142a;
  padding: 6px 12px;
  font-size: 12px;
}
.err { margin: 0; margin-top: 10px; font-size: 12px; color: #ffb4b4; }
.settings-link { color: #f5f3c2; font-size: 12px; }
.suggest {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  width: auto;
  max-width: calc(100% - 16px);
  margin: 8px 8px 0;
  padding: 5px 8px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.04);
  color: rgba(255, 255, 255, 0.78);
  font-size: 12px;
  font-weight: 450;
  line-height: 1.4;
  text-align: left;
}
.suggest small {
  flex: none;
  padding: 1px 5px;
  border-radius: 4px;
  background: rgba(255, 255, 255, 0.08);
  color: #f5f3c2;
  font-size: 10px;
}
.err a { color: #f5f3c2; }
.empty-block, .shelf { color: rgba(255, 255, 255, 0.7); }
.empty-block { min-height: 220px; display: flex; flex-direction: column; align-items: center; justify-content: center; text-align: center; gap: 8px; }
.empty-block b { font-size: 13px; }
.shelf { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; padding: 0; }
.shelf figure { margin: 0; }
.shelf img { height: 120px; border-radius: 8px; }
.shelf figcaption, .hist span { color: rgba(255, 255, 255, 0.45); font-size: 11px; }
.memory section, .history-view input, .composer {
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  background: #1a1e38;
}
.memory section { margin-top: 12px; padding: 12px; }
.memory header { height: auto; padding: 0 0 6px; border: 0; }
.memory header b { font-size: 13px; }
.memory textarea, .composer textarea, .history-view input, .sheet input, .sheet textarea, .more input,
.memory textarea:focus, .composer textarea:focus, .history-view input:focus, .sheet input:focus, .sheet textarea:focus, .more input:focus,
.memory textarea:focus-visible, .composer textarea:focus-visible, .history-view input:focus-visible, .sheet input:focus-visible, .sheet textarea:focus-visible, .more input:focus-visible {
  width: 100%;
  border: 0;
  outline: none;
  box-shadow: none;
  background: transparent;
  resize: vertical;
  color: #fff;
  font-size: 13px;
}
.history-view input { padding: 8px 10px; margin-bottom: 8px; }
.hist {
  width: 100%;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  padding: 8px 4px;
  border: 0;
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  background: transparent;
  text-align: left;
}
.hist span { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 100%; }
.empty-copy { color: rgba(255, 255, 255, 0.45); font-size: 12px; }
.review-dock {
  position: relative;
  margin: 0 12px 8px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  background: #1c203c;
}
.review-diff { max-height: 220px; overflow: auto; padding: 10px 12px 8px; border-bottom: 1px solid rgba(255, 255, 255, 0.08); }
.review-diff header { height: auto; display: flex; align-items: center; gap: 8px; padding: 0 0 8px; border: 0; }
.review-diff header b { font-size: 13px; }
.review-diff header span { color: rgba(255, 255, 255, 0.45); font-size: 11px; }
.review-diff header button { margin-left: auto; width: auto; height: auto; padding: 0; }
.review-diff p { margin: 8px 0 4px; color: rgba(255, 255, 255, 0.45); font-size: 11px; }
.diff-line { display: flex; flex-wrap: wrap; gap: 4px; }
.diff-chip {
  border-radius: 999px;
  padding: 2px 7px;
  background: rgba(255, 255, 255, 0.05);
  color: rgba(255, 255, 255, 0.78);
  font-size: 11px;
}
.diff-chip.removed { color: #ff8d8d; text-decoration: line-through; background: rgba(255, 141, 141, 0.1); }
.diff-chip.added { color: #b7f0c2; background: rgba(80, 180, 120, 0.16); }
.diff-empty { color: rgba(255, 255, 255, 0.35); font-size: 11px; }
.review-one { display: flex; justify-content: flex-end; gap: 6px; margin-top: 10px; }
.review-one .keep { background: rgba(255, 255, 255, 0.12); color: #fff; }
.review-bar { display: flex; align-items: center; gap: 2px; min-height: 36px; padding: 4px 6px; }
.review-bar button, .review-row button, .review-diff header button, .review-one button {
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: rgba(255, 255, 255, 0.78);
  font-size: 12px;
  height: 26px;
  padding: 0 8px;
}
.review-bar button:hover, .review-row button:hover, .review-diff header button:hover, .review-one button:hover { background: rgba(255, 255, 255, 0.06); }
.review-count { display: inline-flex !important; align-items: center; gap: 4px; }
.review-count svg { display: block; transform: rotate(90deg); transition: transform 0.15s ease; }
.review-count svg.open { transform: rotate(-90deg); }
.review-go { background: rgba(255, 255, 255, 0.1) !important; color: #fff !important; }
.review-list {
  position: absolute;
  z-index: 6;
  left: 0;
  right: 0;
  bottom: calc(100% + 6px);
  padding: 4px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 10px;
  background: #1c203c;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
}
.review-row { display: flex; align-items: center; gap: 2px; }
.review-name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; text-align: left; }
.composer { margin: 0 12px 12px; padding: 4px 4px 6px; position: relative; }
.pending { width: 72px; height: 48px; margin: 8px 8px 0; border-radius: 6px; }
.composer textarea,
.composer textarea:focus,
.composer textarea:focus-visible {
  padding: 10px 12px 4px;
  min-height: 44px;
}
.warn { margin: 0 12px 2px; color: #e39b4a; font-size: 11.5px; }
.bar { display: flex; align-items: center; gap: 4px; padding: 6px 6px 2px; position: relative; }
.bar > button {
  width: 28px;
  height: 28px;
  border: 0;
  border-radius: 7px;
  background: transparent;
  color: rgba(255, 255, 255, 0.8);
  display: grid;
  place-items: center;
}
.bar > button.on, .bar > button:hover { background: rgba(255, 255, 255, 0.06); }
.pill {
  width: auto !important;
  height: 26px !important;
  padding: 0 8px;
  border-radius: 999px !important;
  background: #12142a !important;
  font-size: 11.5px;
  display: inline-flex !important;
  gap: 4px;
}
.pill.warn { color: #ff8d8d; }
.pill.missing { color: rgba(255, 255, 255, 0.72); }
.send { width: auto !important; height: 26px !important; padding: 0 12px; }
.send:disabled { opacity: 0.4; }
.pop {
  position: absolute;
  z-index: 5;
  min-width: 180px;
  padding: 6px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 10px;
  background: #1c203c;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
}
.chats { top: 42px; left: 46px; }
.more { top: 42px; right: 40px; }
.usage-pop {
  top: calc(100% + 6px);
  right: 8px;
  width: 248px;
  margin: 0;
  padding: 12px 12px 10px;
  color: rgba(255, 255, 255, 0.72);
  font-size: 12px;
}
.usage-pop > b { display: block; color: #fff; font-size: 13px; font-weight: 600; }
.usage-pop > p { margin: 8px 0 0; line-height: 1.45; }
.usage-kicker {
  margin: 10px 0 6px !important;
  color: rgba(255, 255, 255, 0.42);
  font-size: 11px;
  letter-spacing: 0.04em;
}
.usage-head { display: flex; align-items: baseline; gap: 8px; }
.usage-head strong { color: #fff; font-size: 20px; font-weight: 600; line-height: 1; }
.usage-head span, .usage-remain { color: rgba(255, 255, 255, 0.45); font-size: 11px; }
.usage-meter {
  height: 4px;
  margin-top: 8px;
  border-radius: 2px;
  background: rgba(255, 255, 255, 0.08);
  overflow: hidden;
}
.usage-meter i { display: block; height: 100%; background: #7aa2ff; }
.usage-row {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  padding: 3px 0;
}
.usage-row b { color: #fff; font-weight: 500; font-variant-numeric: tabular-nums; }
.usage-note { color: rgba(255, 255, 255, 0.4) !important; font-size: 11px !important; }
.pop.up { bottom: 42px; left: 8px; }
.model-pop, .mode { right: 8px; left: auto; }
.pop button, .pop a, .more form {
  text-decoration: none;
  width: 100%;
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 2px;
  padding: 7px 8px;
  border: 0;
  border-radius: 7px;
  background: transparent;
  color: #fff;
  text-align: left;
  font-size: 12px;
}
.pop button.on, .pop button:hover, .pop a:hover { background: rgba(255, 255, 255, 0.06); }
.pop button span, .mode span { color: rgba(255, 255, 255, 0.45); font-size: 11px; font-weight: 500; }
.danger { color: #ff8d8d !important; }
.more input { padding: 6px 8px; }
.sheet {
  position: absolute;
  inset: 46px 0 0;
  z-index: 6;
  display: flex;
  flex-direction: column;
  background: #12142a;
}
.sheet header { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 10px 12px; height: auto; border: 0; }
.sheet header button { width: auto; padding: 6px 10px; }
.sheet-body { flex: 1; overflow: auto; padding: 0 12px 16px; }
</style>
