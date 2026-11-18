<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { useAppStore } from "@/stores/app";
import NaiIcon from "@/components/NaiIcon.vue";

type SourceId = "aiTag" | "danbooru" | "safebooru" | "gelbooru";

interface WorkCard {
  source: SourceId;
  id: number;
  title: string;
  author: string;
  views: number;
  bookmarks: number;
  count: number;
  aiType: string;
  thumb: string;
  fallback: string;
  large: string;
  prompt: string;
  model: string;
}

interface WorkDetail {
  prompt: string;
  negative: string;
  model: string;
  image: string;
}

const SOURCES: { id: SourceId; name: string; hint: string }[] = [
  { id: "aiTag", name: "AI TAG", hint: "NAI 作品社区" },
  { id: "danbooru", name: "Danbooru", hint: "标签图库" },
  { id: "safebooru", name: "Safebooru", hint: "全年龄标签图库" },
  { id: "gelbooru", name: "Gelbooru", hint: "标签图库" },
];

const PAGE_SIZE = 60;
const FIRST_TARGET = 120;

const router = useRouter();
const app = useAppStore();
const source = ref<SourceId>("aiTag");
const sourceOpen = ref(false);
const mode = ref<"search" | "month" | "favorite">("search");
const naiOnly = ref(true);
const model = ref("");
const query = ref("");
const promptQuery = ref("");
const timeRange = ref("all");
const period = ref("current");
const years = ref<number[]>([2026, 2025, 2024, 2023]);
const months = ref<string[]>([]);
const cards = ref<WorkCard[]>([]);
const pagesLoaded = ref(0);
const hasMore = ref(true);
const loading = ref(false);
const error = ref("");
const selected = ref<WorkCard | null>(null);
const detail = ref<WorkDetail | null>(null);
const pageInput = ref("1");
const favorites = ref<WorkCard[]>(readFavs());
const wallRef = ref<HTMLElement | null>(null);
const columnCount = ref(4);
const columns = computed(() => {
  const count = Math.max(1, columnCount.value);
  const cols: WorkCard[][] = Array.from({ length: count }, () => []);
  cards.value.forEach((card, index) => cols[index % count].push(card));
  return cols;
});
let requestId = 0;
let detailId = 0;
let cursor = 1;

const currentSource = computed(() => SOURCES.find((item) => item.id === source.value) || SOURCES[0]);
const timeOptions = computed(() => {
  const list = [{ id: "all", label: "全部时间" }];
  for (const year of years.value) {
    list.push({ id: `y${year}`, label: `${year} 年` });
    if (year > 2023) {
      for (let quarter = 1; quarter <= 4; quarter += 1) {
        list.push({ id: `q${year}Q${quarter}`, label: `${year} Q${quarter}` });
      }
    }
  }
  list.push({ id: "older", label: "更早" });
  return list;
});
const visibleFavorites = computed(() => favorites.value.filter((item) => (item.source || "aiTag") === source.value));

function readFavs() {
  try {
    const parsed = JSON.parse(localStorage.getItem("nai_online_favs") || "[]");
    return Array.isArray(parsed) ? parsed.filter((item) => item && typeof item.id === "number") : [];
  } catch {
    return [];
  }
}

function cardKey(card: WorkCard) {
  return `${card.source || "aiTag"}:${card.id}`;
}

function isNai(value: string) {
  const text = value.trim().toLowerCase();
  return text === "nai" || text === "nai_x" || text === "naix" || text === "nai x";
}

function pixivPaths(originals: unknown) {
  const raw = typeof originals === "string" ? originals : JSON.stringify(originals || []);
  let list: string[] = [];
  try {
    const parsed = JSON.parse(raw);
    list = Array.isArray(parsed) ? parsed.map(String) : [];
  } catch {
    list = [];
  }
  const url = list[0] || "";
  const marker = "/img-original/img/";
  const index = url.indexOf(marker);
  if (index < 0) return { thumb: "", fallback: "" };
  const path = url.slice(index + marker.length);
  const slash = path.lastIndexOf("/");
  const file = slash >= 0 ? path.slice(slash + 1) : path;
  const dir = slash >= 0 ? path.slice(0, slash) : "";
  const dot = file.lastIndexOf(".");
  const stem = dot > 0 ? file.slice(0, dot) : file;
  const prefix = dir ? `${dir}/` : "";
  return {
    thumb: `/pixiv/img-master/img/${prefix}${stem}_master1200.jpg`,
    fallback: `/pixiv/img-original/img/${prefix}${file}`,
  };
}

