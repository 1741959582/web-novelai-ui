<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { useRouter } from "vue-router";
import { useAppStore } from "@/stores/app";
import { useMaterialLibrary } from "@/stores/materialLibrary";
import { readImageDataUrl } from "@/api/tauri";
import { promptBlockName } from "@/utils/artistString";
import NaiIcon from "@/components/NaiIcon.vue";
import type { HistoryItem } from "@/types/nai";

const router = useRouter();
const app = useAppStore();
const lib = useMaterialLibrary();
const view = ref<"all" | "recent" | "favorite" | "folder">("all");
const folderId = ref("");
const query = ref("");
const creating = ref(false);
const importing = ref(false);
const importText = ref("");
const draft = reactive({ name: "", positive: "", negative: "" });
const fileInput = ref<HTMLInputElement | null>(null);
const pickingHistory = ref(false);
const historyPicked = ref<string[]>([]);
const historyThumbs = ref<Record<string, string>>({});

const shown = computed(() => {
  const q = query.value.trim().toLowerCase();
  return lib.blocks.filter((block) => {
    if (view.value === "recent" && !block.usedAt) return false;
    if (view.value === "favorite" && !block.favorite) return false;
    if (view.value === "folder" && block.folderId !== folderId.value) return false;
    if (!q) return true;
    return `${block.name} ${block.positive} ${block.negative}`.toLowerCase().includes(q);
  });
});

const heading = computed(() => {
  if (view.value === "recent") return "最近使用";
  if (view.value === "favorite") return "收藏";
  if (view.value === "folder") return lib.folders.find((item) => item.id === folderId.value)?.name || "未分类";
  return "全部块";
});

function openCreate() {
  draft.name = "";
  draft.positive = "";
  draft.negative = "";
  creating.value = true;
}

function saveBlock() {
  if (!draft.positive.trim() && !draft.negative.trim()) return;
  lib.addBlock({
    name: draft.name,
    positive: draft.positive,
    negative: draft.negative,
    folderId: view.value === "folder" ? folderId.value : "",
  });
  creating.value = false;
}

function useBlock(id: string) {
  const block = lib.blocks.find((item) => item.id === id);
  if (!block) return;
  app.params.positivePrompt = block.positive;
  app.params.negativePrompt = block.negative;
  lib.touchBlock(id);
  app.status = `已把「${block.name}」的基础提示词和负面填回去`;
  void router.push("/");
}

const historyItems = computed(() => [...app.history].slice(0, 80));

function folderCount(id: string) {
  return lib.blocks.filter((block) => block.folderId === id).length;
}

function saveArtistFromHistory(items: HistoryItem[]) {
  let saved = 0;
  let skipped = 0;
  const folder = view.value === "folder" ? folderId.value : "";
  for (const item of items) {
    const positive = item.prompt.trim();
    const negative = item.negativePrompt.trim();
    if (!positive && !negative) {
      skipped += 1;
      continue;
    }
    if (lib.blocks.some((block) => block.positive === positive && block.negative === negative)) {
      skipped += 1;
      continue;
    }
    lib.addBlock({ name: promptBlockName(positive), positive, negative, folderId: folder });
    saved += 1;
  }
  return { saved, skipped };
}

async function loadHistoryThumbs() {
  const updates: Record<string, string> = {};
  for (const item of historyItems.value.slice(0, 24)) {
    if (historyThumbs.value[item.id]) continue;
    try {
      updates[item.id] = await readImageDataUrl(item.path);
    } catch {
      updates[item.id] = "";
    }
  }
  if (Object.keys(updates).length) historyThumbs.value = { ...historyThumbs.value, ...updates };
}

function openHistoryPick() {
  historyPicked.value = [];
  pickingHistory.value = true;
  void loadHistoryThumbs();
}

function toggleHistory(id: string) {
  historyPicked.value = historyPicked.value.includes(id)
    ? historyPicked.value.filter((item) => item !== id)
    : [...historyPicked.value, id];
}

function savePickedArtists() {
  const items = historyItems.value.filter((item) => historyPicked.value.includes(item.id));
  const { saved, skipped } = saveArtistFromHistory(items);
  pickingHistory.value = false;
  app.status = saved
    ? `已把 ${saved} 条基础提示词和负面存进块库${skipped ? `，${skipped} 条已经在库里` : ""}`
    : "这些记录没有新的提示词可存";
}

