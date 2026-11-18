<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { useAppStore } from "@/stores/app";
import NaiIcon from "@/components/NaiIcon.vue";

interface WorkCard {
  id: number;
  title: string;
  author: string;
  views: number;
  bookmarks: number;
  count: number;
  aiType: string;
  thumb: string;
}

interface WorkDetail {
  prompt: string;
  negative: string;
  model: string;
  image: string;
}

const router = useRouter();
const app = useAppStore();
const mode = ref<"search" | "month" | "favorite">("search");
const naiOnly = ref(true);
const model = ref("");
const query = ref("");
const cards = ref<WorkCard[]>([]);
const total = ref(0);
const loading = ref(false);
const error = ref("");
const selected = ref<WorkCard | null>(null);
const detail = ref<WorkDetail | null>(null);
const favorites = ref<WorkCard[]>(readFavs());

function readFavs() {
  try {
    const parsed = JSON.parse(localStorage.getItem("nai_online_favs") || "[]");
    return Array.isArray(parsed) ? parsed.filter((item) => item && typeof item.id === "number") : [];
  } catch {
    return [];
  }
}

function thumb(originals: unknown) {
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
  if (!url.includes(marker)) return "";
  const switched = url.replace(marker, "/img-master/img/");
  const dot = switched.lastIndexOf(".");
  if (dot <= switched.lastIndexOf("/")) return "";
  return `/pixiv${switched.slice(switched.indexOf("/img-master"))}`;
}

function isNai(value: string) {
  const text = value.trim().toLowerCase();
  return text === "nai" || text === "nai_x" || text === "naix" || text === "nai x";
}

async function load() {
  loading.value = true;
  error.value = "";
  try {
    if (mode.value === "favorite") {
      cards.value = favorites.value;
      total.value = favorites.value.length;
      return;
    }
    const version = model.value === "3" ? "Stable Diffusion XL" : model.value === "4" ? "NovelAI Diffusion V4" : model.value === "4.5" ? "NovelAI Diffusion V4.5" : model.value === "5" ? "NovelAI Diffusion V5" : "";
    const q = [version, query.value.trim()].filter(Boolean).join(" ");
    const url = mode.value === "month"
      ? `/aitag/api/rank/monthly/real?page=1&page_size=60&q=${encodeURIComponent(q)}&prompt=`
      : `/aitag/api/ai_works_search?page=1&page_size=60&q=${encodeURIComponent(q)}&prompt=&sort=new&time_range=all`;
    const response = await fetch(url, { headers: { Accept: "application/json" } });
    if (!response.ok) throw new Error(`AI TAG ${response.status}`);
    const data = await response.json();
    const items = Array.isArray(data.items) ? data.items : [];
    cards.value = items
      .map((item: Record<string, unknown>) => ({
        id: Number(item.id),
        title: String(item.title || "未命名"),
        author: String(item.userName || ""),
        views: Number(item.total_view || 0),
        bookmarks: Number(item.total_bookmarks || 0),
        count: Number(item.image_count || 1),
        aiType: String(item.AI_type || item.ai_type || ""),
        thumb: thumb(item.original_urls),
      }))
      .filter((card: WorkCard) => !naiOnly.value || isNai(card.aiType));
    total.value = Number(data.total || cards.value.length);
  } catch (reason) {
    error.value = reason instanceof Error ? reason.message : String(reason);
    cards.value = [];
  } finally {
    loading.value = false;
  }
}

