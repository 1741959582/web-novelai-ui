<script setup lang="ts">
import { onBeforeUnmount, ref } from "vue";
import { danbooruSemanticSearch, type SemanticTag } from "@/api/tauri";

const props = defineProps<{ text: string }>();
const emit = defineEmits<{ apply: [string] }>();

const open = ref(false);
const root = ref<HTMLElement | null>(null);
const panel = ref<HTMLElement | null>(null);
const box = ref({ top: 0, left: 0, width: 420 });
const query = ref("");
const mode = ref("concept_explore");
const category = ref("all");
const showNsfw = ref(false);
const busy = ref(false);
const error = ref("");
const hits = ref<SemanticTag[]>([]);
const picked = ref<string[]>([]);

const modes = [
  { id: "full_scene", label: "完整场景" },
  { id: "concept_explore", label: "概念探索" },
  { id: "subject_describe", label: "主体描述" },
  { id: "precise_lookup", label: "精确查找" },
];
const categories = [
  { id: "all", label: "全部" },
  { id: "general", label: "通用" },
  { id: "character", label: "角色" },
  { id: "copyright", label: "作品" },
];

function place() {
  const button = root.value?.querySelector("button");
  if (!button) return;
  const rect = button.getBoundingClientRect();
  const width = Math.min(420, window.innerWidth - 16);
  const left = Math.max(8, Math.min(rect.left, window.innerWidth - width - 8));
  const top = rect.bottom + 280 > window.innerHeight ? Math.max(8, rect.top - 286) : rect.bottom + 6;
  box.value = { top, left, width };
}

function toggleOpen() {
  open.value = !open.value;
  if (open.value) place();
}

function onDocDown(ev: PointerEvent) {
  if (!open.value) return;
  const node = ev.target as Node | null;
  if (node && (root.value?.contains(node) || panel.value?.contains(node))) return;
  open.value = false;
}

window.addEventListener("pointerdown", onDocDown);
onBeforeUnmount(() => window.removeEventListener("pointerdown", onDocDown));

function toggle(tag: string) {
  picked.value = picked.value.includes(tag)
    ? picked.value.filter((item) => item !== tag)
    : [...picked.value, tag];
}

function mergeTags(current: string, tags: string[]) {
  const have = new Set(
    current.split(",").map((item) => item.trim().toLowerCase()).filter(Boolean),
  );
  const next = tags.filter((tag) => tag.trim() && !have.has(tag.trim().toLowerCase()));
  if (!next.length) return current;
  const base = current.trim().replace(/,\s*$/, "");
  return base ? `${base}, ${next.join(", ")}` : next.join(", ");
}

async function search() {
  const text = query.value.trim();
  if (!text || busy.value) return;
  busy.value = true;
  error.value = "";
  try {
    hits.value = await danbooruSemanticSearch({
      query: text,
      searchMode: mode.value,
      category: category.value,
      showNsfw: showNsfw.value,
    });
    if (!hits.value.length) error.value = "没有匹配到标签";
  } catch (err) {
    hits.value = [];
    error.value = err instanceof Error ? err.message : String(err);
  } finally {
    busy.value = false;
  }
}

function apply() {
  if (!picked.value.length) return;
  emit("apply", mergeTags(props.text, picked.value));
  open.value = false;
}
</script>

<template>
  <div ref="root" class="danbooru">
    <button type="button" class="open" :class="{ on: open }" @click.stop="toggleOpen">Danbooru 标签模糊搜索</button>
  </div>
  <Teleport to="body">
    <div
      v-if="open"
      ref="panel"
      class="bubble"
      :style="{ top: `${box.top}px`, left: `${box.left}px`, width: `${box.width}px` }"
      @pointerdown.stop
      @keydown.esc="open = false"
    >
      <div class="row">
        <input
          v-model="query"
          type="text"
          placeholder="用中文描述，例如微笑、水手服"
          @keydown.enter.prevent="search"
        />
        <button type="button" :disabled="busy || !query.trim()" @click="search">{{ busy ? "搜索中" : "搜索" }}</button>
      </div>
      <div class="row filters">
        <select v-model="mode">
          <option v-for="item in modes" :key="item.id" :value="item.id">{{ item.label }}</option>
        </select>
        <select v-model="category">
          <option v-for="item in categories" :key="item.id" :value="item.id">{{ item.label }}</option>
        </select>
        <label><input v-model="showNsfw" type="checkbox" />NSFW</label>
      </div>
      <p v-if="error" class="note">{{ error }}</p>
      <p v-else-if="busy" class="note">第一次搜索可能要等十几秒。</p>
      <div v-if="hits.length" class="hits">
        <button
          v-for="hit in hits"
          :key="hit.tag"
          type="button"
          class="hit"
          :class="{ on: picked.includes(hit.tag) }"
          :title="hit.wiki || hit.cnName"
          @click="toggle(hit.tag)"
        >
          <b>{{ hit.tag }}</b>
          <small v-if="hit.cnName">{{ hit.cnName }}</small>
        </button>
      </div>
      <div class="foot">
        <span>{{ picked.length ? `已选 ${picked.length}` : "点标签选中" }}</span>
        <button type="button" class="add" :disabled="!picked.length" @click="apply">把已选标签加入关键词</button>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.danbooru { position: relative; display: flex; }
.open {
  height: 26px;
  padding: 0 12px;
  border-radius: 999px;
  border: 1px solid var(--bg3);
  background: #1c1e33;
  color: rgba(255,255,255,0.82);
  font-size: 12px;
  white-space: nowrap;
}
.open.on { border-color: #f5f3c2; color: #f5f3c2; }
.bubble {
  position: fixed;
  z-index: 80;
  padding: 10px;
  border-radius: 12px;
  border: 1px solid #3a3d5c;
  background: #14162a;
  box-shadow: 0 16px 40px rgba(0,0,0,0.45);
}
.row { display: flex; gap: 8px; align-items: center; }
.row input, .row select {
  height: 32px;
  border-radius: 8px;
  border: 1px solid var(--bg3);
  background: #0e0f21;
  color: #fff;
  padding: 0 8px;
}
.row input { flex: 1; min-width: 0; }
.filters { margin-top: 8px; }
.filters label { display: flex; align-items: center; gap: 4px; font-size: 12px; color: rgba(255,255,255,0.7); }
.note { margin: 8px 0 0; color: rgba(255,255,255,0.55); font-size: 12px; }
.hits {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  max-height: 220px;
  overflow: auto;
  margin-top: 10px;
}
.hit {
  display: grid;
  gap: 1px;
  max-width: 100%;
  padding: 4px 8px;
  border-radius: 8px;
  border: 1px solid #2d3150;
  background: #1b1e34;
  color: #fff;
  text-align: left;
}
.hit.on { border-color: #f5f3c2; background: #2a2d22; }
.hit b { font-size: 12px; font-weight: 650; }
.hit small {
  color: rgba(255,255,255,0.55);
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.foot { display: flex; align-items: center; justify-content: space-between; gap: 8px; margin-top: 10px; }
.foot span { color: rgba(255,255,255,0.5); font-size: 12px; }
.add {
  height: 30px;
  padding: 0 10px;
  border: 0;
  border-radius: 8px;
  background: #f5f3c2;
  color: #0e0f21;
  font-weight: 700;
  font-size: 12px;
}
.add:disabled, .row button:disabled { opacity: 0.45; }
.row button {
  height: 32px;
  padding: 0 12px;
  border-radius: 8px;
  border: 1px solid var(--bg3);
  background: #22253f;
  color: #fff;
}
</style>
