<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  danbooruBrowse,
  danbooruOnlineTags,
  danbooruRelatedTags,
  danbooruWiki,
  lookupTags,
  translateText,
  type TagSuggestion,
} from "@/api/tauri";
import { DANBOORU_CAT } from "@/utils/tagSuggest";
import { chunkForTranslate, fmtCount, hasCjkText } from "@/utils/textUtils";

const emit = defineEmits<{ insert: [string]; close: [] }>();

const query = ref("");
const localOnly = ref(true);
const hits = ref<TagSuggestion[]>([]);
const related = ref<TagSuggestion[]>([]);
const active = ref("");
const zh = ref<Record<string, string>>({});
const wikiBody = ref("");
const wikiZh = ref("");
const wikiFound = ref(false);
const error = ref("");
const searching = ref(false);
const detailBusy = ref(false);
let searchGen = 0;
let detailGen = 0;
let timer: ReturnType<typeof setTimeout> | null = null;

const activeZh = computed(() => zhOf(active.value));
const wikiText = computed(() => wikiZh.value || wikiBody.value);

function keyOf(name: string) {
  return String(name ?? "").trim().toLowerCase().replace(/\s+/g, " ");
}

function zhOf(name: string) {
  return zh.value[keyOf(name)] || "";
}

function colorOf(category: number) {
  return DANBOORU_CAT[category]?.color || "#94a3b8";
}

function remember(items: TagSuggestion[]) {
  const next = { ...zh.value };
  for (const item of items) {
    const text = String(item?.description ?? "").trim();
    const tag = String(item?.tag ?? "").trim();
    if (text && tag) next[keyOf(tag)] = text;
  }
  zh.value = next;
}

async function fillZh(names: string[]) {
  const missing = [...new Set(names.map((name) => name.trim()).filter(Boolean))].filter((name) => zh.value[keyOf(name)] == null);
  if (!missing.length) return;
  try {
    const hits = await lookupTags(missing);
    const next = { ...zh.value };
    for (const hit of hits) {
      if (hit.found && hit.description.trim()) next[keyOf(hit.tag)] = hit.description.trim();
    }
    zh.value = next;
  } catch {
    /* local library miss still leaves the English tag */
  }
}

async function translateWiki(source: string, gen: number) {
  if (!source.trim() || hasCjkText(source)) {
    if (gen === detailGen) wikiZh.value = source;
    return;
  }
  const parts: string[] = [];
  for (const chunk of chunkForTranslate(source)) {
    try {
      const translated = (await translateText(chunk, "en|zh-CN")).trim();
      parts.push(translated && !/MYMEMORY WARNING/i.test(translated) ? translated : chunk);
    } catch {
      parts.push(chunk);
    }
    if (gen !== detailGen) return;
  }
  wikiZh.value = parts.join("");
}

async function openTag(name: string) {
  const tag = name.trim();
  if (!tag) return;
  active.value = tag;
  const gen = ++detailGen;
  detailBusy.value = true;
  wikiBody.value = "";
  wikiZh.value = "";
  wikiFound.value = false;
  related.value = [];
  try {
    const [rels, page] = await Promise.all([danbooruRelatedTags(tag), danbooruWiki(tag)]);
    if (gen !== detailGen) return;
    related.value = rels;
    wikiFound.value = page.found;
    wikiBody.value = page.body;
    void fillZh(rels.map((item) => item.tag));
    void translateWiki(page.body, gen);
  } catch (err) {
    if (gen === detailGen) error.value = err instanceof Error ? err.message : String(err);
  } finally {
    if (gen === detailGen) detailBusy.value = false;
  }
}

function clearDetail() {
  active.value = "";
  related.value = [];
  wikiBody.value = "";
  wikiZh.value = "";
  wikiFound.value = false;
  detailGen += 1;
}

