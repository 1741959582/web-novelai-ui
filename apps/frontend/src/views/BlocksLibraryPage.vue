<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { useRouter } from "vue-router";
import { useAppStore } from "@/stores/app";
import { useMaterialLibrary } from "@/stores/materialLibrary";
import { appendPromptChunk } from "@/utils/textUtils";
import NaiIcon from "@/components/NaiIcon.vue";

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
  if (block.positive) app.params.positivePrompt = appendPromptChunk(app.params.positivePrompt, block.positive);
  if (block.negative) app.params.negativePrompt = appendPromptChunk(app.params.negativePrompt, block.negative);
  lib.touchBlock(id);
  app.status = `已插入提示词块「${block.name}」`;
  void router.push("/");
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
        <button type="button" title="新建文件夹" @click="addFolder"><NaiIcon name="plus" :size="13" /></button>
      </div>
      <button type="button" :class="{ on: view === 'folder' && !folderId }" @click="pickFolder('')">未分类</button>
      <button v-for="folder in lib.folders" :key="folder.id" type="button" :class="{ on: view === 'folder' && folderId === folder.id }" @click="pickFolder(folder.id)">
        {{ folder.name }}
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
          <p>{{ block.positive || "（没有正面）" }}</p>
          <small v-if="block.negative">负面 {{ block.negative }}</small>
          <div class="acts">
            <button class="primary" type="button" @click="useBlock(block.id)">插入草稿</button>
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
.label-row button, aside > button, .foot {
  display: flex; align-items: center; gap: 8px; width: 100%; height: 34px; padding: 0 8px;
  border: 0; border-radius: 8px; background: transparent; color: rgba(255,255,255,.82); text-align: left;
}
aside > button.on { background: rgba(255,255,255,.08); }
.foot { margin-top: auto; }
section { min-width: 0; display: flex; flex-direction: column; }
.bar { display: flex; align-items: center; gap: 12px; height: 56px; padding: 0 16px; border-bottom: 1px solid rgba(255,255,255,.08); }
h2, h3 { margin: 0; font-size: 16px; font-family: inherit; }
.search { flex: 1; display: flex; align-items: center; gap: 8px; height: 34px; padding: 0 10px; border-radius: 8px; background: rgba(255,255,255,.06); color: rgba(255,255,255,.45); }
.search input { flex: 1; height: 100%; border: 0; background: transparent; color: #fff; }
.primary { border: 0; border-radius: 8px; background: #3b82f6; color: #fff; height: 34px; padding: 0 12px; display: inline-flex; align-items: center; gap: 6px; }
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
.mask { position: fixed; inset: 0; z-index: 40; background: rgba(0,0,0,.45); display: grid; place-items: center; padding: 20px; }
.dialog { width: min(520px, 100%); display: flex; flex-direction: column; gap: 10px; padding: 16px; border-radius: 12px; background: #1a1e38; }
.dialog input, .dialog textarea { width: 100%; }
</style>
