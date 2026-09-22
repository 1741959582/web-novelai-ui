<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import {
  lookupTags,
  quicktagCatalog,
  quicktagEntry,
  quicktagSearch,
  type QuickCategory,
  type QuickCollection,
  type QuickEntry,
  type TagLookup,
} from "@/api/tauri";
import { useAppStore } from "@/stores/app";
import { parseWeightedTag, splitPromptTags } from "@/utils/promptWeight";
import { danbooruCategory } from "@/utils/tagSuggest";

interface PromptChip {
  raw: string;
  key: string;
  zh: string;
  found: boolean;
  category: number;
  count: number;
}

const store = useAppStore();
const router = useRouter();
const collections = ref<QuickCollection[]>([]);
const collectionId = ref("suozhang");
const query = ref("");
const path = ref<string[]>([]);
const page = ref(1);
const safeOnly = ref(true);
const loading = ref(false);
const loadingMore = ref(false);
const error = ref("");
const title = ref("法典图鉴");
const total = ref(0);
const items = ref<QuickEntry[]>([]);
const categories = ref<QuickCategory[]>([]);
const copiedId = ref("");
const copiedMsg = ref("");
const detail = ref<QuickEntry | null>(null);
const detailBusy = ref(false);
const imageIndex = ref(0);
const zoomOpen = ref(false);
const showZh = ref(false);
const picked = ref<PromptChip | null>(null);
const lookups = ref<Record<string, TagLookup>>({});
const skeletonCards = Array.from({ length: 16 }, (_, i) => i);
const skeletonCats = Array.from({ length: 6 }, (_, i) => i);
const selecting = ref(false);
const selected = ref<string[]>([]);
const promptDraft = ref("");
const ucDraft = ref("");
const folded = ref(false);
const scroller = ref<HTMLElement | null>(null);
const sentinel = ref<HTMLElement | null>(null);
let observer: IntersectionObserver | null = null;

const current = computed(() => collections.value.find((c) => c.id === collectionId.value));
const crumbs = computed(() => path.value);
const childCats = computed(() => {
  const depth = path.value.length + 1;
  return categories.value.filter((c) => c.path.length === depth && path.value.every((p, i) => c.path[i] === p));
});
const activeImage = computed(() => {
  const images = detail.value?.images ?? [];
  return images[imageIndex.value] || images[0] || null;
});
const positiveChips = computed(() => chipsFor(promptDraft.value || detail.value?.prompt || ""));
const detailIndex = computed(() => items.value.findIndex((item) => item.id === detail.value?.id));
const neighborPrev = computed(() => items.value[detailIndex.value - 1] || null);
const neighborNext = computed(() => items.value[detailIndex.value + 1] || null);
const selectedText = computed(() => selected.value.join(", "));

function chipsFor(text: string): PromptChip[] {
  return splitPromptTags(text).flatMap((raw) => {
    const parsed = parseWeightedTag(raw);
    const keys = parsed.parts.length ? parsed.parts : [parsed.core || raw];
    return keys.map((key) => {
      const hit = lookups.value[normKey(key)];
      return {
        raw: keys.length > 1 ? key : raw,
        key,
        zh: hit?.description || "",
        found: Boolean(hit?.found),
        category: hit?.category ?? 0,
        count: hit?.count ?? 0,
      };
    });
  });
}

function normKey(value: string) {
  return value.trim().toLowerCase().replace(/_/g, " ");
}

function entryPromptText(entry: QuickEntry) {
  return [entry.prompt, ...entry.characterPrompts.map((c) => c.prompt)].map((s) => s.trim()).filter(Boolean).join("\n");
}

function combinedPrompt(entry: QuickEntry) {
  const sections: string[] = [];
  if (entry.prompt.trim()) sections.push(entry.prompt.trim());
  for (const item of entry.characterPrompts) {
    if (item.prompt.trim()) sections.push(`${item.label || "char"}:\n${item.prompt.trim()}`);
  }
  if (entry.negative.trim()) sections.push(`Negative:\n${entry.negative.trim()}`);
  for (const item of entry.characterPrompts) {
    if (item.negative?.trim()) sections.push(`${item.label || "char"} Negative:\n${item.negative.trim()}`);
  }
  return sections.join("\n\n");
}

