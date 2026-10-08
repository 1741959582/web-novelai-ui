<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { suggestTags as suggestTagsRemote, translateText } from "@/api/tauri";
import NaiIcon from "@/components/NaiIcon.vue";
import { categoryColor, danbooruCategory, lookupZhTag, suggestTags as suggestLocal, type TagHit } from "@/utils/tagSuggest";
import TagExplorer from "@/components/TagExplorer.vue";
import TagWeightList from "@/components/TagWeightList.vue";
import { highlightWeightedPrompt, parseWeightedTag, splitPromptTags } from "@/utils/promptWeight";
import { translateWeightedPrompt } from "@/utils/promptTools";
import { appendPromptChunk, fmtCount, hasCjkText, wordAtCursor } from "@/utils/textUtils";

const props = withDefaults(defineProps<{
  modelValue: string;
  enabled?: boolean;
  placeholder?: string;
}>(), {
  enabled: true,
});
const emit = defineEmits<{ "update:modelValue": [string] }>();

type AcTab = "tran" | "lib";
const ta = ref<HTMLTextAreaElement | null>(null);
const hl = ref<HTMLElement | null>(null);
const libHits = ref<TagHit[]>([]);
const tranHits = ref<TagHit[]>([]);
const acTab = ref<AcTab>("lib");
const active = ref(0);
const composing = ref(false);
const showTags = ref(false);
const showExplorer = ref(false);
const menuOpen = ref(false);
const moreEl = ref<HTMLElement | null>(null);
const translating = ref(false);
const note = ref("");
const acStyle = ref<Record<string, string>>({});
let timer: ReturnType<typeof setTimeout> | null = null;
let suggestSeq = 0;

const chips = computed(() => splitPromptTags(props.modelValue).map((raw) => parseWeightedTag(raw)));
const highlighted = computed(() => highlightWeightedPrompt(props.modelValue));
const currentHits = computed(() => (acTab.value === "tran" ? tranHits.value : libHits.value));
const hasTran = computed(() => tranHits.value.length > 0);
const hasLib = computed(() => libHits.value.length > 0);
const showTabs = computed(() => hasTran.value && hasLib.value);
const showMenu = computed(() => hasTran.value || hasLib.value);

watch([hasTran, hasLib], () => {
  if (acTab.value === "tran" && !hasTran.value && hasLib.value) acTab.value = "lib";
  if (acTab.value === "lib" && !hasLib.value && hasTran.value) acTab.value = "tran";
});

function placeMenu() {
  const el = ta.value;
  if (!el) return;
  const r = el.getBoundingClientRect();
  acStyle.value = {
    top: `${Math.round(r.bottom + 4)}px`,
    left: `${Math.round(r.left)}px`,
    width: `${Math.round(Math.max(r.width, 280))}px`,
  };
}

function clearHits() {
  libHits.value = [];
  tranHits.value = [];
}

function pickTab() {
  if (hasTran.value && hasLib.value) return;
  if (hasTran.value) acTab.value = "tran";
  else if (hasLib.value) acTab.value = "lib";
}

async function loadLibrary(word: string) {
  let next = suggestLocal(word, 8);
  try {
    const remote = await suggestTagsRemote(word, 8);
    if (remote.length) {
      next = remote.map((item) => {
        const cat = danbooruCategory(item.category);
        return {
          tag: item.tag,
          zh: item.description,
          category: cat.label,
          count: item.count,
          color: cat.color,
        };
      });
    }
  } catch {
    /* keep local capsule fallback */
  }
  return next;
}

async function loadTranslate(word: string) {
  const out: TagHit[] = [];
  const seen = new Set<string>();
  const push = (tag: string, zh: string) => {
    const key = tag.toLowerCase();
    if (!tag || seen.has(key)) return;
    seen.add(key);
    out.push({ tag, zh, category: "翻译", color: "#f5f3c2" });
  };
  const dict = lookupZhTag(word);
  if (dict) push(dict, word);
  if (!hasCjkText(word)) return out;
  try {
    const translated = (await translateText(word, "zh-CN|en")).trim();
    if (translated && translated !== word) push(translated.replace(/_/g, " "), word);
  } catch {
    /* ignore live translate miss */
  }
  return out;
}

function scheduleSuggest(text: string, cursor: number) {
  if (timer) clearTimeout(timer);
  if (!props.enabled || composing.value) {
    clearHits();
    return;
  }
  const { word } = wordAtCursor(text, cursor);
  if (word.length < 1) {
    clearHits();
    return;
  }
  const seq = ++suggestSeq;
  timer = setTimeout(async () => {
    const [lib, tran] = await Promise.all([loadLibrary(word), loadTranslate(word)]);
    if (seq !== suggestSeq) return;
    libHits.value = lib;
    tranHits.value = tran;
    active.value = 0;
    pickTab();
    if (lib.length || tran.length) placeMenu();
  }, 140);
}

