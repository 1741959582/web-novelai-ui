<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { useAppStore } from "@/stores/app";
import { useDeepseekStore, type DsChat } from "@/stores/deepseekChat";
import { renderMarkdown } from "@/utils/markdown";

defineOptions({ name: "DeepSeekPage" });

const app = useAppStore();
const store = useDeepseekStore();
const draft = ref("");
const scroller = ref<HTMLElement | null>(null);
const box = ref<HTMLTextAreaElement | null>(null);

onMounted(() => {
  void store.load();
});

const groups = computed(() => {
  const start = new Date();
  start.setHours(0, 0, 0, 0);
  const today = start.getTime();
  const yesterday = today - 86400000;
  const buckets: { label: string; items: DsChat[] }[] = [
    { label: "今天", items: [] },
    { label: "昨天", items: [] },
    { label: "更早", items: [] },
  ];
  const sorted = [...store.chats].sort((a, b) => b.updatedAt - a.updatedAt);
  for (const chat of sorted) {
    if (chat.updatedAt >= today) buckets[0].items.push(chat);
    else if (chat.updatedAt >= yesterday) buckets[1].items.push(chat);
    else buckets[2].items.push(chat);
  }
  return buckets.filter((bucket) => bucket.items.length);
});

const tail = computed(() => {
  const messages = store.active?.messages;
  if (!messages?.length) return "";
  const last = messages[messages.length - 1];
  return `${last.content}\n${last.reasoning}`;
});

watch(tail, async () => {
  await nextTick();
  const el = scroller.value;
  if (el) el.scrollTop = el.scrollHeight;
});

function onKeydown(event: KeyboardEvent) {
  if (event.key !== "Enter" || event.shiftKey) return;
  event.preventDefault();
  void submit();
}

async function submit() {
  const text = draft.value;
  if (!text.trim() || store.busy) return;
  draft.value = "";
  await nextTick();
  if (box.value) box.value.style.height = "auto";
  await store.send(text);
}

function modelLabel(id: string) {
  if (id === "deepseek-chat") return "DeepSeek Chat";
  if (id === "deepseek-reasoner") return "DeepSeek Reasoner · 深度思考";
  return id;
}

const copied = ref("");

async function copyText(text: string, key: string) {
  try {
    await navigator.clipboard.writeText(text);
    copied.value = key;
    window.setTimeout(() => {
      if (copied.value === key) copied.value = "";
    }, 1200);
  } catch {
    copied.value = "";
  }
}

function onMdClick(event: MouseEvent, messageId: string) {
  const target = event.target;
  if (!(target instanceof HTMLElement)) return;
  const button = target.closest("button.copy");
  if (!(button instanceof HTMLButtonElement)) return;
  const code = button.parentElement?.querySelector("pre")?.innerText ?? "";
  void copyText(code, `${messageId}:code`);
  button.textContent = "已复制";
  window.setTimeout(() => {
    if (button.isConnected) button.textContent = "复制";
  }, 1200);
}

function grow() {
  const el = box.value;
  if (!el) return;
  el.style.height = "auto";
  el.style.height = `${Math.min(el.scrollHeight, 180)}px`;
}
</script>