function addFolder() {
  const name = window.prompt("文件夹名称");
  if (!name?.trim()) return;
  const folder = lib.addFolder(name);
  folderId.value = folder.id;
  view.value = "folder";
}

function pickFolder(id: string) {
  folderId.value = id;
  view.value = "folder";
}

function importParagraphs() {
  const chunks = importText.value.split(/\n\s*\n/).map((part) => part.trim()).filter(Boolean);
  if (!chunks.length) return;
  for (const chunk of chunks) {
    const lines = chunk.split("\n");
    const name = lines[0].length <= 24 && !lines[0].includes(",") ? lines[0] : "";
    const body = name ? lines.slice(1).join("\n").trim() : chunk;
    const [positive, negative = ""] = body.split(/\n-{3,}\n/);
    lib.addBlock({ name: name || "导入块", positive: positive || body, negative, folderId: folderId.value });
  }
  importText.value = "";
  importing.value = false;
  app.status = `已导入 ${chunks.length} 个提示词块`;
}

function exportJson() {
  const blob = new Blob([JSON.stringify(lib.blocks, null, 2)], { type: "application/json" });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = "prompt-blocks.json";
  link.click();
  URL.revokeObjectURL(url);
}

function onImportFile(event: Event) {
  const file = (event.target as HTMLInputElement).files?.[0];
  if (!file) return;
  const reader = new FileReader();
  reader.onload = () => {
    const text = String(reader.result || "");
    if (file.name.endsWith(".json")) {
      try {
        const parsed = JSON.parse(text);
        const list = Array.isArray(parsed) ? parsed : [];
        for (const item of list) {
          if (!item || typeof item !== "object") continue;
          lib.addBlock({
            name: String(item.name || "导入块"),
            positive: String(item.positive || item.prompt || ""),
            negative: String(item.negative || ""),
          });
        }
        app.status = `已导入 ${list.length} 个提示词块`;
      } catch {
        app.status = "这个 JSON 读不了";
      }
    } else {
      importText.value = text;
      importing.value = true;
    }
  };
  reader.readAsText(file);
  (event.target as HTMLInputElement).value = "";
}
</script>