function syncScroll() {
  const el = ta.value;
  const back = hl.value;
  if (!el || !back) return;
  back.scrollTop = el.scrollTop;
  back.scrollLeft = el.scrollLeft;
}

function onInput(e: Event) {
  const el = e.target as HTMLTextAreaElement;
  note.value = "";
  emit("update:modelValue", el.value);
  scheduleSuggest(el.value, el.selectionStart ?? el.value.length);
}

function onDocPointer(e: Event) {
  const node = e.target as Node | null;
  if (node && moreEl.value?.contains(node)) return;
  menuOpen.value = false;
}

watch(menuOpen, (open) => {
  if (open) document.addEventListener("pointerdown", onDocPointer, true);
  else document.removeEventListener("pointerdown", onDocPointer, true);
});

function insertExplored(tag: string) {
  emit("update:modelValue", appendPromptChunk(props.modelValue, tag));
}

function openBricks() {
  menuOpen.value = false;
  if (!chips.value.length) {
    note.value = "先输入提示词，再拆成积木";
    return;
  }
  showTags.value = true;
}

async function translateNow() {
  if (translating.value) return;
  menuOpen.value = false;
  translating.value = true;
  try {
    const result = await translateWeightedPrompt(props.modelValue);
    emit("update:modelValue", result.text);
    note.value = result.note;
  } finally {
    translating.value = false;
  }
}

function applyTag(tag: string) {
  const el = ta.value;
  const value = props.modelValue;
  const cursor = el?.selectionStart ?? value.length;
  const { start } = wordAtCursor(value, cursor);
  let end = cursor;
  while (end < value.length && /[\w-]/.test(value[end])) end++;
  const after = value.slice(end).replace(/^\s*,\s*/, "").trimStart();
  const next = `${value.slice(0, start)}${tag}, ${after}`;
  emit("update:modelValue", next);
  clearHits();
  requestAnimationFrame(() => {
    if (!el) return;
    const pos = start + tag.length + 2;
    el.focus();
    el.setSelectionRange(pos, pos);
  });
}

function onBlur() {
  setTimeout(clearHits, 180);
}

function onKeydown(e: KeyboardEvent) {
  if (e.isComposing || composing.value || !currentHits.value.length) return;
  if (e.key === "ArrowDown") {
    e.preventDefault();
    active.value = Math.min(active.value + 1, currentHits.value.length - 1);
  } else if (e.key === "ArrowUp") {
    e.preventDefault();
    active.value = Math.max(active.value - 1, 0);
  } else if (e.key === "Enter" || e.key === "Tab") {
    e.preventDefault();
    applyTag(currentHits.value[active.value].tag);
  } else if (e.key === "Escape") {
    e.preventDefault();
    clearHits();
  }
}

function setTab(next: AcTab) {
  acTab.value = next;
  active.value = 0;
}

onBeforeUnmount(() => {
  if (timer) clearTimeout(timer);
  suggestSeq += 1;
  document.removeEventListener("pointerdown", onDocPointer, true);
});
</script>

<template>
  <div class="pf">
    <div class="editor">
      <div ref="hl" class="hl" aria-hidden="true" v-html="highlighted" />
      <textarea
        ref="ta"
        :value="modelValue"
        :placeholder="placeholder"
        @input="onInput"
        @scroll="syncScroll"
        @keydown="onKeydown"
        @compositionstart="composing = true; clearHits()"
        @compositionend="(e) => { composing = false; onInput(e) }"
        @blur="onBlur"
      />
    </div>
    <div class="bar">
      <slot name="tools" />
      <div ref="moreEl" class="more">
        <button type="button" class="tag-btn" :class="{ on: menuOpen }" title="积木和翻译" @click="menuOpen = !menuOpen">
          <NaiIcon name="globe" :size="13" />
        </button>
        <div v-if="menuOpen" class="pop">
          <button type="button" @click="openBricks">
            <NaiIcon name="layers" :size="14" />积木
          </button>
          <button type="button" :disabled="translating" @click="translateNow">
            <NaiIcon name="globe" :size="14" />{{ translating ? "翻译中…" : "翻译" }}
          </button>
        </div>
      </div>
      <button type="button" class="tag-btn" :class="{ on: showExplorer }" @click="showExplorer = !showExplorer">扩展</button>
      <button type="button" class="tag-btn" :class="{ on: showTags }" :disabled="!chips.length" @click="showTags = !showTags">
        标签{{ chips.length ? ` ${chips.length}` : "" }}
      </button>
    </div>
    <p v-if="note" class="note">{{ note }}</p>
    <TagExplorer v-if="showExplorer" @insert="insertExplored" @close="showExplorer = false" />
    <TagWeightList
      v-if="showTags && chips.length"
      class="wgts"
      :model-value="modelValue"
      @update:model-value="emit('update:modelValue', $event)"
    />
    <Teleport to="body">
      <div v-if="showMenu" class="ac" role="listbox" :style="acStyle">
        <div v-if="showTabs" class="tabs">
          <button type="button" :class="{ on: acTab === 'tran' }" @mousedown.prevent="setTab('tran')">翻译</button>
          <button type="button" :class="{ on: acTab === 'lib' }" @mousedown.prevent="setTab('lib')">提示词库</button>
        </div>
        <p v-else class="only">{{ hasTran ? "翻译" : "提示词库" }}</p>
        <button
          v-for="(item, i) in currentHits"
          :key="`${acTab}-${item.tag}-${i}`"
          type="button"
          class="ac-item"
          :class="{ on: i === active }"
          @mousedown.prevent="applyTag(item.tag)"
          @mouseenter="active = i"
        >
          <i class="dot" :style="{ background: item.color || categoryColor(item.category) }" />
          <span class="main">
            <b>{{ item.tag }}</b>
            <small>{{ item.zh }}</small>
          </span>
          <em>
            {{ item.category }}
            <span v-if="item.count">{{ fmtCount(item.count) }}</span>
          </em>
        </button>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.pf { position: relative; }