async function searchNow() {
  const text = query.value.trim();
  const gen = ++searchGen;
  if (!text) {
    hits.value = [];
    error.value = "";
    clearDetail();
    return;
  }
  searching.value = true;
  error.value = "";
  try {
    let rows: TagSuggestion[] = [];
    if (localOnly.value) {
      rows = await danbooruBrowse(text, 80);
    } else {
      const [local, online] = await Promise.allSettled([danbooruBrowse(text, 80), danbooruOnlineTags(text, 40)]);
      if (local.status === "fulfilled" && Array.isArray(local.value)) rows = local.value;
      if (online.status === "fulfilled") {
        const seen = new Set(rows.map((item) => keyOf(item.tag)));
        rows = [...rows, ...online.value.filter((item) => !seen.has(keyOf(item.tag)))];
      }
      if (!rows.length) {
        const reason = [local, online].find((item) => item.status === "rejected");
        if (reason && reason.status === "rejected") throw reason.reason;
      }
    }
    if (gen !== searchGen) return;
    const list = (Array.isArray(rows) ? rows : []).filter((item) => String(item?.tag ?? "").trim());
    hits.value = list;
    if (!list.some((item) => keyOf(item.tag) === keyOf(active.value))) clearDetail();
    remember(list);
    void fillZh(list.map((item) => item.tag));
  } catch (err) {
    if (gen === searchGen) {
      hits.value = [];
      error.value = err instanceof Error ? err.message : String(err);
      clearDetail();
    }
  } finally {
    if (gen === searchGen) searching.value = false;
  }
}

function schedule() {
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => void searchNow(), 180);
}

watch(localOnly, () => {
  if (query.value.trim()) void searchNow();
});

function pick(name: string) {
  void openTag(name);
}

function insert(name: string) {
  const tag = name.trim();
  if (!tag) return;
  emit("insert", tag);
  void openTag(tag);
}

function onSearchKey(e: KeyboardEvent) {
  if (e.key === "Tab" && hits.value.length) {
    e.preventDefault();
    insert(active.value || hits.value[0].tag);
  } else if (e.key === "Enter") {
    e.preventDefault();
    if (active.value) insert(active.value);
    else if (hits.value[0]) insert(hits.value[0].tag);
  }
}

function wikiHref(name: string) {
  const slug = name.trim().toLowerCase().replace(/\s+/g, "_");
  return `https://danbooru.donmai.us/wiki_pages/${encodeURIComponent(slug)}`;
}
</script>

<template>
  <Teleport to="body">
  <div class="mask">
  <section class="explorer" tabindex="-1" @mousedown.stop @keydown.esc="emit('close')">
    <header class="top">
      <strong>标签扩展</strong>
      <button type="button" class="chip" :class="{ on: localOnly }" @click="localOnly = !localOnly">仅本地</button>
      <span class="grow" />
      <button type="button" class="x" @click="emit('close')">×</button>
    </header>
    <input v-model="query" placeholder="搜索标签，例如 fate" @input="schedule" @keydown="onSearchKey" />
    <p class="meta">
      <span v-if="active">Tab 补全：{{ active }}</span>
      <span v-else>单击查看百科，双击插入</span>
      <span>结果数量 {{ hits.length }}</span>
    </p>
    <div class="split">
      <div class="col">
        <p v-if="searching" class="empty">搜索中…</p>
        <div v-else-if="error" class="blank warn">
          <strong>{{ error }}</strong>
        </div>
        <div v-else-if="query.trim() && !hits.length" class="blank">
          <strong>没有匹配的标签</strong>
          <span>换个关键词，或关闭「仅本地」后再搜</span>
        </div>
        <button
          v-for="item in hits"
          :key="item.tag"
          type="button"
          class="row"
          :class="{ on: keyOf(item.tag) === keyOf(active) }"
          @click="pick(item.tag)"
          @dblclick="insert(item.tag)"
        >
          <i :style="{ background: colorOf(item.category) }" />
          <b>{{ item.tag }}</b>
          <small v-if="zhOf(item.tag)">{{ zhOf(item.tag) }}</small>
          <em v-if="item.count">{{ fmtCount(item.count) }}</em>
        </button>
      </div>
      <div class="col side">
        <header>
          <strong>关联标签</strong>
          <span v-if="active">{{ active }}</span>
        </header>
        <p v-if="!active" class="empty">选一个标签后显示经常一起出现的词</p>
        <p v-else-if="detailBusy && !related.length" class="empty">读取中…</p>
        <p v-else-if="active && !related.length" class="empty">没有关联标签</p>
        <button
          v-for="item in related"
          :key="item.tag"
          type="button"
          class="row"
          @click="pick(item.tag)"
          @dblclick="insert(item.tag)"
        >
          <i :style="{ background: colorOf(item.category) }" />
          <b>{{ item.tag }}</b>
          <small v-if="zhOf(item.tag)">{{ zhOf(item.tag) }}</small>
          <em v-if="item.count">{{ fmtCount(item.count) }}</em>
        </button>
      </div>
    </div>
    <article class="wiki">
      <header>
        <strong>标签小百科</strong>
        <span v-if="active">{{ active }}<template v-if="activeZh"> · {{ activeZh }}</template></span>
        <span class="grow" />
        <a v-if="active" :href="wikiHref(active)" target="_blank" rel="noreferrer">原页面</a>
      </header>
      <p v-if="!active" class="empty">选中标签后显示 Danbooru 百科的中文</p>
      <p v-else-if="detailBusy && !wikiText" class="empty">读取百科…</p>
      <p v-else-if="!wikiFound" class="empty">这个标签没有百科</p>
      <p v-else class="body">{{ wikiText }}</p>
    </article>
  </section>
  </div>
  </Teleport>