<template>
  <div class="page">
    <aside class="side">
      <div class="brand">
        <svg viewBox="0 0 32 32" aria-hidden="true">
          <path fill="#4d6bfe" d="M8 18c0-6 4-11 9-11 2 0 3 1 4 2 2-1 4 0 5 2 2 1 3 4 2 6 2 1 2 4 0 6-2 3-6 5-10 5-6 0-10-4-10-10Z" />
          <circle cx="14" cy="16" r="1.2" fill="#fff" />
        </svg>
        DeepSeek
      </div>
      <button class="new" type="button" @click="store.newChat()">
        <span>+</span>
        开启新对话
      </button>
      <div class="list">
        <section v-for="group in groups" :key="group.label">
          <h3>{{ group.label }}</h3>
          <button
            v-for="chat in group.items"
            :key="chat.id"
            type="button"
            class="chat"
            :class="{ on: chat.id === store.activeId }"
            @click="store.openChat(chat.id)"
          >
            <span>{{ chat.title }}</span>
            <i title="删除" @click.stop="store.removeChat(chat.id)">×</i>
          </button>
        </section>
      </div>
      <div class="foot">
        <button class="mem-toggle" type="button" :class="{ on: store.memoryOn }" @click="store.memoryOn = !store.memoryOn">
          记忆
          <small>{{ store.memoryOn ? `已打开 · ${store.memories.length} 条` : "已关闭" }}</small>
        </button>
        <div v-show="store.memoryOn" class="mem-list">
          <p v-if="!store.memories.length">调教后的回答会记在这里，开新对话时也会带上。</p>
          <article v-for="item in [...store.memories].reverse()" :key="item.id">
            <header>
              <strong>{{ item.user }}</strong>
              <button type="button" title="忘掉这条" @click="store.forget(item.id)">×</button>
            </header>
            <p>{{ item.answer }}</p>
          </article>
        </div>
        <label>
          API Key
          <input v-model="store.apiKey" type="password" autocomplete="off" placeholder="sk-..." />
        </label>
      </div>
    </aside>

    <section class="main">
      <div ref="scroller" class="thread">
        <div v-if="!store.active?.messages.length" class="hello">
          <svg viewBox="0 0 64 64" aria-hidden="true">
            <path fill="#4d6bfe" d="M14 36c0-12 8-22 18-22 4 0 6 2 8 4 4-2 8 0 10 4 4 2 6 8 4 12 4 2 4 8 0 12-4 6-12 10-20 10-12 0-20-8-20-20Z" />
            <circle cx="28" cy="32" r="2.4" fill="#fff" />
          </svg>
          <h1>今天有什么可以帮到你？</h1>
        </div>
        <div v-else class="col">
          <article v-for="message in store.active.messages" :key="message.id" class="msg" :class="message.role">
            <div v-if="message.role === 'user'" class="bubble">
              <img v-if="message.image && !app.settings.stealthMode" :src="message.image" alt="" />
              {{ message.content }}
            </div>
            <div v-else class="assistant">
              <details v-if="message.reasoning" class="think" :open="!message.content">
                <summary>{{ message.content ? "已思考" : "思考中…" }}</summary>
                <p>{{ message.reasoning }}</p>
              </details>
              <div v-if="message.content" class="md" @click="onMdClick($event, message.id)" v-html="renderMarkdown(message.content)" />
              <span v-else-if="store.busy" class="caret" />
              <button v-if="message.content" class="copy-msg" type="button" @click="copyText(message.content, message.id)">
                {{ copied === message.id ? "已复制" : "复制" }}
              </button>
            </div>
          </article>
        </div>
      </div>

      <div class="dock">
        <p v-if="store.error" class="err">{{ store.error }}</p>
        <form class="composer" @submit.prevent="submit">
          <textarea
            ref="box"
            v-model="draft"
            rows="1"
            placeholder="给 DeepSeek 发送消息"
            @keydown="onKeydown"
            @input="grow"
          />
          <div class="bar">
            <select v-model="store.model" class="model" aria-label="选择模型">
              <option v-for="id in store.models" :key="id" :value="id">{{ modelLabel(id) }}</option>
            </select>
            <button v-if="store.busy" class="send stop" type="button" title="停止" @click="store.stop()">
              <i />
            </button>
            <button v-else class="send" type="submit" :disabled="!draft.trim()" title="发送">
              <svg viewBox="0 0 24 24" aria-hidden="true"><path fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" d="M12 19V6M6.5 11.5 12 6l5.5 5.5" /></svg>
            </button>
          </div>
        </form>
        <p class="hint">记忆保存的是调教之后的回答。Enter 发送，Shift+Enter 换行。</p>
      </div>
    </section>
  </div>
</template>