<template>
  <div class="lib">
    <aside>
      <header>
        <NaiIcon name="grid" :size="16" />
        <strong>块库</strong>
      </header>
      <button type="button" :class="{ on: view === 'all' }" @click="view = 'all'"><NaiIcon name="grid" :size="14" /> 全部块</button>
      <button type="button" :class="{ on: view === 'recent' }" @click="view = 'recent'"><NaiIcon name="history" :size="14" /> 最近使用</button>
      <button type="button" :class="{ on: view === 'favorite' }" @click="view = 'favorite'"><NaiIcon name="star" :size="14" /> 收藏</button>
      <p class="label">收藏集</p>
      <p class="note">把卡片拖到这里的收藏集，一个块可以进多个收藏集。</p>
      <div class="label-row">
        <span>文件夹</span>
        <button class="add-folder" type="button" title="新建文件夹" @click="addFolder"><NaiIcon name="plus" :size="13" /></button>
      </div>
      <button type="button" class="folder" :class="{ on: view === 'folder' && !folderId }" @click="pickFolder('')">
        <NaiIcon name="folder" :size="14" />
        <span>未分类</span>
        <em>{{ folderCount("") }}</em>
      </button>
      <button v-for="folder in lib.folders" :key="folder.id" type="button" class="folder" :class="{ on: view === 'folder' && folderId === folder.id }" @click="pickFolder(folder.id)">
        <NaiIcon name="folder" :size="14" />
        <span>{{ folder.name }}</span>
        <em>{{ folderCount(folder.id) }}</em>
      </button>
      <button class="foot" type="button" @click="fileInput?.click()">导入 / 导出</button>
      <input ref="fileInput" hidden type="file" accept=".txt,.json,text/plain,application/json" @change="onImportFile" />
    </aside>
    <section>
      <header class="bar">
        <h2>{{ heading }}</h2>
        <label class="search">
          <NaiIcon name="search" :size="14" />
          <input v-model="query" placeholder="搜索标题、内容或标签" />
        </label>
        <button class="plain" type="button" @click="openHistoryPick">从记录存画师串</button>
        <button class="primary" type="button" @click="openCreate"><NaiIcon name="plus" :size="14" /> 新建块</button>
      </header>
      <div v-if="!shown.length" class="empty">
        <NaiIcon name="grid" :size="28" />
        <strong>块库还是空的</strong>
        <p>把常用的一串提示词存成块：画师池、角色、服装、质量词……插进提示词后可以固定、按顺序轮流或随机抽取。</p>
        <div>
          <button class="primary" type="button" @click="openCreate"><NaiIcon name="plus" :size="14" /> 新建块</button>
          <button type="button" @click="fileInput?.click()">导入 TXT...</button>
          <button type="button" @click="importing = true">整理导入...</button>
          <button type="button" @click="openHistoryPick">从记录存画师串</button>
          <button type="button" @click="exportJson">导出 JSON</button>
        </div>
      </div>
      <div v-else class="grid">
        <article v-for="block in shown" :key="block.id">
          <header>
            <strong>{{ block.name }}</strong>
            <button type="button" :class="{ on: block.favorite }" title="收藏" @click="lib.updateBlock(block.id, { favorite: !block.favorite })">
              <NaiIcon name="star" :size="14" />
            </button>
          </header>
          <p><b>基础提示词</b>{{ block.positive || "（空）" }}</p>
          <p class="neg"><b>负面</b>{{ block.negative || "（空）" }}</p>
          <div class="acts">
            <button class="primary" type="button" @click="useBlock(block.id)">填回提示词</button>
            <button type="button" @click="lib.removeBlock(block.id)"><NaiIcon name="trash" :size="14" /></button>
          </div>
        </article>
      </div>
    </section>

    <div v-if="creating" class="mask" @click.self="creating = false">
      <form class="dialog" @submit.prevent="saveBlock">
        <h3>新建块</h3>
        <input v-model="draft.name" placeholder="名称，例如：夕光教室" />
        <textarea v-model="draft.positive" rows="5" placeholder="正面提示词" />
        <textarea v-model="draft.negative" rows="3" placeholder="负面提示词，可空" />
        <div class="acts">
          <button type="button" @click="creating = false">取消</button>
          <button class="primary" type="submit">保存</button>
        </div>
      </form>
    </div>
    <div v-if="pickingHistory" class="mask" @click.self="pickingHistory = false">
      <form class="dialog history-dialog" @submit.prevent="savePickedArtists">
        <h3>从生成记录存画师串</h3>
        <p>勾选记录里的图。整段基础提示词和负面会一起存，填回时两个框一起恢复。</p>
        <div v-if="!historyItems.length" class="hint">生成记录还是空的。</div>
        <div v-else class="history-list">
          <label v-for="item in historyItems" :key="item.id" :class="{ on: historyPicked.includes(item.id) }">
            <input type="checkbox" :checked="historyPicked.includes(item.id)" @change="toggleHistory(item.id)" />
            <img v-if="historyThumbs[item.id]" :src="historyThumbs[item.id]" alt="" />
            <span v-else class="ph" />
            <span class="copy">
              <strong>{{ item.prompt || "（没有基础提示词）" }}</strong>
              <small v-if="item.negativePrompt">负面 {{ item.negativePrompt }}</small>
              <small>{{ item.width }}×{{ item.height }}</small>
            </span>
          </label>
        </div>
        <div class="acts">
          <button type="button" @click="pickingHistory = false">取消</button>
          <button class="primary" type="submit" :disabled="!historyPicked.length">存入当前文件夹</button>
        </div>
      </form>
    </div>
    <div v-if="importing" class="mask" @click.self="importing = false">
      <form class="dialog" @submit.prevent="importParagraphs">
        <h3>整理导入</h3>
        <p>空行分开每一块。第一行不超过 24 字且没有逗号时当作名称，用一行 --- 隔开负面。</p>
        <textarea v-model="importText" rows="10" placeholder="块的名字&#10;1girl, silver hair&#10;---&#10;lowres" />
        <div class="acts">
          <button type="button" @click="importing = false">取消</button>
          <button class="primary" type="submit">导入</button>
        </div>
      </form>
    </div>
  </div>
</template>

