<script setup lang="ts">
import { computed, ref } from "vue";
import NaiIcon from "@/components/NaiIcon.vue";
import TagWeightList from "@/components/TagWeightList.vue";
import {
  DEFAULT_NORMALIZE_OPTIONS,
  normalizePrompt,
} from "@/utils/promptNormalize";
import { parseWeightedTag, splitPromptTags } from "@/utils/promptWeight";
import {
  loadChunks,
  loadPresets,
  makeChunk,
  makePreset,
  saveAutoComplete,
  saveChunks,
  savePresets,
  translatePromptToEnglish,
  type PromptChunk,
  type PromptPreset,
} from "@/utils/promptTools";
import { appendPromptChunk } from "@/utils/textUtils";

const props = defineProps<{
  modelValue: string;
  autoComplete: boolean;
  positivePrompt: string;
}>();
const emit = defineEmits<{
  "update:modelValue": [string];
  "update:autoComplete": [boolean];
  "update:positivePrompt": [string];
  notice: [string];
}>();

type Panel = "" | "preset" | "weight" | "chunk";
const panel = ref<Panel>("");
const presets = ref(loadPresets());
const chunks = ref(loadChunks());
const translating = ref(false);
const presetName = ref("");
const chunkName = ref("");
const chunkBody = ref("");
const chunkQuery = ref("");
const presetQuery = ref("");

const tags = computed(() => splitPromptTags(props.modelValue).map((raw) => parseWeightedTag(raw)));
const filteredPresets = computed(() => {
  const q = presetQuery.value.trim().toLowerCase();
  if (!q) return presets.value;
  return presets.value.filter((p) => p.name.toLowerCase().includes(q) || p.prompt.toLowerCase().includes(q));
});
const filteredChunks = computed(() => {
  const q = chunkQuery.value.trim().toLowerCase();
  if (!q) return chunks.value;
  return chunks.value.filter((c) => c.name.toLowerCase().includes(q) || c.content.toLowerCase().includes(q));
});

function setValue(next: string) {
  emit("update:modelValue", next);
}

function toggle(next: Panel) {
  panel.value = panel.value === next ? "" : next;
}

function applyPreset(item: PromptPreset) {
  emit("update:positivePrompt", item.prompt);
  emit("notice", `已套用预设「${item.name}」`);
  panel.value = "";
}

function saveCurrentPreset() {
  const prompt = props.positivePrompt.trim();
  if (!prompt) {
    emit("notice", "正面提示词为空");
    return;
  }
  const item = makePreset(presetName.value || `预设 ${presets.value.length + 1}`, prompt);
  presets.value = [item, ...presets.value];
  savePresets(presets.value);
  presetName.value = "";
  emit("notice", "已保存正面预设");
}

function removePreset(id: string) {
  presets.value = presets.value.filter((p) => p.id !== id);
  savePresets(presets.value);
}

async function translateNow() {
  if (translating.value) return;
  translating.value = true;
  try {
    const result = await translatePromptToEnglish(props.modelValue);
    setValue(result.text);
    emit("notice", result.note);
  } finally {
    translating.value = false;
  }
}

function normalizeNow() {
  const next = normalizePrompt(props.modelValue, DEFAULT_NORMALIZE_OPTIONS);
  setValue(next);
  emit("notice", "已标准化提示词");
}

function toggleSuggest() {
  const next = !props.autoComplete;
  saveAutoComplete(next);
  emit("update:autoComplete", next);
  emit("notice", next ? "提词已开启" : "提词已关闭");
}

function insertChunk(item: PromptChunk) {
  setValue(appendPromptChunk(props.modelValue, item.content));
  emit("notice", `已插入「${item.name}」`);
}

function saveChunk() {
  if (!chunkName.value.trim() || !chunkBody.value.trim()) {
    emit("notice", "请填写名称和提词内容");
    return;
  }
  chunks.value = [makeChunk(chunkName.value, chunkBody.value), ...chunks.value];
  saveChunks(chunks.value);
  chunkName.value = "";
  chunkBody.value = "";
  emit("notice", "已保存自定义提词");
}

function removeChunk(id: string) {
  chunks.value = chunks.value.filter((c) => c.id !== id);
  saveChunks(chunks.value);
}
</script>

