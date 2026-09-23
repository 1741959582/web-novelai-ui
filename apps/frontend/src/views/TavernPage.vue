<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { llmModels, tavernOpenDir, type ProviderId, type TavernCard } from "@/api/tavern";
import CardEditor from "@/components/tavern/CardEditor.vue";
import ChatMessageItem from "@/components/tavern/ChatMessageItem.vue";
import { useAppStore } from "@/stores/app";
import { DEFAULT_DIRECTOR_PROMPT, DEFAULT_MAIN_PROMPT, FOLLOW_STYLE, useTavernStore } from "@/stores/tavern";
import { NAI_MODELS, NAI_SAMPLERS, isV4Plus } from "@/types/nai";
import { emptyCard } from "@/utils/charaCard";

const tavern = useTavernStore();
const app = useAppStore();
const router = useRouter();

type SideTab = "cards" | "chars" | "styles" | "api";
const sideTab = ref<SideTab>("cards");
const showPanel = ref(true);
const input = ref("");
const editing = ref<TavernCard | null>(null);
const zoomUrl = ref("");
const models = ref<string[]>([]);
const modelsBusy = ref(false);
const scroller = ref<HTMLElement | null>(null);
const cardFile = ref<HTMLInputElement | null>(null);
const presetFile = ref<HTMLInputElement | null>(null);
const cardQuery = ref("");

const cfg = computed(() => tavern.state.config);
const prov = computed(() => tavern.provider);
const messages = computed(() => tavern.chat?.messages ?? []);
const lastAssistant = computed(() => [...messages.value].reverse().find((m) => m.role === "assistant") ?? null);
const filteredCards = computed(() => {
  const q = cardQuery.value.trim().toLowerCase();
  if (!q) return tavern.state.cards;
  return tavern.state.cards.filter((c) => c.name.toLowerCase().includes(q) || c.tags.some((t) => t.toLowerCase().includes(q)));
});
const modelLabel = computed(() => NAI_MODELS.find((m) => m.value === app.params.model)?.label ?? app.params.model);
const samplerLabel = computed(() => NAI_SAMPLERS.find((s) => s.value === app.params.sampler)?.short ?? app.params.sampler);
const charPromptOk = computed(() => isV4Plus(app.params.model));

onMounted(async () => {
  await tavern.load();
  scrollBottom();
});

onBeforeUnmount(() => tavern.flush());

watch(
  () => [
    messages.value.length,
    messages.value[messages.value.length - 1]?.swipes.join("").length,
    messages.value[messages.value.length - 1]?.draftState,
  ],
  () => scrollBottom(true),
);

watch(
  () => [messages.value.reduce((n, m) => n + m.images.length, 0), Object.keys(tavern.imageUrls).length],
  () => window.setTimeout(() => scrollBottom(), 60),
);

function scrollBottom(onlyIfNear = false) {
  void nextTick(() => {
    const el = scroller.value;
    if (!el) return;
    if (onlyIfNear && el.scrollHeight - el.scrollTop - el.clientHeight > 240) return;
    el.scrollTop = el.scrollHeight;
  });
}

// ---------------- 输入 ----------------
async function send() {
  const text = input.value;
  input.value = "";
  scrollBottom();
  await tavern.send(text);
}

function onKey(e: KeyboardEvent) {
  if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
    e.preventDefault();
    if (!tavern.busy) void send();
  }
}

// ---------------- 角色卡 ----------------
function newCard() {
  const card = tavern.addCard(emptyCard());
  tavern.selectCard(card.id);
  editing.value = card;
}

function onCardFiles(e: Event) {
  const files = (e.target as HTMLInputElement).files;
  if (files?.length) void tavern.importCardFiles(files);
  (e.target as HTMLInputElement).value = "";
}

function deleteCard(card: TavernCard) {
  if (window.confirm(`删除角色卡「${card.name}」和它的所有聊天记录？`)) void tavern.deleteCard(card.id);
}

function deleteChat() {
  if (tavern.chat && window.confirm("删除当前对话？")) void tavern.deleteChat(tavern.chat.id);
}