function modelQuery() {
  if (model.value === "3") return "Stable Diffusion XL";
  if (model.value === "4") return "NovelAI Diffusion V4";
  if (model.value === "4.5") return "NovelAI Diffusion V4.5";
  if (model.value === "5") return "NovelAI Diffusion V5";
  return "";
}

function mapAiTag(item: Record<string, unknown>): WorkCard {
  const pictures = pixivPaths(item.original_urls);
  return {
    source: "aiTag",
    id: Number(item.id),
    title: String(item.title || "未命名"),
    author: String(item.userName || ""),
    views: Number(item.total_view || 0),
    bookmarks: Number(item.total_bookmarks || 0),
    count: Number(item.image_count || 1),
    aiType: String(item.AI_type || item.ai_type || ""),
    thumb: pictures.thumb,
    fallback: pictures.fallback,
    large: pictures.thumb,
    prompt: "",
    model: "",
  };
}

function mapDonmai(item: Record<string, unknown>, id: SourceId): WorkCard | null {
  const thumb = String(item.preview_file_url || "");
  const postId = Number(item.id);
  if (!postId || !thumb) return null;
  const tags = String(item.tag_string || "").replaceAll("_", " ");
  return {
    source: id,
    id: postId,
    title: tags.split(" ").slice(0, 6).join(" ") || `#${postId}`,
    author: String(item.tag_string_artist || ""),
    views: Number(item.score || 0),
    bookmarks: Number(item.fav_count || 0),
    count: 1,
    aiType: String(item.rating || ""),
    thumb,
    fallback: "",
    large: String(item.large_file_url || item.file_url || thumb),
    prompt: tags,
    model: "",
  };
}

function mapSafebooru(item: Record<string, unknown>): WorkCard | null {
  const postId = Number(item.id);
  const thumb = String(item.preview_url || "").replace(/^https?:\/\/safebooru\.org/, "/safebooru");
  if (!postId || !thumb) return null;
  const tags = String(item.tags || "").trim().replaceAll("_", " ");
  const large = String(item.sample_url || item.file_url || item.preview_url || "").replace(/^https?:\/\/safebooru\.org/, "/safebooru");
  return {
    source: "safebooru",
    id: postId,
    title: tags.split(" ").slice(0, 6).join(" ") || `#${postId}`,
    author: String(item.owner || ""),
    views: Number(item.score || 0),
    bookmarks: 0,
    count: 1,
    aiType: String(item.rating || ""),
    thumb,
    fallback: "",
    large: large || thumb,
    prompt: tags,
    model: "",
  };
}

function gelbooruImage(url: string) {
  const match = url.match(/^https?:\/\/(img\d+)\.gelbooru\.com(\/.*)$/);
  return match ? `/gelimg/${match[1]}.gelbooru.com${match[2]}` : url;
}

function parseGelbooru(html: string): WorkCard[] {
  const out: WorkCard[] = [];
  const re = /<article class="thumbnail-preview">([\s\S]*?)<\/article>/g;
  let match: RegExpExecArray | null;
  while ((match = re.exec(html))) {
    const block = match[1];
    const id = Number(/id="p(\d+)"/.exec(block)?.[1] || 0);
    const src = gelbooruImage((/<img[^>]*\ssrc="([^"]+)"/.exec(block)?.[1] || "").replaceAll("&amp;", "&"));
    const alt = (/<img[^>]*\salt="([^"]*)"/.exec(block)?.[1] || "").replaceAll("&amp;", "&");
    if (!id || !src) continue;
    const tags = alt.includes("|") ? alt.split("|").slice(1).join("|").trim() : alt;
    out.push({
      source: "gelbooru",
      id,
      title: tags.split(",").slice(0, 4).join(",") || `#${id}`,
      author: "",
      views: 0,
      bookmarks: 0,
      count: 1,
      aiType: "",
      thumb: src,
      fallback: "",
      large: src,
      prompt: tags,
      model: "",
    });
  }
  return out;
}