async function copyText(text: string, message: string, id = "") {
  const value = text.trim();
  if (!value) return;
  try {
    await navigator.clipboard.writeText(value);
    copiedId.value = id;
    copiedMsg.value = message;
    store.status = message;
    window.setTimeout(() => {
      if (copiedId.value === id) copiedId.value = "";
    }, 900);
  } catch {
    store.status = "复制失败，请手动选择文本";
  }
}

async function loadCatalog() {
  error.value = "";
  try {
    const data = await quicktagCatalog(safeOnly.value);
    collections.value = data.collections;
    if (!collections.value.some((c) => c.id === collectionId.value)) {
      collectionId.value = collections.value[0]?.id || "suozhang";
    }
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  }
}

async function loadPage(reset = false) {
  if (!collectionId.value || loading.value || loadingMore.value) return;
  if (reset) page.value = 1;
  const appending = !reset && page.value > 1;
  if (appending) loadingMore.value = true;
  else loading.value = true;
  error.value = "";
  try {
    const data = await quicktagSearch({
      collectionId: collectionId.value,
      query: query.value,
      path: path.value,
      page: page.value,
      pageSize: 48,
      safeOnly: safeOnly.value,
    });
    title.value = data.collectionTitle || "法典图鉴";
    total.value = data.total;
    items.value = appending ? [...items.value, ...data.items] : data.items;
    categories.value = data.categories;
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
    if (!appending) items.value = [];
    if (appending && page.value > 1) page.value -= 1;
  } finally {
    loading.value = false;
    loadingMore.value = false;
  }
}

function loadMore() {
  if (loading.value || loadingMore.value || items.value.length >= total.value) return;
  page.value += 1;
  void loadPage(false);
}

function bindScroll() {
  observer?.disconnect();
  if (!scroller.value || !sentinel.value) return;
  observer = new IntersectionObserver((entries) => {
    if (entries.some((entry) => entry.isIntersecting)) loadMore();
  }, { root: scroller.value, rootMargin: "320px" });
  observer.observe(sentinel.value);
}

watch([items, loading, loadingMore], async () => {
  await nextTick();
  bindScroll();
});

function setCollection(id: string) {
  collectionId.value = id;
  path.value = [];
  page.value = 1;
  query.value = "";
}

function openCat(next: string[]) {
  path.value = next;
  void loadPage(true);
}

function searchNow() {
  void loadPage(true);
}

async function rememberLookups(entry: QuickEntry) {
  const names = [
    ...splitPromptTags(entry.prompt),
    ...splitPromptTags(entry.negative),
    ...entry.characterPrompts.flatMap((c) => [...splitPromptTags(c.prompt), ...splitPromptTags(c.negative || "")]),
  ].flatMap((raw) => parseWeightedTag(raw).parts);
  const unique = [...new Set(names.map((name) => name.trim()).filter(Boolean))];
  const missing = unique.filter((name) => !(normKey(name) in lookups.value));
  if (!missing.length) return;
  try {
    const hits = await lookupTags(missing);
    const next = { ...lookups.value };
    for (const hit of hits) next[normKey(hit.tag)] = hit;
    lookups.value = next;
  } catch {
    /* keep English-only chips */
  }
}

async function openDetail(entry: QuickEntry) {
  detailBusy.value = true;
  imageIndex.value = 0;
  zoomOpen.value = false;
  picked.value = null;
  selecting.value = false;
  selected.value = [];
  try {
    const full = await quicktagEntry(entry.collectionId, entry.id).catch(() => entry);
    detail.value = full;
    promptDraft.value = full.prompt;
    ucDraft.value = full.negative;
    await rememberLookups(full);
  } finally {
    detailBusy.value = false;
  }
}

function closeDetail() {
  detail.value = null;
  zoomOpen.value = false;
  picked.value = null;
  selecting.value = false;
  selected.value = [];
}

function stepDetail(delta: number) {
  const next = items.value[detailIndex.value + delta];
  if (next) void openDetail(next);
}

function stepLightbox(delta: number) {
  const images = detail.value?.images ?? [];
  const nextImg = imageIndex.value + delta;
  if (nextImg >= 0 && nextImg < images.length) {
    imageIndex.value = nextImg;
    return;
  }
  stepDetail(delta);
}

function toggleSelect(chip: PromptChip) {
  const key = chip.raw;
  selected.value = selected.value.includes(key)
    ? selected.value.filter((item) => item !== key)
    : [...selected.value, key];
}