</template>

<style scoped>
.mask {
  position: fixed;
  inset: 0;
  z-index: 70;
  background: rgba(6, 8, 18, 0.55);
}
.explorer {
  position: fixed;
  z-index: 71;
  left: 50%;
  top: 50%;
  transform: translate(-50%, -50%);
  width: min(980px, calc(100vw - 48px));
  height: min(720px, calc(100vh - 48px));
  display: flex;
  flex-direction: column;
  gap: 12px;
  padding: 16px 18px;
  overflow: hidden;
  border-radius: 14px;
  background: #14162b;
  border: 1px solid #2b2e4a;
  box-shadow: 0 24px 60px rgba(0,0,0,0.45);
}
.top, .wiki header, .side header {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}
.top strong, .wiki header strong, .side header strong { font-size: 13px; }
.grow { flex: 1; }
.chip, .x {
  height: 24px;
  border: 0;
  border-radius: 999px;
  background: #1f2138;
  color: rgba(255,255,255,0.72);
  font-size: 12px;
}
.chip { padding: 0 10px; }
.chip.on { background: #2e3152; color: var(--heading); }
.x { width: 24px; }
input {
  width: 100%;
  height: 32px;
  padding: 0 10px;
  border-radius: 8px;
  background: #0e1020;
}
.meta, .empty {
  margin: 0;
  font-size: 11px;
  color: rgba(255,255,255,0.46);
}
.meta { display: flex; justify-content: space-between; gap: 8px; }
.blank {
  min-height: 180px;
  display: grid;
  align-content: center;
  justify-items: center;
  gap: 8px;
  padding: 24px 16px;
  text-align: center;
}
.blank strong { color: #fff; font-size: 15px; font-weight: 650; }
.blank.warn strong { color: #ff8d7a; }
.blank span { color: rgba(255,255,255,0.5); font-size: 12px; }
.split {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(0, 1.4fr) minmax(280px, 0.8fr);
  gap: 16px;
}
.col {
  min-width: 0;
  min-height: 0;
  overflow: auto;
  display: grid;
  align-content: start;
  gap: 2px;
  padding: 4px;
  border-radius: 10px;
  background: #0e1020;
}
.side header span, .wiki header span {
  color: rgba(255,255,255,0.5);
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.row {
  width: 100%;
  min-width: 0;
  min-height: 36px;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 6px 10px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: #fff;
  text-align: left;
  overflow: hidden;
}
.row.on, .row:hover { background: #262948; }
.row i {
  flex: 0 0 8px;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  align-self: center;
}
.row b {
  flex: 0 1 auto;
  font-size: 13px;
  font-weight: 700;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  min-width: 0;
}
.row small {
  flex: 1;
  min-width: 0;
  color: rgba(255,255,255,0.5);
  font-size: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.row em { flex: 0 0 auto; color: rgba(255,255,255,0.48); font-size: 12px; font-style: normal; margin-left: auto; white-space: nowrap; }
.wiki {
  display: grid;
  gap: 6px;
  padding-top: 4px;
  border-top: 1px solid #2b2e4a;
}
.wiki a { color: #8eb6ff; font-size: 12px; }
.body {
  margin: 0;
  max-height: 160px;
  overflow: auto;
  white-space: pre-wrap;
  line-height: 1.6;
  font-size: 13px;
  color: rgba(255,255,255,0.86);
}
@media (max-width: 800px) {
  .explorer {
    width: calc(100vw - 16px);
    height: calc(100vh - 16px);
  }
  .split { grid-template-columns: 1fr; }
}
</style>