// ---------------- 预设 ----------------
function newCharPreset() {
  tavern.state.charPresets.unshift({ id: crypto.randomUUID(), name: `角色 ${tavern.state.charPresets.length + 1}`, note: "", prompt: "", negative: "" });
}

function newStylePreset() {
  tavern.state.stylePresets.unshift({ id: crypto.randomUUID(), name: `画风 ${tavern.state.stylePresets.length + 1}`, prompt: "", negative: "" });
}

function removeCharPreset(id: string) {
  if (!window.confirm("删除这个角色 tag 预设？")) return;
  tavern.state.charPresets = tavern.state.charPresets.filter((p) => p.id !== id);
  for (const c of tavern.state.cards) c.charPresetIds = c.charPresetIds.filter((x) => x !== id);
  if (tavern.chat) tavern.chat.castIds = tavern.chat.castIds.filter((x) => x !== id);
}

function removeStylePreset(id: string) {
  if (!window.confirm("删除这个画风预设？")) return;
  tavern.state.stylePresets = tavern.state.stylePresets.filter((p) => p.id !== id);
  for (const c of tavern.state.cards) if (c.stylePresetId === id) c.stylePresetId = "";
  if (tavern.chat?.styleId === id) tavern.chat.styleId = "";
}

function inCast(id: string) {
  return tavern.chat?.castIds.includes(id) ?? false;
}

function toggleCast(id: string) {
  if (!tavern.chat) return;
  const ids = tavern.chat.castIds;
  tavern.chat.castIds = ids.includes(id) ? ids.filter((x) => x !== id) : [...ids, id];
}

function onPresetFile(e: Event) {
  const file = (e.target as HTMLInputElement).files?.[0];
  if (file) void tavern.importPresets(file);
  (e.target as HTMLInputElement).value = "";
}

// ---------------- 接口 ----------------
async function fetchModels() {
  modelsBusy.value = true;
  tavern.error = "";
  try {
    models.value = await llmModels(prov.value.baseUrl, prov.value.apiKey);
    tavern.notice = models.value.length ? `获取到 ${models.value.length} 个模型，点模型框可选择` : "接口没有返回模型列表";
  } catch (e) {
    tavern.error = String(e);
  } finally {
    modelsBusy.value = false;
  }
}

function switchProvider(id: ProviderId) {
  cfg.value.provider = id;
  models.value = [];
}

function resetPrompts() {
  if (!window.confirm("把主提示词和插图导演提示词恢复为默认？")) return;
  cfg.value.mainPrompt = DEFAULT_MAIN_PROMPT;
  cfg.value.directorPrompt = DEFAULT_DIRECTOR_PROMPT;
}

// ---------------- 插图 ----------------
async function illustrateLatest() {
  const m = lastAssistant.value;
  if (!m) return;
  await tavern.planImage(m);
  if (cfg.value.imageMode === "auto" && m.draftState === "ready") await tavern.drawImage(m);
}

function dismissMsg() {
  tavern.error = "";
  tavern.notice = "";
}
</script>