<style scoped>
.lib { height: 100%; display: grid; grid-template-columns: 240px 1fr; background: #12141f; color: #fff; }
aside { display: flex; flex-direction: column; gap: 2px; padding: 8px; border-right: 1px solid rgba(255,255,255,.08); background: #16182c; overflow: auto; }
aside header, .label-row { display: flex; align-items: center; gap: 8px; height: 40px; padding: 0 8px; }
.label, .note { margin: 10px 8px 4px; color: rgba(255,255,255,.45); font-size: 12px; }
.note { margin-top: 0; line-height: 1.45; }
.label-row { justify-content: space-between; color: rgba(255,255,255,.55); font-size: 12px; }
.add-folder {
  width: 24px; height: 24px; padding: 0; border: 0; border-radius: 6px;
  display: grid; place-items: center; background: rgba(255,255,255,.08); color: #fff;
}
.folder { justify-content: flex-start; }
.folder span { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.folder em { margin-left: auto; color: rgba(255,255,255,.4); font-style: normal; font-size: 11px; }
aside > button, .foot {
  display: flex; align-items: center; gap: 8px; width: 100%; height: 34px; padding: 0 8px;
  border: 0; border-radius: 8px; background: transparent; color: rgba(255,255,255,.82); text-align: left;
}
aside > button.on { background: rgba(255,255,255,.08); }
.foot { margin-top: auto; }
section { min-width: 0; display: flex; flex-direction: column; }
.bar { display: flex; align-items: center; flex-wrap: wrap; gap: 12px; min-height: 56px; padding: 10px 16px; border-bottom: 1px solid rgba(255,255,255,.08); }
h2, h3 { margin: 0; font-size: 16px; font-family: inherit; }
.search { flex: 1; display: flex; align-items: center; gap: 8px; height: 34px; padding: 0 10px; border-radius: 8px; background: rgba(255,255,255,.06); color: rgba(255,255,255,.45); }
.search input { flex: 1; height: 100%; border: 0; background: transparent; color: #fff; }
.primary, .plain { border-radius: 8px; height: 34px; padding: 0 12px; display: inline-flex; align-items: center; gap: 6px; }
.primary { border: 0; background: #3b82f6; color: #fff; }
.plain { border: 1px solid rgba(255,255,255,.12); background: rgba(255,255,255,.04); color: #fff; }
.empty { flex: 1; display: grid; place-items: center; align-content: center; gap: 10px; text-align: center; padding: 32px; }
.empty p { max-width: 460px; margin: 0; color: rgba(255,255,255,.55); line-height: 1.55; }
.empty div, .acts { display: flex; gap: 8px; flex-wrap: wrap; justify-content: center; }
.empty button, .acts button { height: 34px; padding: 0 12px; border-radius: 8px; border: 1px solid rgba(255,255,255,.12); background: rgba(255,255,255,.04); color: #fff; }
.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 12px; padding: 16px; overflow: auto; }
article { display: flex; flex-direction: column; gap: 8px; padding: 12px; border-radius: 12px; background: #1a1e38; border: 1px solid rgba(255,255,255,.08); }
article header { display: flex; justify-content: space-between; align-items: center; }
article header button { border: 0; background: transparent; color: rgba(255,255,255,.4); }
article header button.on { color: #f5c16c; }
article p, article small { margin: 0; color: rgba(255,255,255,.55); font-size: 12px; }
article p { display: -webkit-box; -webkit-line-clamp: 4; -webkit-box-orient: vertical; overflow: hidden; }
article p b { display: block; margin-bottom: 2px; color: rgba(255,255,255,.4); font-size: 11px; font-weight: 600; }
article p.neg { color: #f0c4c4; }
.mask { position: fixed; inset: 0; z-index: 40; background: rgba(0,0,0,.45); display: grid; place-items: center; padding: 20px; }
.dialog { width: min(520px, 100%); display: flex; flex-direction: column; gap: 10px; padding: 16px; border-radius: 12px; background: #1a1e38; }
.dialog input, .dialog textarea { width: 100%; }
.history-dialog { width: min(640px, 100%); max-height: min(78vh, 720px); }
.history-dialog p, .hint { margin: 0; color: rgba(255,255,255,.55); font-size: 13px; line-height: 1.5; }
.history-dialog code { color: #f5f3c2; }
.history-list { overflow: auto; display: flex; flex-direction: column; gap: 6px; min-height: 0; }
.history-list label { display: grid; grid-template-columns: 16px 48px 1fr; gap: 8px; align-items: center; padding: 6px; border-radius: 8px; background: rgba(255,255,255,.04); }
.history-list label.on { outline: 1px solid #f5f3c2; }
.history-list img, .ph { width: 48px; height: 48px; object-fit: cover; border-radius: 6px; background: rgba(255,255,255,.06); }
.copy { min-width: 0; display: flex; flex-direction: column; gap: 2px; }
.copy strong { font-size: 12px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.copy small { color: rgba(255,255,255,.45); }
</style>
