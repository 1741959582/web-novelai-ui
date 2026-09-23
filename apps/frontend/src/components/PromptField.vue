<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { suggestTags as suggestTagsRemote, translateText } from "@/api/tauri";
import { categoryColor, danbooruCategory, lookupZhTag, suggestTags as suggestLocal, type TagHit } from "@/utils/tagSuggest";
import TagWeightList from "@/components/TagWeightList.vue";
import { parseWeightedTag, splitPromptTags } from "@/utils/promptWeight";
import { fmtCount, hasCjkText, wordAtCursor } from "@/utils/textUtils";

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
const libHits = ref<TagHit[]>([]);
const tranHits = ref<TagHit[]>([]);
const acTab = ref<AcTab>("lib");
const active = ref(0);
const composing = ref(false);
const showTags = ref(false);
const acStyle = ref<Record<string, string>>({});
let timer: ReturnType<typeof setTimeout> | null = null;
let suggestSeq = 0;

const chips = computed(() => splitPromptTags(props.modelValue).map((raw) => parseWeightedTag(raw)));
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

function onInput(e: Event) {
  const el = e.target as HTMLTextAreaElement;
  emit("update:modelValue", el.value);
  scheduleSuggest(el.value, el.selectionStart ?? el.value.length);
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
});
</script>

<template>
  <div class="pf">
    <textarea
      ref="ta"
      :value="modelValue"
      :placeholder="placeholder"
      @input="onInput"
      @keydown="onKeydown"
      @compositionstart="composing = true; clearHits()"
      @compositionend="(e) => { composing = false; onInput(e) }"
      @blur="onBlur"
    />
    <div class="bar">
      <slot name="tools" />
      <button type="button" class="tag-btn" :class="{ on: showTags }" :disabled="!chips.length" @click="showTags = !showTags">
        标签{{ chips.length ? ` ${chips.length}` : "" }}
      </button>
    </div>
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
textarea {
  width: 100%;
  min-height: 88px;
  resize: vertical;
  background: var(--bg0);
  border: 0;
  border-radius: 6px;
  padding: 10px;
  line-height: 1.45;
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