<template>
  <div class="tavern" :class="{ 'no-panel': !showPanel }">
    <!-- ============ 左侧 ============ -->
    <aside class="side">
      <nav class="tabs">
        <button type="button" :class="{ on: sideTab === 'cards' }" @click="sideTab = 'cards'">角色卡</button>
        <button type="button" :class="{ on: sideTab === 'chars' }" @click="sideTab = 'chars'">角色tag</button>
        <button type="button" :class="{ on: sideTab === 'styles' }" @click="sideTab = 'styles'">画风</button>
        <button type="button" :class="{ on: sideTab === 'api' }" @click="sideTab = 'api'">接口</button>
      </nav>

      <!-- 角色卡 -->
      <section v-if="sideTab === 'cards'" class="pane">
        <div class="bar">
          <button type="button" class="act" @click="cardFile?.click()">导入</button>
          <button type="button" class="act" @click="newCard">新建</button>
          <input ref="cardFile" type="file" accept=".png,.json" multiple hidden @change="onCardFiles" />
        </div>
        <input v-model="cardQuery" class="search" placeholder="搜索名字或标签" />
        <p v-if="!tavern.state.cards.length" class="hint">支持 SillyTavern 的 PNG / JSON 角色卡（V1/V2/V3），可以一次选多张。</p>
        <div class="list">
          <article
            v-for="card in filteredCards"
            :key="card.id"
            class="card-item"
            :class="{ on: card.id === tavern.state.activeCardId }"
            @click="tavern.selectCard(card.id)"
          >
            <div class="thumb"><img v-if="card.avatar" :src="card.avatar" alt="" /><span v-else>{{ card.name.slice(0, 1) }}</span></div>
            <div class="meta">
              <b>{{ card.name }}</b>
              <small>{{ card.tags.slice(0, 3).join(" · ") || card.description.slice(0, 30) }}</small>
              <div v-if="card.id === tavern.state.activeCardId" class="mini" @click.stop>
                <button type="button" @click="editing = card">编辑</button>
                <button type="button" @click="tavern.exportCard(card, 'png')">导出PNG</button>
                <button type="button" @click="tavern.exportCard(card, 'json')">JSON</button>
                <button type="button" class="danger" @click="deleteCard(card)">删除</button>
              </div>
            </div>
          </article>
        </div>
      </section>

      <!-- 角色 tag 预设 -->
      <section v-else-if="sideTab === 'chars'" class="pane">
        <div class="bar">
          <button type="button" class="act" @click="newCharPreset">新建</button>
          <button type="button" class="act" title="把生成页的角色提示词导入为预设" @click="tavern.importCharsFromGeneratePage">从生成页</button>
          <button type="button" class="act" @click="presetFile?.click()">导入</button>
          <button type="button" class="act" @click="tavern.exportPresets">导出</button>
          <input ref="presetFile" type="file" accept=".json" hidden @change="onPresetFile" />
        </div>
        <p class="hint">每个预设是一个角色的固定外貌 tag。勾选「出场」后，AI 构思插图时会从这里挑人，并只补充动作和表情。</p>
        <p v-if="!charPromptOk" class="warn">当前生成页模型不支持角色提示词，出图时会合并进正面提示词。</p>
        <div class="list">
          <article v-for="p in tavern.state.charPresets" :key="p.id" class="preset" :class="{ on: inCast(p.id) }">
            <div class="preset-head">
              <input v-model="p.name" class="name" placeholder="名字（AI 用它识别）" />
              <label v-if="tavern.chat" class="inline"><input type="checkbox" :checked="inCast(p.id)" @change="toggleCast(p.id)" />出场</label>
              <button type="button" class="x" @click="removeCharPreset(p.id)">×</button>
            </div>
            <input v-model="p.note" placeholder="说明 / 别名（可选，如：女主角, Alice）" />
            <textarea v-model="p.prompt" rows="3" placeholder="girl, silver hair, long hair, red eyes, school uniform, ..." />
            <details>
              <summary>角色负面</summary>
              <textarea v-model="p.negative" rows="2" placeholder="可留空" />
            </details>
          </article>
        </div>
      </section>

      <!-- 画风预设 -->
      <section v-else-if="sideTab === 'styles'" class="pane">
        <div class="bar">
          <button type="button" class="act" @click="newStylePreset">新建</button>
          <button type="button" class="act" @click="tavern.importStyleFromGeneratePage">从生成页</button>
          <button type="button" class="act" @click="presetFile?.click()">导入</button>
          <button type="button" class="act" @click="tavern.exportPresets">导出</button>
          <input ref="presetFile" type="file" accept=".json" hidden @change="onPresetFile" />
        </div>
        <p class="hint">画风会写进 NAI 的画风提示词（放在最前），适合放画师串、质量词、媒介等。</p>
        <div class="list">
          <article v-for="s in tavern.state.stylePresets" :key="s.id" class="preset" :class="{ on: tavern.chat?.styleId === s.id }">
            <div class="preset-head">
              <input v-model="s.name" class="name" />
              <label v-if="tavern.chat" class="inline"><input type="radio" :checked="tavern.chat.styleId === s.id" @change="tavern.chat!.styleId = s.id" />使用</label>
              <button type="button" class="x" @click="removeStylePreset(s.id)">×</button>
            </div>
            <textarea v-model="s.prompt" rows="3" placeholder="artist:xxx, year 2024, ..." />
            <details>
              <summary>画风负面</summary>
              <textarea v-model="s.negative" rows="2" placeholder="可留空" />
            </details>
          </article>
        </div>
      </section>

      <!-- 接口 -->
      <section v-else class="pane form">
        <div class="seg">
          <button v-for="id in (['deepseek', 'xai', 'custom'] as ProviderId[])" :key="id" type="button" :class="{ on: cfg.provider === id }" @click="switchProvider(id)">
            {{ cfg.providers[id].label.split("（")[0] }}
          </button>
        </div>
        <label>API 地址<input v-model="prov.baseUrl" placeholder="https://…/v1" /></label>
        <label>API Key<input v-model="prov.apiKey" type="password" placeholder="sk-… / xai-…" autocomplete="off" /></label>
        <label>对话模型
          <div class="with-btn">
            <input v-model="prov.model" list="tavern-models" />
            <button type="button" class="act" :disabled="modelsBusy" @click="fetchModels">{{ modelsBusy ? "…" : "获取列表" }}</button>
          </div>
        </label>
        <label>插图构思模型（留空 = 同上）<input v-model="prov.directorModel" list="tavern-models" placeholder="可用便宜快速的模型" /></label>
        <datalist id="tavern-models"><option v-for="m in models" :key="m" :value="m" /></datalist>
        <p class="hint">Key 只保存在本机。走「设置」里的代理。</p>

        <h4>生成参数</h4>
        <div class="grid2">
          <label>温度 {{ cfg.temperature }}<input v-model.number="cfg.temperature" type="range" min="0" max="2" step="0.05" /></label>
          <label>Top P {{ cfg.topP }}<input v-model.number="cfg.topP" type="range" min="0" max="1" step="0.05" /></label>
          <label>最大回复长度<input v-model.number="cfg.maxTokens" type="number" min="0" step="100" /></label>
          <label>带入历史条数<input v-model.number="cfg.contextMessages" type="number" min="2" step="2" /></label>
          <label>世界书扫描条数<input v-model.number="cfg.loreScanDepth" type="number" min="1" /></label>
          <label>插图参考条数<input v-model.number="cfg.directorContext" type="number" min="1" /></label>
        </div>
        <label class="inline"><input v-model="cfg.stream" type="checkbox" />流式输出</label>
        <label class="inline"><input v-model="cfg.showReasoning" type="checkbox" />显示推理模型的思考过程</label>

        <h4>我的设定</h4>
        <label>我的名字<input v-model="cfg.userName" /></label>
        <label>我的人设（可选）<textarea v-model="cfg.userPersona" rows="3" /></label>

        <h4>提示词</h4>
        <label>主提示词<textarea v-model="cfg.mainPrompt" rows="7" /></label>
        <details>
          <summary>插图导演提示词（高级）</summary>
          <textarea v-model="cfg.directorPrompt" rows="14" />
          <p class="hint"><code v-pre>{{cast}}</code> 会被替换为出场角色列表。必须让模型只输出 JSON。</p>
        </details>
        <div class="bar">
          <button type="button" class="act" @click="resetPrompts">恢复默认提示词</button>
          <button type="button" class="act" @click="tavernOpenDir">打开数据目录</button>
        </div>
      </section>
    </aside>

    <!-- ============ 中间聊天 ============ -->
    <main class="chat">
      <header class="chat-head">
        <template v-if="tavern.activeCard">
          <div class="thumb sm"><img v-if="tavern.activeCard.avatar" :src="tavern.activeCard.avatar" alt="" /><span v-else>{{ tavern.activeCard.name.slice(0, 1) }}</span></div>
          <b class="title">{{ tavern.activeCard.name }}</b>
          <select :value="tavern.chat?.id" @change="tavern.openChat(($event.target as HTMLSelectElement).value)">
            <option v-for="c in tavern.cardChats" :key="c.id" :value="c.id">{{ c.title }}（{{ c.count }}）</option>
          </select>
          <button type="button" class="act" :disabled="tavern.busy" @click="tavern.newChat">新对话</button>
          <button type="button" class="act" :disabled="tavern.busy" @click="deleteChat">删除对话</button>
        </template>
        <b v-else class="title">酒馆</b>
        <span class="grow" />
        <button type="button" class="act" @click="showPanel = !showPanel">{{ showPanel ? "收起插图栏" : "插图栏" }}</button>
      </header>

      <div v-if="tavern.error || tavern.notice" class="banner" :class="{ err: tavern.error }" @click="dismissMsg">
        {{ tavern.error || tavern.notice }} <small>（点击关闭）</small>
      </div>

      <div ref="scroller" class="messages">
        <div v-if="!tavern.activeCard" class="empty">
          <h3>和角色对话，边聊边出图</h3>
          <ol>
            <li>左侧「接口」选择 DeepSeek 或 xAI，填入 API Key。</li>
            <li>「角色卡」导入 SillyTavern 角色卡，或新建一张。</li>
            <li>「角色tag」写好角色外貌 tag，「画风」写好画师串。</li>
            <li>右侧插图栏选择出图方式：手动、确认后生图、全自动。</li>
          </ol>
          <p class="hint">出图使用生成页当前的模型、采样器、步数等参数，需要在「设置」里填好 NovelAI Token。</p>
        </div>
        <template v-else>
          <p v-if="!messages.length" class="hint center">这张卡没有开场白，直接开始说话吧。</p>
          <ChatMessageItem
            v-for="(m, i) in messages"
            :key="m.id"
            :message="m"
            :is-last="i === messages.length - 1"
            @zoom="zoomUrl = $event"
          />
        </template>
      </div>

      <footer v-if="tavern.activeCard" class="composer">
        <textarea
          v-model="input"
          rows="3"
          :placeholder="`以 ${cfg.userName || 'User'} 的身份说话…（Enter 发送，Shift+Enter 换行）`"
          @keydown="onKey"
        />
        <div class="send-btns">
          <button v-if="tavern.busy" type="button" class="btn danger" @click="tavern.stop">停止</button>
          <button v-else type="button" class="btn primary" @click="send">发送</button>
          <button type="button" class="btn" :disabled="tavern.busy || !messages.length" @click="tavern.regenerate">重新生成</button>
        </div>
      </footer>
    </main>

    <!-- ============ 右侧插图栏 ============ -->
    <aside v-if="showPanel" class="panel">
      <h4>插图</h4>
      <div class="seg">
        <button type="button" :class="{ on: cfg.imageMode === 'manual' }" @click="cfg.imageMode = 'manual'">手动</button>
        <button type="button" :class="{ on: cfg.imageMode === 'confirm' }" @click="cfg.imageMode = 'confirm'">确认后生图</button>
        <button type="button" :class="{ on: cfg.imageMode === 'auto' }" @click="cfg.imageMode = 'auto'">全自动</button>
      </div>
      <p class="hint">
        {{ cfg.imageMode === "manual" ? "点消息上的 🎨 才会构思插图。" : cfg.imageMode === "confirm" ? "每次回复后 AI 自动写好提示词，你检查修改后再点生成。" : "每次回复后自动构思并出图（消耗 Anlas / 次数）。" }}
      </p>

      <template v-if="tavern.chat">
        <h5>本次对话出场角色</h5>
        <p v-if="!tavern.state.charPresets.length" class="hint">还没有角色 tag 预设。没有也能出图，AI 会自己写外貌，但一致性会差一些。</p>
        <label v-for="p in tavern.state.charPresets" :key="p.id" class="inline cast">
          <input type="checkbox" :checked="inCast(p.id)" @change="toggleCast(p.id)" />
          <span>{{ p.name }}</span>
        </label>

        <h5>画风</h5>
        <select v-model="tavern.chat.styleId">
          <option value="">不使用</option>
          <option :value="FOLLOW_STYLE">跟随生成页画风</option>
          <option v-for="s in tavern.state.stylePresets" :key="s.id" :value="s.id">{{ s.name }}</option>
        </select>
      </template>

      <h5>尺寸</h5>
      <div class="seg">
        <button type="button" :class="{ on: cfg.sizeMode === 'auto' }" @click="cfg.sizeMode = 'auto'">AI 选横竖</button>
        <button type="button" :class="{ on: cfg.sizeMode === 'follow' }" @click="cfg.sizeMode = 'follow'">跟随生成页</button>
      </div>

      <h5>通用负面</h5>
      <textarea v-model="cfg.extraNegative" rows="2" placeholder="每张插图都会带上" />

      <h5>出图参数</h5>
      <p class="params">
        {{ modelLabel }}<br />
        {{ samplerLabel }} · {{ app.params.steps }} 步 · CFG {{ app.params.cfgScale }}
        <template v-if="cfg.sizeMode === 'follow'"><br />{{ app.params.width }}×{{ app.params.height }}</template>
      </p>
      <button type="button" class="act wide" @click="router.push('/')">去生成页调整参数</button>

      <div class="panel-actions">
        <button type="button" class="btn primary" :disabled="!lastAssistant || tavern.busy || lastAssistant.draftState === 'loading'" @click="illustrateLatest">
          为最新回复构思插图
        </button>
        <button type="button" class="btn" :disabled="!lastAssistant" @click="lastAssistant && tavern.manualDraft(lastAssistant)">手写插图提示词</button>
      </div>
      <p v-if="!app.hasToken" class="warn">还没有填写 NovelAI Token，无法出图。</p>
    </aside>

    <CardEditor v-if="editing" :card="editing" @close="editing = null" />
    <div v-if="zoomUrl" class="zoom" @click="zoomUrl = ''"><img :src="zoomUrl" alt="" /></div>
  </div>