async function fetchPage(page: number): Promise<{ cards: WorkCard[]; raw: number; pageSize: number }> {
  if (source.value === "gelbooru") {
    const tags = query.value.trim() || "sort:id:desc";
    const pid = (page - 1) * 42;
    const url = `/gelbooru/index.php?page=post&s=list&pid=${pid}&tags=${encodeURIComponent(tags)}`;
    const response = await fetch(url);
    if (!response.ok) throw new Error(`Gelbooru ${response.status}`);
    const parsed = parseGelbooru(await response.text());
    return { cards: parsed, raw: parsed.length, pageSize: 42 };
  }
  if (source.value === "safebooru") {
    const url = `/safebooru/index.php?page=dapi&s=post&q=index&json=1&limit=100&pid=${page - 1}&tags=${encodeURIComponent(query.value.trim())}`;
    const response = await fetch(url, { headers: { Accept: "application/json" } });
    if (!response.ok) throw new Error(`Safebooru ${response.status}`);
    const data = await response.json();
    const items = Array.isArray(data) ? data : Array.isArray(data?.post) ? data.post : data?.post ? [data.post] : [];
    return {
      cards: items
        .map((item: Record<string, unknown>) => mapSafebooru(item))
        .filter((item: WorkCard | null): item is WorkCard => Boolean(item)),
      raw: items.length,
      pageSize: 100,
    };
  }
  if (source.value === "danbooru") {
    const url = `/danbooru/posts.json?limit=100&page=${page}&tags=${encodeURIComponent(query.value.trim())}`;
    const response = await fetch(url, { headers: { Accept: "application/json" } });
    if (response.status === 403) throw new Error(`${currentSource.value.name} 拒绝了列表请求`);
    if (!response.ok) throw new Error(`${currentSource.value.name} ${response.status}`);
    const data = await response.json();
    const items = Array.isArray(data) ? data : [];
    return {
      cards: items
        .map((item: Record<string, unknown>) => mapDonmai(item, source.value))
        .filter((item: WorkCard | null): item is WorkCard => Boolean(item)),
      raw: items.length,
      pageSize: 100,
    };
  }
  const q = [modelQuery(), query.value.trim()].filter(Boolean).join(" ");
  const url = mode.value === "month"
    ? period.value === "current"
      ? `/aitag/api/rank/monthly/real?page=${page}&page_size=${PAGE_SIZE}&q=${encodeURIComponent(q)}&prompt=${encodeURIComponent(promptQuery.value.trim())}`
      : `/aitag/api/rank/monthly/fixed?page=${page}&page_size=${PAGE_SIZE}&month=${encodeURIComponent(period.value)}&q=${encodeURIComponent(q)}&prompt=${encodeURIComponent(promptQuery.value.trim())}`
    : `/aitag/api/ai_works_search?page=${page}&page_size=${PAGE_SIZE}&q=${encodeURIComponent(q)}&prompt=${encodeURIComponent(promptQuery.value.trim())}&sort=new&time_range=${encodeURIComponent(timeRange.value)}`;
  const response = await fetch(url, { headers: { Accept: "application/json" } });
  if (!response.ok) throw new Error(`AI TAG ${response.status}`);
  const data = await response.json();
  const items = Array.isArray(data.items) ? data.items : [];
  const mapped = items
    .map((item: Record<string, unknown>) => mapAiTag(item))
    .filter((card: WorkCard) => source.value !== "aiTag" || !naiOnly.value || isNai(card.aiType));
  return { cards: mapped, raw: items.length, pageSize: PAGE_SIZE };
}