function pickChip(chip: PromptChip) {
  if (selecting.value) {
    toggleSelect(chip);
    return;
  }
  picked.value = picked.value?.raw === chip.raw && picked.value.key === chip.key ? null : chip;
}

async function importEntry(entry: QuickEntry) {
  store.importCodexEntry(entry);
  closeDetail();
  await router.push("/");
}

function onKey(ev: KeyboardEvent) {
  if (!detail.value) return;
  if (ev.key === "Escape") {
    if (zoomOpen.value) zoomOpen.value = false;
    else closeDetail();
  }
  if (zoomOpen.value) return;
  if (ev.key === "ArrowLeft") stepLightbox(-1);
  if (ev.key === "ArrowRight") stepLightbox(1);
}

watch(safeOnly, async () => {
  await loadCatalog();
  await loadPage(true);
});

watch(collectionId, () => {
  void loadPage(true);
});

onMounted(async () => {
  window.addEventListener("keydown", onKey);
  await loadCatalog();
  await loadPage(true);
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKey);
  observer?.disconnect();
});
</script>

<template>
  <div class="wrap">
    <header class="head">
      <h2>法典图鉴</h2>
      <label class="safe">
        <input v-model="safeOnly" type="checkbox" />
        全年龄
      </label>
    </header>

    <div class="bar">
      <select :value="collectionId" @change="setCollection(($event.target as HTMLSelectElement).value)">
        <option v-for="item in collections" :key="item.id" :value="item.id">
          {{ item.title }} · {{ item.entryCount }}
        </option>
      </select>
      <input v-model="query" placeholder="搜索标题 / 提示词，空格同时满足，-词排除" @keydown.enter="searchNow" />
      <button type="button" class="act" @click="searchNow">搜索</button>
    </div>

    <nav v-if="current || crumbs.length" class="crumbs">
      <button type="button" @click="openCat([])">{{ title }}</button>
      <button v-for="(part, i) in crumbs" :key="`${part}-${i}`" type="button" @click="openCat(crumbs.slice(0, i + 1))">
        / {{ part }}
      </button>
      <span>{{ total }} 条</span>
    </nav>

    <div v-if="childCats.length" class="cats">
      <button v-for="cat in childCats" :key="cat.path.join('/')" type="button" @click="openCat(cat.path)">
        {{ cat.path[cat.path.length - 1] }}
        <em>{{ cat.count }}</em>
      </button>
    </div>

    <div v-if="loading && !childCats.length" class="cats" aria-hidden="true">
      <span v-for="n in skeletonCats" :key="`cat-${n}`" class="sk sk-cat" />
    </div>

    <p v-if="error" class="err">{{ error }}</p>
    <p v-else-if="loading" class="hint">正在从法典接口拉取数据…</p>
    <p v-else-if="!items.length" class="hint">这部法典没有匹配的词条</p>

    <div ref="scroller" class="scroller">
    <div v-if="loading" class="grid" aria-busy="true" aria-label="法典加载中">
      <article v-for="n in skeletonCards" :key="`sk-${n}`" class="card sk-card">
        <div class="sk sk-thumb" />
        <div class="card-acts">
          <span class="sk sk-btn" />
          <span class="sk sk-btn" />
        </div>
        <div class="meta">
          <span class="sk sk-line" />
          <span class="sk sk-line short" />
        </div>
      </article>
    </div>

    <div v-else class="grid">
      <article v-for="entry in items" :key="entry.id" class="card" :class="{ copied: copiedId === entry.id }">
        <button type="button" class="thumb" :title="copiedId === entry.id ? '已复制' : '点击复制提示词'" @click="copyText(entryPromptText(entry), `已复制正向${entry.characterPrompts.length ? `（含 ${entry.characterPrompts.length} 组角色词）` : ''}：${entry.title || entry.id}`, entry.id)">
          <img v-if="entry.coverUrl" :src="entry.coverUrl" :alt="entry.title" referrerpolicy="no-referrer" />
          <div v-else class="ph">无图</div>
          <span v-if="copiedId === entry.id" class="flash">已复制</span>
        </button>
        <div class="card-acts">
          <button type="button" @click="openDetail(entry)">放大</button>
          <button type="button" @click="copyText(entryPromptText(entry), `已复制正向：${entry.title || entry.id}`, entry.id)">复制</button>
          <button v-if="entry.negative" type="button" @click="copyText(entry.negative, `已复制负面：${entry.title || entry.id}`, `${entry.id}-neg`)">负面</button>
        </div>
        <div class="meta">
          <b>{{ entry.title || entry.id }}</b>
          <small>{{ entry.path.join(" / ") }}</small>
        </div>
      </article>
    </div>

    <div ref="sentinel" class="sentinel" />
    <p v-if="loadingMore" class="hint more">正在加载下一页…</p>
    <p v-else-if="items.length && items.length < total" class="hint more">拉到底继续加载</p>
    <p v-else-if="items.length && items.length >= total" class="hint more">已经到底了 · {{ items.length }} 条</p>
    </div>

    <div v-if="detail" class="lb" :class="{ folded }" role="dialog" aria-modal="true" aria-label="图片灯箱" @click.self="closeDetail">
      <img v-if="neighborPrev?.coverUrl" class="lb-ghost left" :src="neighborPrev.coverUrl" alt="" referrerpolicy="no-referrer" />
      <img v-if="neighborNext?.coverUrl" class="lb-ghost right" :src="neighborNext.coverUrl" alt="" referrerpolicy="no-referrer" />
      <button class="lb-circle close" type="button" aria-label="关闭" @click="closeDetail">×</button>
      <button class="lb-circle nav prev" type="button" aria-label="上一条" :disabled="detailIndex <= 0 && imageIndex <= 0" @click="stepLightbox(-1)">‹</button>
      <button class="lb-circle nav next" type="button" aria-label="下一条" :disabled="detailIndex >= items.length - 1 && imageIndex >= (detail.images.length || 1) - 1" @click="stepLightbox(1)">›</button>
      <button type="button" class="lb-shot" title="点击看原图" @click="zoomOpen = true">
        <img v-if="activeImage" :src="activeImage.previewUrl || activeImage.originalUrl" :alt="detail.title" referrerpolicy="no-referrer" />
        <div v-else class="ph tall">无图</div>
      </button>
      <div v-if="(detail.images?.length || 0) > 1" class="lb-thumbs">
        <button v-for="(img, i) in detail.images" :key="img.previewUrl + i" type="button" :class="{ on: i === imageIndex }" @click="imageIndex = i">
          <img :src="img.previewUrl" alt="" referrerpolicy="no-referrer" />
        </button>
      </div>
      <button class="lb-fold" type="button" :title="folded ? '展开信息栏' : '收起信息栏'" @click="folded = !folded">{{ folded ? "‹" : "›" }}</button>
      <aside v-show="!folded" class="lb-info" @click.stop>
        <h3>{{ detail.title || detail.id }}</h3>
        <p class="lb-meta">
          {{ imageIndex + 1 }} / {{ Math.max(detail.images.length, 1) }}
          <template v-if="detailIndex >= 0"> · 第 {{ detailIndex + 1 }} / {{ total || items.length }} 条</template>
          <template v-if="detail.path.length"> · {{ detail.path.join(" › ") }}</template>
        </p>
        <p class="params">
          <span v-if="detail.model">{{ detail.model }}</span>
          <span v-if="detail.sampler">{{ detail.sampler }}</span>
          <span v-if="detail.steps">{{ detail.steps }} steps</span>
          <span v-if="detail.cfgScale">CFG {{ detail.cfgScale }}</span>
          <span v-if="detail.seed">seed {{ detail.seed }}</span>
          <span v-if="detail.width && detail.height">{{ detail.width }}×{{ detail.height }}</span>
        </p>
        <div class="lb-actions">
          <button type="button" @click="copyText(combinedPrompt(detail), `已复制全部：${detail.title || detail.id}`)">复制全部</button>
          <button type="button" @click="copyText(detail.sourceUrl, '已复制分享链接')">分享</button>
          <button type="button" class="primary" :disabled="detailBusy" @click="importEntry(detail)">导入到生成</button>
        </div>

        <div class="section-head">
          <b>正向 tags</b>
          <label class="zh"><input v-model="showZh" type="checkbox" /> 中文对照</label>
          <button type="button" :disabled="!promptDraft.trim()" @click="copyText(promptDraft, `已复制正向：${detail.title || detail.id}`)">复制正向</button>
          <button type="button" :class="{ on: selecting }" @click="selecting = !selecting; selected = []; picked = null">{{ selecting ? "取消选择" : "选择 tag" }}</button>
        </div>
        <textarea
          v-if="!showZh && !selecting"
          v-model="promptDraft"
          class="prompt-input"
          spellcheck="false"
          rows="6"
          placeholder="这条没有可复制的正向 tags"
        />
        <div v-else class="prompt-box" :class="{ zh: showZh, selecting }">
          <button
            v-for="chip in positiveChips"
            :key="`p-${chip.raw}-${chip.key}`"
            type="button"
            class="tok"
            :class="{ on: selecting ? selected.includes(chip.raw) : picked?.raw === chip.raw && picked.key === chip.key, miss: showZh && !chip.found }"
            @click="pickChip(chip)"
          >
            <span>{{ chip.raw }}</span>
            <em v-if="showZh && chip.zh">{{ chip.zh }}</em>
          </button>
        </div>
        <div v-if="selecting" class="select-bar">
          <span>已选 {{ selected.length }} 项</span>
          <button type="button" :disabled="!selected.length" @click="copyText(selectedText, `已复制 ${selected.length} 项：${detail.title || detail.id}`)">复制所选</button>
        </div>
        <div v-if="picked && !selecting" class="tag-detail">
          <div>
            <b>{{ picked.key }}</b>
            <small>{{ picked.found ? picked.zh || danbooruCategory(picked.category).label : "词库里没有这条" }}</small>
          </div>
          <p>
            {{ danbooruCategory(picked.category).label }}
            <template v-if="picked.count"> · {{ picked.count }} 次</template>
          </p>
          <div class="row">
            <button type="button" @click="copyText(picked.raw, `已复制标签：${picked.key}`)">复制这个 tag</button>
            <a :href="`https://danbooru.donmai.us/wiki_pages/${encodeURIComponent(picked.key.replace(/ /g, '_'))}`" target="_blank" rel="noreferrer">Danbooru 维基</a>
          </div>
        </div>

        <template v-for="(ch, i) in detail.characterPrompts" :key="`c-${i}`">
          <div class="section-head">
            <b>{{ ch.label || `角色 ${i + 1}` }}</b>
            <button type="button" @click="copyText(ch.prompt, `已复制${ch.label || `角色 ${i + 1}`}：${detail.title || detail.id}`)">复制</button>
          </div>
          <textarea :value="ch.prompt" class="prompt-input slim" readonly rows="3" spellcheck="false" />
          <template v-if="ch.negative?.trim()">
            <div class="section-head">
              <b>{{ ch.label || `角色 ${i + 1}` }} 负面</b>
              <button type="button" @click="copyText(ch.negative || '', `已复制${ch.label || `角色 ${i + 1}`}负面`)">复制</button>
            </div>
            <textarea :value="ch.negative" class="prompt-input slim" readonly rows="2" spellcheck="false" />
          </template>
        </template>

        <template v-if="detail.negative || ucDraft">
          <div class="section-head">
            <b>负面</b>
            <button type="button" @click="copyText(ucDraft, `已复制负面：${detail.title || detail.id}`)">复制负面</button>
          </div>
          <textarea v-model="ucDraft" class="prompt-input slim" spellcheck="false" rows="4" />
        </template>
        <p v-if="detail.note" class="note">{{ detail.note }}</p>
      </aside>
    </div>

    <div v-if="zoomOpen && activeImage" class="zoom" @click.self="zoomOpen = false">
      <button class="lb-circle close" type="button" @click="zoomOpen = false">×</button>
      <img :src="activeImage.originalUrl || activeImage.previewUrl" :alt="detail?.title || ''" referrerpolicy="no-referrer" />
    </div>
  </div>