async function openCard(card: WorkCard) {
  selected.value = card;
  detail.value = null;
  try {
    const response = await fetch(`/aitag/api/work/${card.id}`, { headers: { Accept: "application/json" } });
    if (!response.ok) throw new Error(`作品 ${response.status}`);
    const data = await response.json();
    const image = (data.images || [])[0] || {};
    const path = String(image.image_path || "");
    detail.value = {
      prompt: String(image.prompt_text || ""),
      negative: "",
      model: String(image.model || ""),
      image: path ? `/aiimg/${path.replace(/^\/+/, "")}` : card.thumb,
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
    error.value = reason instanceof Error ? reason.message : String(reason);
  }
}

function send() {
  if (!detail.value) return;
  if (detail.value.prompt) app.params.positivePrompt = detail.value.prompt;
  if (detail.value.negative) app.params.negativePrompt = detail.value.negative;
  app.status = "已把作品提示词发到生图";
  void router.push("/");
}

function toggleFav(card: WorkCard) {
  favorites.value = favorites.value.some((item) => item.id === card.id)
    ? favorites.value.filter((item) => item.id !== card.id)
    : [card, ...favorites.value];
  localStorage.setItem("nai_online_favs", JSON.stringify(favorites.value));
}

function pickModel(value: string) {
  model.value = model.value === value ? "" : value;
  if (model.value) naiOnly.value = true;
  void load();
}

watch(naiOnly, () => void load());
onMounted(() => void load());
</script>

<template>
  <div class="lib">
    <aside>
      <header><strong>在线画廊</strong></header>
      <div class="source">
        <b>AI TAG</b>
        <small>NAI 作品社区</small>
      </div>
      <p class="label">浏览</p>
      <button type="button" :class="{ on: mode === 'search' }" @click="mode = 'search'; load()"><NaiIcon name="search" :size="14" /> 搜索</button>
      <button type="button" :class="{ on: mode === 'month' }" @click="mode = 'month'; load()"><NaiIcon name="chart" :size="14" /> 月榜</button>
      <button type="button" :class="{ on: mode === 'favorite' }" @click="mode = 'favorite'; load()"><NaiIcon name="star" :size="14" /> 我的收藏 <em>{{ favorites.length }}</em></button>
    </aside>
    <section>
      <header class="bar">
        <label class="search">
          <input v-model="query" placeholder="作品、作者、标题、标签" @keydown.enter="load" />
        </label>
        <button type="button" :class="{ on: naiOnly }" @click="naiOnly = !naiOnly">仅 NAI 图片</button>
        <span>模型</span>
        <button type="button" :class="{ on: !model }" @click="pickModel('')">全部</button>
        <button v-for="item in ['3', '4', '4.5', '5']" :key="item" type="button" :class="{ on: model === (item === '3' ? '3' : item) }" @click="pickModel(item === '3' ? '3' : item)">
          {{ item === "3" ? "V3" : item === "4" ? "V4" : item === "4.5" ? "V4.5" : "V5" }}
        </button>
        <span class="grow">已加载 {{ cards.length }} 张</span>
      </header>
      <p v-if="error" class="error">{{ error }}</p>
      <p v-else-if="loading" class="error">正在加载 AI TAG…</p>
      <div class="wall">
        <button v-for="card in cards" :key="card.id" type="button" class="card" :class="{ on: selected?.id === card.id }" @click="openCard(card)">
          <img v-if="card.thumb" :src="card.thumb" alt="" />
          <span v-if="card.count > 1" class="count">{{ card.count }}</span>
          <span class="meta">
            <b>{{ card.title }}</b>
            <small>{{ card.author }} · {{ card.views }} · {{ card.bookmarks }}</small>
          </span>
        </button>
      </div>
    </section>
    <aside class="inspector">
      <header><strong>作品</strong></header>
      <div v-if="!selected" class="blank">单击一张卡片，在这里看 Prompt 和生成参数</div>
      <div v-else class="detail">
        <img v-if="detail?.image || selected.thumb" :src="detail?.image || selected.thumb" alt="" />
        <h3>{{ selected.title }}</h3>
        <p>{{ selected.author }}</p>
        <small>{{ detail?.model || selected.aiType }}</small>
        <textarea readonly :value="detail?.prompt || '正在读取提示词…'" rows="8" />
        <textarea v-if="detail?.negative" readonly :value="detail.negative" rows="4" />
        <div class="acts">
          <button class="primary" type="button" :disabled="!detail" @click="send">发到生图</button>
          <button type="button" @click="toggleFav(selected)">{{ favorites.some((item) => item.id === selected.id) ? "已收藏" : "收藏" }}</button>
        </div>
      </div>
    </aside>
  </div>
</template>

<style scoped>
.lib { height: 100%; display: grid; grid-template-columns: 220px 1fr 300px; background: #12141f; color: #fff; }
aside { display: flex; flex-direction: column; gap: 2px; padding: 8px; background: #16182c; border-right: 1px solid rgba(255,255,255,.08); overflow: auto; }
.inspector { border-right: 0; border-left: 1px solid rgba(255,255,255,.08); }
header { display: flex; align-items: center; height: 40px; padding: 0 8px; }
.source { margin: 8px; padding: 10px; border-radius: 10px; background: rgba(255,255,255,.05); display: grid; }
.source small, .label, .blank, .detail p, .detail small { color: rgba(255,255,255,.55); }
.label { margin: 12px 8px 4px; font-size: 12px; }
aside > button, .bar button {
  display: flex; align-items: center; gap: 8px; height: 32px; padding: 0 8px; border: 0; border-radius: 8px;
  background: transparent; color: rgba(255,255,255,.82);
}
aside > button.on, .bar button.on { background: rgba(96,165,250,.16); color: #8eb7ff; }
em { margin-left: auto; font-style: normal; color: rgba(255,255,255,.4); }
section { min-width: 0; display: flex; flex-direction: column; }
.bar { display: flex; align-items: center; gap: 6px; min-height: 52px; padding: 8px 12px; border-bottom: 1px solid rgba(255,255,255,.08); flex-wrap: wrap; }
.search { flex: 1; min-width: 180px; }
.search input, textarea { width: 100%; border: 0; border-radius: 8px; background: rgba(255,255,255,.06); color: #fff; padding: 8px 10px; }
.grow { margin-left: auto; color: rgba(255,255,255,.45); font-size: 12px; }
.error { margin: 8px 12px; color: #e39b4a; }
.wall { flex: 1; overflow: auto; padding: 12px; columns: 4 180px; column-gap: 10px; }
.card { display: block; width: 100%; margin: 0 0 10px; padding: 0; break-inside: avoid; border: 0; border-radius: 10px; overflow: hidden; background: #1a1e38; color: inherit; text-align: left; position: relative; }
.card.on { outline: 2px solid #60a5fa; }
.card img { width: 100%; display: block; background: #10111c; }
.count { position: absolute; top: 8px; right: 8px; padding: 2px 6px; border-radius: 6px; background: rgba(0,0,0,.55); font-size: 12px; }
.meta { display: grid; gap: 2px; padding: 8px; }
.meta b, .meta small { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.meta small { color: rgba(255,255,255,.5); }
.blank, .detail { padding: 16px; }
.detail { display: flex; flex-direction: column; gap: 8px; }
.detail img { width: 100%; border-radius: 8px; background: #10111c; }
h3 { margin: 0; font-size: 16px; font-family: inherit; }
.acts { display: flex; gap: 8px; }
.primary { border: 0; border-radius: 8px; background: #3b82f6; color: #fff; height: 32px; padding: 0 12px; }
.acts button { height: 32px; padding: 0 12px; border-radius: 8px; border: 1px solid rgba(255,255,255,.12); background: transparent; color: #fff; }
</style>