async function load(reset: boolean) {
  const token = ++requestId;
  if (mode.value === "favorite") {
    cards.value = visibleFavorites.value;
    pagesLoaded.value = cards.value.length ? 1 : 0;
    hasMore.value = false;
    error.value = "";
    loading.value = false;
    return;
  }
  if (reset) {
    cards.value = [];
    pagesLoaded.value = 0;
    hasMore.value = true;
    error.value = "";
    cursor = Math.max(1, Number(pageInput.value) || 1);
  } else if (loading.value || !hasMore.value) {
    return;
  }
  loading.value = true;
  const goal = reset ? FIRST_TARGET : cards.value.length + 1;
  try {
    const seen = new Set(cards.value.map(cardKey));
    while (hasMore.value && (reset ? cards.value.length < goal : true)) {
      const batch = await fetchPage(cursor);
      if (token !== requestId) return;
      const fresh = batch.cards.filter((card) => {
        const key = cardKey(card);
        if (seen.has(key)) return false;
        seen.add(key);
        return true;
      });
      cards.value = [...cards.value, ...fresh];
      pagesLoaded.value += 1;
      cursor += 1;
      if (batch.raw < batch.pageSize) hasMore.value = false;
      if (!reset || pagesLoaded.value >= 8) break;
    }
  } catch (reason) {
    if (token === requestId) error.value = reason instanceof Error ? reason.message : String(reason);
  } finally {
    if (token === requestId) loading.value = false;
  }
}

function reload() {
  pageInput.value = "1";
  void load(true);
}

function jump() {
  const page = Math.max(1, Math.floor(Number(pageInput.value) || 1));
  pageInput.value = String(page);
  void load(true);
}

function loadMore() {
  if (!loading.value && hasMore.value && mode.value !== "favorite") void load(false);
}

function onScroll(event: Event) {
  const el = event.target as HTMLElement;
  if (el.scrollHeight - el.scrollTop - el.clientHeight < 480) loadMore();
}

function pickSource(id: SourceId) {
  source.value = id;
  sourceOpen.value = false;
  if (id !== "aiTag" && mode.value === "month") mode.value = "search";
  reload();
}

function pickMode(next: "search" | "month" | "favorite") {
  mode.value = next;
  reload();
}

function pickModel(value: string) {
  model.value = model.value === value ? "" : value;
  if (model.value) naiOnly.value = true;
  reload();
}

function fallbackThumb(card: WorkCard, event: Event) {
  const img = event.target as HTMLImageElement;
  if (!card.fallback || img.dataset.fell) return;
  img.dataset.fell = "1";
  img.src = card.fallback;
}

async function openCard(card: WorkCard) {
  selected.value = card;
  const token = ++detailId;
  if (card.source !== "aiTag") {
    detail.value = {
      prompt: card.prompt,
      negative: "",
      model: card.model,
      image: card.large || card.thumb,
    };
    return;
  }
  detail.value = null;
  try {
    const response = await fetch(`/aitag/api/work/${card.id}`, { headers: { Accept: "application/json" } });
    if (!response.ok) throw new Error(`作品 ${response.status}`);
    const data = await response.json();
    if (token !== detailId) return;
    const image = (data.images || [])[0] || {};
    const path = String(image.image_path || "");
    detail.value = {
      prompt: String(image.prompt_text || ""),
      negative: "",
      model: String(image.model || ""),
      image: path ? `/aiimg/${path.replace(/^\/+/, "")}` : card.large || card.thumb,
    };
    const raw = image.ai_json;
    if (typeof raw === "string" && raw.includes("uc")) {
      try {
        const parsed = JSON.parse(raw);
        detail.value.negative = String(parsed.uc || parsed.negative_prompt || "");
        if (!detail.value.prompt) detail.value.prompt = String(parsed.prompt || "");
      } catch {
        /* prompt_text is enough */
      }
    }
  } catch (reason) {
    if (token === detailId) error.value = reason instanceof Error ? reason.message : String(reason);
  }
}