<template>
  <div class="tb">
    <div class="tools">
      <button type="button" :class="{ on: panel === 'preset' }" @click="toggle('preset')">
        <NaiIcon name="layers" :size="13" />正面预设
      </button>
      <button type="button" :class="{ on: panel === 'weight' }" @click="toggle('weight')">
        <NaiIcon name="sliders" :size="13" />权重微调({{ tags.length }})
      </button>
      <button type="button" :disabled="translating" @click="translateNow">
        <NaiIcon name="globe" :size="13" />{{ translating ? "翻译中…" : "自动检测→英文" }}
      </button>
      <button type="button" @click="normalizeNow">
        <NaiIcon name="sparkle" :size="13" />标准化
      </button>
      <button type="button" :class="{ on: autoComplete }" @click="toggleSuggest">
        <NaiIcon name="bulb" :size="13" />提词：{{ autoComplete ? "开" : "关" }}
      </button>
      <button type="button" :class="{ on: panel === 'chunk' }" @click="toggle('chunk')">
        <NaiIcon name="plus" :size="13" />自定义提词
      </button>
    </div>
    <p class="hint">提词分「翻译」和「提示词库」两个页。点「标签」可多选或拖到另一个标签上合成；合成格式可选 {} 或 ::。</p>

    <section v-if="panel === 'preset'" class="panel">
      <header>
        <strong>正面提示词预设</strong>
        <span>选择预设替换当前正面提示词</span>
      </header>
      <div class="row">
        <input v-model="presetName" placeholder="预设名称（可选）" />
        <button type="button" class="act" @click="saveCurrentPreset">保存当前</button>
      </div>
      <input v-model="presetQuery" class="search" placeholder="搜索名称或提示词" />
      <p v-if="!filteredPresets.length" class="empty">还没有正面提示词预设</p>
      <button
        v-for="item in filteredPresets"
        :key="item.id"
        type="button"
        class="card"
        @click="applyPreset(item)"
      >
        <div>
          <b>{{ item.name }}</b>
          <small>{{ item.prompt }}</small>
        </div>
        <i @click.stop="removePreset(item.id)"><NaiIcon name="trash" :size="13" /></i>
      </button>
    </section>

    <section v-if="panel === 'weight'" class="panel">
      <header>
        <strong>权重微调</strong>
        <span>多选或拖入合并 · {} 或 :: · − / + 调权重</span>
      </header>
      <TagWeightList :model-value="modelValue" @update:model-value="setValue" />
    </section>

    <section v-if="panel === 'chunk'" class="panel">
      <header>
        <strong>自定义提词块</strong>
        <span>保存常用词组，插入而不是替换</span>
      </header>
      <input v-model="chunkName" placeholder="例如：角色服装" />
      <textarea v-model="chunkBody" rows="2" placeholder="例如：white dress, blue ribbon" />
      <button type="button" class="act" @click="saveChunk">保存提词</button>
      <input v-model="chunkQuery" class="search" placeholder="搜索提词名称或内容" />
      <p v-if="!filteredChunks.length" class="empty">还没有自定义提词</p>
      <button
        v-for="item in filteredChunks"
        :key="item.id"
        type="button"
        class="card"
        @click="insertChunk(item)"
      >
        <div>
          <b>{{ item.name }}</b>
          <small>{{ item.content }}</small>
        </div>
        <i @click.stop="removeChunk(item.id)"><NaiIcon name="trash" :size="13" /></i>
      </button>
    </section>
  </div>
</template>

<style scoped>
.tb { margin-top: 8px; }
.tools {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 6px;
}
.tools button {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  height: 32px;
  border: 0;
  border-radius: 999px;
  background: #1f2138;
  color: rgba(255,255,255,0.82);
  font-size: 12px;
  font-weight: 650;
}
.tools button:hover { background: #262948; color: #fff; }
.tools button.on { background: #2e3152; color: var(--heading); }
.tools button:disabled { opacity: 0.55; }
.hint {
  margin: 8px 0 0;
  color: rgba(255,255,255,0.38);
  font-size: 11px;
  line-height: 1.5;
}
.panel {
  margin-top: 8px;
  background: #16182d;
  border: 1px solid #2b2e4a;
  border-radius: 10px;
  padding: 10px;
  display: flex;
  flex-direction: column;
  gap: 7px;
}
.panel header { display: flex; flex-direction: column; gap: 2px; }
.panel strong { color: var(--heading); font-size: 13px; }
.panel header span, .empty { color: rgba(255,255,255,0.42); font-size: 11px; }
.row { display: flex; gap: 6px; }
input, textarea, .search {
  width: 100%;
  background: var(--bg0);
  border: 0;
  border-radius: 7px;
  padding: 7px 9px;
  color: #fff;
}
.act {
  height: 32px;
  border: 0;
  border-radius: 8px;
  background: #2e3152;
  color: var(--heading);
  padding: 0 10px;
  white-space: nowrap;
}
.card {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  width: 100%;
  text-align: left;
  background: #1c1e36;
  border: 0;
  border-radius: 8px;
  padding: 8px;
  color: #fff;
}
.card b { display: block; font-size: 12px; }
.card small {
  display: block;
  margin-top: 3px;
  color: rgba(255,255,255,0.45);
  font-size: 11px;
  line-height: 1.4;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
.card i {
  color: rgba(255,255,255,0.45);
  display: grid;
  place-items: center;
}
</style>