<style scoped>
.page {
  height: 100%;
  min-height: 0;
  display: flex;
  background: #fff;
  color: #1d2129;
  color-scheme: light;
  font-family: "Segoe UI", "PingFang SC", "Microsoft YaHei UI", sans-serif;
}
.side {
  width: 268px;
  flex: none;
  display: flex;
  flex-direction: column;
  background: #f7f9fc;
  border-right: 1px solid #e8edf5;
}
.brand {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 18px 16px 6px;
  font-size: 20px;
  font-weight: 700;
  color: #1d2129;
}
.brand svg, .hello svg { width: 28px; height: 28px; }
.hello svg { width: 54px; height: 54px; }
.new {
  margin: 10px 12px 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  height: 40px;
  border: 1px solid #d9e2f2;
  border-radius: 12px;
  background: #fff;
  color: #4d6bfe;
}
.new span { font-size: 18px; line-height: 1; }
.list { flex: 1; min-height: 0; overflow: auto; padding: 8px; }
.list h3 {
  margin: 10px 8px 4px;
  color: #86909c;
  font-family: inherit;
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0;
}
.chat {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 6px;
  border: 0;
  background: transparent;
  color: #1d2129;
  border-radius: 10px;
  padding: 8px 8px 8px 10px;
  text-align: left;
  font-weight: 500;
}
.chat span { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.chat i { display: none; font-style: normal; color: #86909c; padding: 0 4px; }
.chat:hover i, .chat.on i { display: block; }
.chat.on, .chat:hover { background: #e8eeff; }
.foot { padding: 10px 12px 14px; border-top: 1px solid #e8edf5; display: grid; gap: 8px; }
.mem-toggle, .foot label { display: flex; justify-content: space-between; align-items: center; color: #4e5969; font-size: 12px; }
.mem-toggle { border: 0; background: transparent; padding: 0; font-weight: 600; }
.mem-toggle.on small { color: #4d6bfe; }
.mem-list { max-height: 180px; overflow: auto; display: grid; gap: 8px; }
.mem-list p { margin: 0; color: #86909c; font-size: 12px; line-height: 1.45; display: -webkit-box; -webkit-line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; }
.mem-list article { background: #fff; border: 1px solid #e5e6eb; border-radius: 8px; padding: 6px 8px; }
.mem-list header { display: flex; gap: 6px; align-items: flex-start; }
.mem-list strong { flex: 1; min-width: 0; font-size: 12px; color: #1d2129; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
.mem-list header button { border: 0; background: transparent; color: #86909c; padding: 0 2px; line-height: 1; }
.mem-toggle small, .foot label { font-weight: 500; }
.foot textarea, .foot input {
  width: 100%;
  margin-top: 4px;
  background: #fff;
  color: #1d2129;
  border: 1px solid #e5e6eb;
  border-radius: 8px;
  padding: 8px;
}
.main { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.thread { flex: 1; min-height: 0; min-width: 0; overflow: auto; }
.hello {
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 14px;
}
.hello h1 {
  margin: 0;
  color: #1d2129;
  font-family: inherit;
  font-size: 26px;
  font-weight: 600;
  letter-spacing: 0;
}
.col { width: min(800px, calc(100% - 32px)); max-width: 100%; margin: 0 auto; padding: 28px 0 12px; display: flex; flex-direction: column; gap: 18px; min-width: 0; }
.msg { width: 100%; min-width: 0; max-width: 100%; background: transparent; }
.msg.user { display: flex; justify-content: flex-end; }
.bubble {
  flex: 0 1 auto;
  width: max-content;
  max-width: min(560px, 100%);
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  word-break: break-word;
  background: #edf2ff;
  color: #1d2129;
  border-radius: 14px;
  padding: 10px 14px;
  text-align: left;
}
.bubble img {
  display: block;
  width: 120px;
  max-height: 160px;
  object-fit: cover;
  border-radius: 8px;
  margin-bottom: 8px;
}
.assistant { min-width: 0; max-width: 100%; }
.md { color: #1d2129; line-height: 1.7; font-size: 15px; min-width: 0; max-width: 100%; overflow-wrap: anywhere; }
.md :deep(p) { margin: 0 0 10px; overflow-wrap: anywhere; word-break: break-word; }
.md :deep(h1), .md :deep(h2), .md :deep(h3) {
  margin: 14px 0 8px;
  color: #1d2129;
  font-family: inherit;
  letter-spacing: 0;
}
.md :deep(ul) { margin: 0 0 10px; padding-left: 1.2em; }
.md :deep(.codeblock) {
  position: relative;
  margin: 0 0 10px;
  max-width: 100%;
  min-width: 0;
  background: #f6f8fa;
  border-radius: 10px;
}
.md :deep(.copy) {
  position: absolute;
  top: 8px;
  right: 8px;
  border: 0;
  border-radius: 6px;
  background: #fff;
  color: #4e5969;
  font-size: 12px;
  padding: 2px 8px;
}
.md :deep(pre) {
  margin: 0;
  padding: 28px 12px 12px;
  max-width: 100%;
  overflow: auto;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  word-break: break-word;
}
.md :deep(code) { font-family: Consolas, "Sarasa Mono SC", monospace; font-size: 13px; white-space: pre-wrap; }
.md :deep(p code), .md :deep(li code) { background: #f2f3f5; padding: 1px 4px; border-radius: 4px; }
.md :deep(a) { color: #4d6bfe; font-weight: 500; }
.think { margin-bottom: 8px; color: #4e5969; font-size: 13px; min-width: 0; max-width: 100%; }
.think summary { cursor: pointer; color: #4d6bfe; }
.think p { margin: 8px 0 0; white-space: pre-wrap; overflow-wrap: anywhere; word-break: break-word; }
.copy-msg {
  margin-top: 4px;
  border: 0;
  background: transparent;
  color: #86909c;
  font-size: 12px;
  padding: 0;
}
.caret {
  display: inline-block;
  width: 8px;
  height: 16px;
  background: #4d6bfe;
  border-radius: 2px;
  animation: blink 1s steps(1) infinite;
}
@keyframes blink { 50% { opacity: 0; } }
.dock { width: min(800px, calc(100% - 32px)); margin: 0 auto 16px; }
.err { margin: 0 8px 8px; color: #d93025; font-size: 13px; }
.composer {
  border: 1px solid #e5e6eb;
  border-radius: 22px;
  background: #fff;
  box-shadow: 0 8px 24px rgba(29, 33, 41, 0.06);
  padding: 12px 12px 8px;
}
.composer textarea {
  width: 100%;
  border: 0;
  resize: none;
  outline: none;
  background: transparent;
  color: #1d2129;
  font-size: 15px;
  line-height: 1.5;
  max-height: 180px;
}
.bar { display: flex; align-items: center; justify-content: space-between; margin-top: 6px; }
.model {
  max-width: 240px;
  border: 1px solid #e5e6eb;
  background: #fff;
  color: #1d2129;
  border-radius: 999px;
  padding: 4px 12px;
  font-size: 13px;
}
.send {
  width: 32px;
  height: 32px;
  border: 0;
  border-radius: 50%;
  background: #4d6bfe;
  color: #fff;
  display: grid;
  place-items: center;
}
.send svg { width: 18px; height: 18px; }
.send:disabled { background: #c9cdd4; }
.send.stop i { width: 10px; height: 10px; background: #fff; border-radius: 2px; }
.hint { margin: 8px 0 0; text-align: center; color: #86909c; font-size: 12px; }
</style>