function send() {
  const prompt = detail.value?.prompt || selected.value?.prompt || "";
  const negative = detail.value?.negative || "";
  if (!prompt && !negative) return;
  if (prompt) app.params.positivePrompt = prompt;
  if (negative) app.params.negativePrompt = negative;
  app.status = "已把作品提示词发到生图";
  void router.push("/");
}

function favored(card: WorkCard) {
  return favorites.value.some((item) => cardKey(item) === cardKey(card));
}

function toggleFav(card: WorkCard) {
  favorites.value = favored(card)
    ? favorites.value.filter((item) => cardKey(item) !== cardKey(card))
    : [{ ...card, source: card.source || "aiTag" }, ...favorites.value];
  localStorage.setItem("nai_online_favs", JSON.stringify(favorites.value));
  if (mode.value === "favorite") cards.value = visibleFavorites.value;
}

async function loadConfig() {
  try {
    const response = await fetch("/aitag/api/config", { headers: { Accept: "application/json" } });
    if (!response.ok) return;
    const data = await response.json();
    if (Array.isArray(data.available_years) && data.available_years.length) {
      years.value = data.available_years.map(Number).filter((year: number) => Number.isFinite(year));
    }
    if (Array.isArray(data.available_months)) {
      months.value = data.available_months.map(String).filter((month: string) => /^\d{4}-\d{2}$/.test(month));
    }
  } catch {
    /* built-in years still cover the menu */
  }
}

let wallObserver: ResizeObserver | undefined;

function measureWall() {
  const width = wallRef.value?.clientWidth ?? 0;
  if (!width) return;
  columnCount.value = Math.max(2, Math.min(6, Math.floor((width + 10) / 220)));
}

onMounted(() => {
  void loadConfig();
  void load(true);
  measureWall();
  wallObserver = new ResizeObserver(() => measureWall());
  if (wallRef.value) wallObserver.observe(wallRef.value);
});

onBeforeUnmount(() => wallObserver?.disconnect());
</script>