.editor { position: relative; background: var(--bg0); border-radius: 6px; }
.hl, .editor textarea {
  display: block;
  width: 100%;
  min-height: 88px;
  margin: 0;
  padding: 10px;
  border: 0;
  border-radius: 6px;
  font: inherit;
  font-size: 0.875rem;
  line-height: 1.45;
  letter-spacing: normal;
  white-space: pre-wrap;
  overflow-wrap: break-word;
  word-break: break-word;
  box-sizing: border-box;
}
.hl {
  position: absolute;
  inset: 0;
  overflow: hidden;
  pointer-events: none;
  color: var(--text);
  background: transparent;
  padding-right: 24px;
}
.editor textarea {
  position: relative;
  z-index: 1;
  resize: vertical;
  background: transparent;
  color: transparent;
  caret-color: #fff;
  scrollbar-gutter: stable;
}
.editor textarea::selection {
  background: rgba(124, 131, 214, 0.35);
  color: transparent;
}
.hl :deep(.w-up) {
  color: #ff6a3d;
  background: rgba(255, 90, 48, 0.18);
  border-radius: 3px;
}
.hl :deep(.w-lo) {
  color: #6aa7ff;
  background: rgba(80, 150, 255, 0.18);
  border-radius: 3px;
}
.hl :deep(.w-off) {
  color: rgba(255, 255, 255, 0.38);
  text-decoration: line-through;
}
.bar {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 8px;
  margin-top: 6px;
}
.tag-btn {
  height: 26px;
  padding: 0 10px;
  border: 0;
  border-radius: 999px;
  background: #1f2138;
  color: rgba(255,255,255,0.78);
  font-size: 12px;
}
.tag-btn.on, .tag-btn:hover { background: #2e3152; color: var(--heading); }
.tag-btn:disabled { opacity: 0.45; }
.more { position: relative; }
.more > .tag-btn { display: inline-flex; align-items: center; justify-content: center; width: 28px; padding: 0; }
.pop {
  position: absolute;
  right: 0;
  top: calc(100% + 6px);
  z-index: 30;
  min-width: 132px;
  padding: 4px;
  border-radius: 10px;
  background: #1a1c34;
  border: 1px solid #2b2e4a;
  box-shadow: 0 12px 28px rgba(0,0,0,0.35);
}
.pop button {
  width: 100%;
  height: 32px;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 10px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: #fff;
  font-size: 13px;
  text-align: left;
}
.pop button:hover { background: #262948; }
.pop button:disabled { opacity: 0.5; }
.note { margin: 4px 0 0; color: rgba(255,255,255,0.5); font-size: 11px; }
.wgts { margin-top: 8px; }
.ac {
  position: fixed;
  z-index: 80;
  background: #1a1c34;
  border: 1px solid #2b2e4a;
  border-radius: 10px;
  padding: 4px;
  box-shadow: 0 12px 28px rgba(0,0,0,0.35);
}
.tabs, .only {
  display: flex;
  gap: 4px;
  padding: 4px 4px 6px;
}
.tabs button {
  flex: 1;
  height: 26px;
  border: 0;
  border-radius: 7px;
  background: #14162b;
  color: rgba(255,255,255,0.62);
  font-size: 12px;
}
.tabs button.on { background: #2e3152; color: var(--heading); }
.only {
  margin: 0;
  color: rgba(255,255,255,0.42);
  font-size: 11px;
}
.ac-item {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 8px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: #fff;
  text-align: left;
}
.ac-item.on { background: #262948; }
.dot { width: 7px; height: 7px; border-radius: 50%; flex-shrink: 0; }
.main { display: flex; flex-direction: column; min-width: 0; flex: 1; }
.main b { font-size: 12px; font-weight: 700; }
.main small { color: rgba(255,255,255,0.5); font-size: 11px; }
em { font-style: normal; color: rgba(255,255,255,0.38); font-size: 11px; display: grid; justify-items: end; gap: 2px; }
</style>