</template>

<style scoped>
.wrap { padding: 12px 16px 0; height: 100%; min-height: 0; display: flex; flex-direction: column; overflow: hidden; }
.head { flex: none; display: flex; justify-content: space-between; gap: 12px; align-items: center; }
.scroller { flex: 1; min-height: 0; overflow: auto; padding-bottom: 16px; }
.sentinel { height: 1px; }
.bar, .crumbs, .cats { flex: none; }
h2 { margin: 0 0 6px; font-size: 18px; }
h3 { margin: 0 0 4px; font-size: 18px; }
p, .hint, small { margin: 0; color: rgba(255,255,255,0.5); font-size: 12px; line-height: 1.5; }
.safe { display: flex; align-items: center; gap: 6px; color: rgba(255,255,255,0.72); font-size: 12px; }
.bar, .pager, .row, .section-head, .info-nav { display: flex; gap: 8px; align-items: center; flex-wrap: nowrap; }
.bar { overflow-x: auto; }
.bar input { min-width: 0; }
.bar, .pager { margin: 12px 0; }
select, .bar input, .act, .pager button, .crumbs button, .cats button, .card-acts button, .section-head button, .row button, .info-nav button, .tag-detail button, .lb-actions button, .select-bar button {
  background: #16182d;
  border: 0;
  border-radius: 8px;
  color: #fff;
  padding: 8px 10px;
}
.bar input { flex: 1; }
select { max-width: 42%; }
.act, .card-acts button, .section-head button, .row button, .info-nav button, .lb-actions button, .select-bar button { background: #2e3152; color: var(--heading); }
.crumbs { display: flex; flex-wrap: wrap; gap: 6px; align-items: center; color: rgba(255,255,255,0.45); font-size: 12px; }
.cats { display: flex; flex-wrap: wrap; gap: 6px; margin-bottom: 10px; }
.cats em { font-style: normal; opacity: 0.55; margin-left: 4px; }
.err { color: var(--danger); }
.hint.more { text-align: center; padding: 16px 0 8px; }
.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); gap: 10px; }
.card {
  display: flex;
  flex-direction: column;
  gap: 6px;
  overflow: hidden;
  border-radius: 10px;
  background: #16182d;
}
.card.copied { outline: 1px solid #9ae6b4; }
.sk {
  display: block;
  border-radius: 8px;
  background: linear-gradient(90deg, #1a1c33 25%, #262a48 50%, #1a1c33 75%);
  background-size: 200% 100%;
  animation: shimmer 1.2s ease-in-out infinite;
}
.sk-card { pointer-events: none; }
.sk-thumb { width: 100%; aspect-ratio: 3 / 4; border-radius: 0; }
.sk-btn { flex: 1; height: 28px; }
.sk-line { height: 12px; width: 78%; margin-top: 4px; }
.sk-line.short { width: 52%; height: 10px; }
.sk-cat { width: 88px; height: 32px; border-radius: 8px; }
@keyframes shimmer {
  0% { background-position: 100% 0; }
  100% { background-position: -100% 0; }
}
@media (prefers-reduced-motion: reduce) {
  .sk { animation: none; background: #1f223c; }
}
.thumb { position: relative; padding: 0; border: 0; background: #101226; color: #fff; }
.card img, .ph {
  width: 100%;
  aspect-ratio: 3 / 4;
  object-fit: cover;
  background: #101226;
}
.ph, .ph.tall { display: grid; place-items: center; color: rgba(255,255,255,0.35); }
.ph.tall { min-height: 48vh; aspect-ratio: auto; }
.flash {
  position: absolute;
  inset: auto 8px 8px auto;
  background: #9ae6b4;
  color: #0e0f21;
  border-radius: 999px;
  padding: 3px 8px;
  font-size: 11px;
  font-weight: 700;
}
.card-acts { display: flex; gap: 6px; padding: 0 8px; }
.card-acts button { flex: 1; padding: 6px 8px; font-size: 12px; }
.meta { padding: 0 8px 8px; }
.meta b { display: block; font-size: 12px; color: #fff; }
.pager { justify-content: center; }
.pager button:disabled, .lb-circle:disabled { opacity: 0.35; }
.lb, .zoom {
  position: fixed;
  inset: 0;
  z-index: 60;
  background: rgba(12, 13, 24, 0.82);
  display: grid;
  place-items: center;
}
.lb-ghost {
  position: absolute;
  top: 50%;
  transform: translateY(-50%);
  width: min(280px, 22vw);
  height: 70vh;
  object-fit: cover;
  filter: blur(18px) saturate(0.7);
  opacity: 0.28;
  pointer-events: none;
  border-radius: 16px;
}
.lb-ghost.left { left: 0; }
.lb-ghost.right { right: 0; }
.lb-circle {
  position: absolute;
  z-index: 3;
  width: 42px;
  height: 42px;
  border: 0;
  border-radius: 50%;
  background: rgba(18, 20, 40, 0.88);
  color: #fff;
  font-size: 22px;
  box-shadow: 0 8px 24px rgba(0,0,0,0.28);
}
.lb-circle.close { top: 16px; left: 16px; }
.lb-circle.nav.prev { left: 16px; top: 50%; transform: translateY(-50%); }
.lb-circle.nav.next { right: 348px; top: 50%; transform: translateY(-50%); }
.lb.folded .nav.next { right: 16px; }
.lb.folded .lb-fold { right: 16px; border-radius: 8px; }
.lb-shot {
  max-width: min(58vw, 820px);
  max-height: 92vh;
  padding: 0;
  border: 0;
  background: transparent;
  border-radius: 8px;
  overflow: hidden;
}
.lb-shot img { max-width: min(58vw, 820px); max-height: 92vh; object-fit: contain; display: block; }
.lb-thumbs {
  position: absolute;
  left: 50%;
  bottom: 16px;
  transform: translateX(-50%);
  display: flex;
  gap: 6px;
  z-index: 3;
}
.lb-thumbs button { width: 42px; height: 56px; padding: 0; border: 1px solid transparent; border-radius: 6px; overflow: hidden; background: #101226; }
.lb-thumbs button.on { border-color: var(--heading); }
.lb-thumbs img { width: 100%; height: 100%; object-fit: cover; }
.lb-fold {
  position: absolute;
  right: 332px;
  top: 50%;
  transform: translateY(-50%);
  z-index: 4;
  width: 22px;
  height: 48px;
  border: 0;
  border-radius: 8px 0 0 8px;
  background: #191b31;
  color: #fff;
}
.lb-info {
  position: absolute;
  top: 16px;
  right: 16px;
  bottom: 16px;
  width: min(320px, 34vw);
  overflow: auto;
  background: #191b31;
  border: 1px solid #2a2e4d;
  border-radius: 16px;
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  box-shadow: 0 18px 50px rgba(0,0,0,0.35);
}
.lb-meta { color: rgba(255,255,255,0.5); font-size: 12px; }
.lb-actions, .select-bar { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
.section-head { justify-content: flex-start; gap: 6px; }
.section-head b { color: #fff; margin-right: auto; }
.section-head button.on { background: #3d4270; }
.zh { display: flex; align-items: center; gap: 4px; color: rgba(255,255,255,0.6); font-size: 12px; white-space: nowrap; }
.zh input { accent-color: #f5f3c2; width: 14px; height: 14px; }
.prompt-input, .prompt-box {
  width: 100%;
  min-height: 108px;
  resize: vertical;
  background: #121428;
  border: 1px solid #2a2e4d;
  border-radius: 12px;
  color: rgba(255,255,255,0.88);
  padding: 10px 12px;
  font: 13px/1.55 ui-monospace, Consolas, monospace;
  box-sizing: border-box;
}
.prompt-input.slim { min-height: 64px; }
.prompt-box { display: flex; flex-wrap: wrap; align-content: flex-start; gap: 4px 2px; }
.tok {
  display: inline-flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 1px;
  padding: 1px 4px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font: inherit;
  text-align: left;
}
.tok em { font-style: normal; color: rgba(255,255,255,0.42); font-size: 11px; }
.tok.on { background: #2e3152; }
.tok.miss { opacity: 0.7; }
.params { display: flex; flex-wrap: wrap; gap: 6px; }
.params span, .note { background: #1b1e36; border-radius: 999px; padding: 3px 8px; color: rgba(255,255,255,0.7); font-size: 11px; }
.note { border-radius: 8px; }
.tag-detail {
  background: #121428;
  border-radius: 10px;
  padding: 10px;
  display: grid;
  gap: 6px;
}
.tag-detail b { color: #fff; display: block; }
.tag-detail a { color: var(--heading); font-size: 12px; }
.primary { background: #f5f3c2 !important; color: #0e0f21 !important; font-weight: 700; }
.zoom { z-index: 70; background: rgba(0,0,0,0.92); }
.zoom img { max-width: 96vw; max-height: 94vh; object-fit: contain; }
.zoom .close { position: absolute; top: 16px; left: 16px; }
@media (max-width: 920px) {
  .lb-shot, .lb-shot img { max-width: 94vw; max-height: 48vh; }
  .lb-info { position: static; width: min(94vw, 420px); max-height: 42vh; margin-top: 48vh; }
  .lb-fold { display: none; }
  .lb-circle.nav.next { right: 16px; }
}
</style>