<template>
  <div class="lib">
    <aside>
      <header><strong>在线画廊</strong></header>
      <div class="source-wrap">
        <button type="button" class="source" @click.stop="sourceOpen = !sourceOpen">
          <b>{{ currentSource.name }}</b>
          <small>{{ currentSource.hint }}</small>
          <NaiIcon name="down" :size="14" />
        </button>
        <div v-if="sourceOpen" class="menu">
          <button v-for="item in SOURCES" :key="item.id" type="button" :class="{ on: source === item.id }" @click="pickSource(item.id)">
            <b>{{ item.name }}</b>
            <small>{{ item.hint }}</small>
          </button>
        </div>
      </div>
      <div v-if="sourceOpen" class="mask" @click="sourceOpen = false"></div>
      <p class="label">浏览</p>
      <button type="button" :class="{ on: mode === 'search' }" @click="pickMode('search')"><NaiIcon name="search" :size="14" /> 搜索</button>
      <button v-if="source === 'aiTag'" type="button" :class="{ on: mode === 'month' }" @click="pickMode('month')"><NaiIcon name="chart" :size="14" /> 月榜</button>
      <button type="button" :class="{ on: mode === 'favorite' }" @click="pickMode('favorite')"><NaiIcon name="star" :size="14" /> 我的收藏 <em>{{ visibleFavorites.length }}</em></button>
    </aside>
    <section>
      <header class="bar">
        <label class="search">
          <input v-model="query" placeholder="作品、作者、标题、标签" @keydown.enter="reload" />
        </label>
        <label v-if="source === 'aiTag'" class="search prompt">
          <input v-model="promptQuery" placeholder="Prompt 含有" @keydown.enter="reload" />
        </label>
        <button v-if="source === 'aiTag'" type="button" :class="{ on: naiOnly }" @click="naiOnly = !naiOnly; reload()">仅 NAI 图片</button>
        <template v-if="source === 'aiTag'">
          <span>模型</span>
          <button type="button" :class="{ on: !model }" @click="pickModel('')">全部</button>
          <button v-for="item in ['3', '4', '4.5', '5']" :key="item" type="button" :class="{ on: model === item }" @click="pickModel(item)">
            {{ item === "3" ? "V3" : item === "4" ? "V4" : item === "4.5" ? "V4.5" : "V5" }}
          </button>
          <select v-if="mode === 'search'" v-model="timeRange" class="time" @change="reload">
            <option v-for="item in timeOptions" :key="item.id" :value="item.id">{{ item.label }}</option>
          </select>
          <select v-else-if="mode === 'month'" v-model="period" class="time" @change="reload">
            <option value="current">本期</option>
            <option v-for="item in months" :key="item" :value="item">{{ item }}</option>
          </select>
        </template>
        <span class="grow">已加载 {{ cards.length }} 张 · {{ pagesLoaded }} 页</span>
        <label v-if="mode !== 'favorite'" class="jump">
          跳转到页
          <input v-model="pageInput" inputmode="numeric" @keydown.enter="jump" />
        </label>
        <button v-if="mode !== 'favorite'" type="button" @click="jump">跳转</button>
      </header>
      <p v-if="error" class="error">{{ error }}</p>
      <p v-else-if="loading && !cards.length" class="error">正在加载…</p>
      <div ref="wallRef" class="wall" @scroll="onScroll">
        <div class="cols">
          <div v-for="(col, index) in columns" :key="index" class="col">
            <button v-for="card in col" :key="cardKey(card)" type="button" class="card" :class="{ on: selected && cardKey(selected) === cardKey(card) }" @click="openCard(card)">
              <img v-if="card.thumb" :src="card.thumb" alt="" @error="fallbackThumb(card, $event)" />
              <span v-else class="ph"></span>
              <span v-if="card.count > 1" class="count">{{ card.count }}</span>
              <span class="meta">
                <b>{{ card.title }}</b>
                <small>{{ card.author }}<template v-if="card.views"> · {{ card.views }}</template><template v-if="card.bookmarks"> · {{ card.bookmarks }}</template></small>
              </span>
            </button>
          </div>
        </div>
        <button v-if="hasMore && cards.length" type="button" class="more" @click="loadMore">{{ loading ? "正在加载…" : "加载更多" }}</button>
      </div>
    </section>
    <aside class="inspector">
      <header><strong>作品</strong></header>
      <div v-if="!selected" class="blank">单击一张卡片，在这里看 Prompt 和生成参数</div>
      <div v-else class="detail">
        <img v-if="detail?.image || selected.thumb" :src="detail?.image || selected.large || selected.thumb" alt="" />
        <h3>{{ selected.title }}</h3>
        <p>{{ selected.author }}</p>
        <small>{{ detail?.model || selected.aiType }}</small>
        <textarea readonly :value="detail?.prompt || selected.prompt || '正在读取提示词…'" rows="8" />
        <textarea v-if="detail?.negative" readonly :value="detail.negative" rows="4" />
        <div class="acts">
          <button class="primary" type="button" :disabled="!(detail?.prompt || selected.prompt)" @click="send">发到生图</button>
          <button type="button" @click="toggleFav(selected)">{{ favored(selected) ? "已收藏" : "收藏" }}</button>
        </div>
      </div>
    </aside>
  </div>
</template>