</template>

<style scoped>
.tavern { height: 100%; min-height: 0; display: grid; grid-template-columns: 300px minmax(0, 1fr) 260px; overflow: hidden; }
.tavern.no-panel { grid-template-columns: 300px minmax(0, 1fr); }
.side, .panel { min-height: 0; display: flex; flex-direction: column; background: var(--bg1); border-right: 1px solid var(--bg3); }
.panel { border-right: 0; border-left: 1px solid var(--bg3); padding: 12px; overflow: auto; }
.tabs { display: flex; gap: 4px; padding: 10px 10px 0; }
.tabs button { flex: 1; border: 0; background: #2e3152; color: var(--heading); border-radius: 8px; padding: 7px 0; font-size: 13px; }
.tabs button.on { background: #3d4270; }
.pane { flex: 1; min-height: 0; overflow: auto; padding: 10px; display: flex; flex-direction: column; gap: 8px; }
.bar { display: flex; gap: 6px; flex-wrap: wrap; }
.act { border: 0; border-radius: 8px; background: #2e3152; color: var(--heading); padding: 6px 10px; font-size: 12.5px; }
.act:disabled { opacity: 0.45; cursor: default; }
.act.wide { width: 100%; }
input, textarea, select { background: #16182d; border-radius: 8px; padding: 7px 9px; font-size: 13px; color: #fff; }
textarea { resize: vertical; line-height: 1.55; }
.search { width: 100%; }
.list { display: flex; flex-direction: column; gap: 8px; }
.card-item { display: flex; gap: 10px; padding: 8px; border-radius: 10px; background: #16182d; cursor: pointer; border: 1px solid transparent; }
.card-item.on { border-color: var(--heading); }
.thumb { width: 52px; height: 72px; flex: none; border-radius: 8px; overflow: hidden; background: var(--bg3); display: grid; place-items: center; color: var(--heading); font-weight: 700; }
.thumb img { width: 100%; height: 100%; object-fit: cover; object-position: top; }
.thumb.sm { width: 32px; height: 32px; border-radius: 50%; }
.meta { min-width: 0; display: flex; flex-direction: column; gap: 2px; flex: 1; }
.meta small { color: var(--muted); font-size: 11.5px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.mini { display: flex; gap: 4px; margin-top: 4px; flex-wrap: wrap; }
.mini button { border: 0; background: #2e3152; color: var(--heading); border-radius: 6px; padding: 3px 7px; font-size: 11.5px; }
.mini .danger { color: var(--danger); }
.preset { background: #16182d; border-radius: 10px; padding: 8px; display: flex; flex-direction: column; gap: 6px; border: 1px solid transparent; }
.preset.on { border-color: #f5f3c255; }
.preset input, .preset textarea { background: #0e0f21; }
.preset details summary { font-size: 11.5px; color: var(--muted); cursor: pointer; }
.preset details textarea { width: 100%; margin-top: 4px; }
.preset-head { display: flex; gap: 6px; align-items: center; }
.preset-head .name { flex: 1; min-width: 0; font-weight: 600; }
.x { background: none; border: 0; color: var(--danger); font-size: 18px; padding: 0 4px; }
.inline { display: flex; flex-direction: row; align-items: center; gap: 5px; font-size: 12.5px; color: var(--text); white-space: nowrap; }
.form label:not(.inline) { display: flex; flex-direction: column; gap: 4px; font-size: 12px; color: var(--muted); }
.form h4, .panel h4 { margin: 8px 0 0; font-size: 14px; }
.panel h5 { margin: 14px 0 6px; font-size: 12.5px; color: var(--heading); font-family: inherit; }
.with-btn { display: flex; gap: 6px; }
.with-btn input { flex: 1; min-width: 0; }
.grid2 { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
.grid2 input[type="number"] { width: 100%; }
.form details summary { font-size: 12.5px; color: var(--heading); cursor: pointer; }
.form details textarea { width: 100%; margin-top: 6px; }
.seg { display: flex; gap: 4px; }
.seg button { flex: 1; border: 0; background: #2e3152; color: var(--heading); border-radius: 8px; padding: 6px 0; font-size: 12.5px; }
.seg button.on { background: var(--heading); color: var(--bg0); }
.hint { color: var(--muted); font-size: 12px; line-height: 1.55; margin: 0; }
.hint.center { text-align: center; padding: 20px; }
.warn { color: #fbbf24; font-size: 12px; margin: 0; }

.chat { min-height: 0; display: flex; flex-direction: column; }
.chat-head { display: flex; align-items: center; gap: 8px; padding: 8px 14px; border-bottom: 1px solid var(--bg3); }
.chat-head .title { color: var(--heading); font-size: 15px; white-space: nowrap; }
.chat-head select { max-width: 240px; }
.grow { flex: 1; }
.banner { margin: 8px 14px 0; padding: 7px 10px; border-radius: 8px; background: #34d39922; color: #a7f3d0; font-size: 12.5px; cursor: pointer; word-break: break-all; }
.banner.err { background: #ff787822; color: #fecaca; }
.banner small { opacity: 0.6; }
.messages { flex: 1; min-height: 0; overflow: auto; padding: 4px 18px 12px; }
.empty { max-width: 520px; margin: 60px auto; }
.empty ol { line-height: 2; padding-left: 20px; color: #ffffffcc; }
.composer { display: flex; gap: 8px; padding: 10px 14px 12px; border-top: 1px solid var(--bg3); }
.composer textarea { flex: 1; resize: none; font-size: 14px; }
.send-btns { display: flex; flex-direction: column; gap: 6px; width: 96px; }
.send-btns .btn { padding: 7px 0; }
.btn:disabled { opacity: 0.45; cursor: default; }

.cast { margin-bottom: 4px; }
.panel select, .panel textarea { width: 100%; }
.params { font-size: 12px; color: #ffffffcc; line-height: 1.6; margin: 0 0 8px; }
.panel-actions { display: flex; flex-direction: column; gap: 6px; margin: 16px 0 8px; }

.zoom { position: fixed; inset: 0; background: #000d; z-index: 60; display: grid; place-items: center; cursor: zoom-out; }
.zoom img { max-width: 94vw; max-height: 94vh; border-radius: 6px; }

@media (max-width: 1100px) {
  .tavern { grid-template-columns: 250px minmax(0, 1fr) 230px; }
  .tavern.no-panel { grid-template-columns: 250px minmax(0, 1fr); }
}
</style>