<style scoped>
.lib { height: 100%; min-height: 0; overflow: hidden; display: grid; grid-template-columns: 220px minmax(0, 1fr) 280px; background: #12141f; color: #fff; }
aside { min-height: 0; display: flex; flex-direction: column; gap: 2px; padding: 8px; background: #16182c; border-right: 1px solid rgba(255,255,255,.08); overflow: auto; }
.inspector { border-right: 0; border-left: 1px solid rgba(255,255,255,.08); }
header { display: flex; align-items: center; height: 40px; padding: 0 8px; flex: 0 0 auto; }
.source-wrap { position: relative; margin: 8px; }
.source { position: relative; width: 100%; margin: 0; padding: 10px 28px 10px 10px; border: 0; border-radius: 10px; background: rgba(255,255,255,.05); color: inherit; display: grid; text-align: left; }
.source :deep(svg) { position: absolute; right: 10px; top: 16px; }
.source small, .label, .blank, .detail p, .detail small, .menu small { color: rgba(255,255,255,.55); }
.mask { position: fixed; inset: 0; z-index: 20; }
.menu { position: absolute; z-index: 21; left: 0; right: 0; top: calc(100% + 4px); padding: 4px; border-radius: 10px; background: #1c2038; border: 1px solid rgba(255,255,255,.12); display: grid; gap: 2px; }
.menu button { display: grid; gap: 2px; padding: 8px; border: 0; border-radius: 8px; background: transparent; color: inherit; text-align: left; }
.menu button.on { background: rgba(96,165,250,.16); color: #8eb7ff; }
.label { margin: 12px 8px 4px; font-size: 12px; }
aside > button, .bar button {
  display: flex; align-items: center; gap: 8px; height: 32px; padding: 0 8px; border: 0; border-radius: 8px;
  background: transparent; color: rgba(255,255,255,.82);
}
aside > button.on, .bar button.on { background: rgba(96,165,250,.16); color: #8eb7ff; }
em { margin-left: auto; font-style: normal; color: rgba(255,255,255,.4); }
section { min-width: 0; min-height: 0; overflow: hidden; display: flex; flex-direction: column; }
.bar { flex: 0 0 auto; height: auto; display: flex; align-items: center; gap: 6px; min-height: 48px; padding: 8px 12px; border-bottom: 1px solid rgba(255,255,255,.08); flex-wrap: nowrap; overflow-x: auto; }
.search { flex: 1 1 180px; min-width: 140px; max-width: 280px; }
.search.prompt { flex: 1 1 140px; max-width: 200px; }
.search input, textarea, .time, .jump input { width: 100%; border: 0; border-radius: 8px; background: rgba(255,255,255,.06); color: #fff; padding: 8px 10px; }
.time { width: auto; height: 32px; }
.grow { margin-left: auto; color: rgba(255,255,255,.45); font-size: 12px; white-space: nowrap; }
.jump { display: flex; align-items: center; gap: 6px; color: rgba(255,255,255,.55); font-size: 12px; white-space: nowrap; }
.jump input { width: 64px; height: 32px; }
.error { margin: 8px 12px; color: #e39b4a; flex: 0 0 auto; }
.wall { flex: 1 1 auto; min-height: 0; overflow: auto; }
.cols { display: flex; align-items: flex-start; gap: 10px; padding: 12px; }
.col { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 10px; }
.card { display: block; width: 100%; margin: 0; padding: 0; border: 0; border-radius: 10px; overflow: hidden; background: #1a1e38; color: inherit; text-align: left; position: relative; }
.card.on { outline: 2px solid #60a5fa; }
.card img, .ph { width: 100%; height: auto; display: block; background: #10111c; }
.ph { min-height: 160px; }
.count { position: absolute; top: 8px; right: 8px; padding: 2px 6px; border-radius: 6px; background: rgba(0,0,0,.55); font-size: 12px; }
.meta { display: grid; gap: 2px; padding: 8px; }
.meta b, .meta small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.meta small { color: rgba(255,255,255,.5); }
.more { display: block; width: calc(100% - 24px); margin: 0 12px 12px; height: 36px; border: 1px solid rgba(255,255,255,.12); border-radius: 10px; background: transparent; color: rgba(255,255,255,.8); }
.blank { flex: 1; display: grid; place-items: center; padding: 24px; text-align: center; line-height: 1.5; }
.detail { padding: 16px; min-height: 0; overflow: auto; }
.detail { display: flex; flex-direction: column; gap: 8px; }
.detail img { width: 100%; border-radius: 8px; background: #10111c; }
h3 { margin: 0; font-size: 16px; font-family: inherit; }
.acts { display: flex; gap: 8px; }
.primary { border: 0; border-radius: 8px; background: #3b82f6; color: #fff; height: 32px; padding: 0 12px; }
.acts button { height: 32px; padding: 0 12px; border-radius: 8px; border: 1px solid rgba(255,255,255,.12); background: transparent; color: #fff; }
</style>
