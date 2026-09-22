<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import {
  bumpTagWeight,
  extractPartAndMerge,
  extractPartFromGroup,
  formatTagWeight,
  mergeTagWithNext,
  mergeTags,
  parseWeightedTag,
  removeTagFromPrompt,
  renamePartInGroup,
  renameTagInPrompt,
  resetTagWeight,
  splitNumericGroup,
  splitPromptTags,
  toggleNumericEmphasis,
  type MergeStyle,
} from "@/utils/promptWeight";

const props = defineProps<{ modelValue: string }>();
const emit = defineEmits<{ "update:modelValue": [string] }>();

type DragState =
  | { kind: "tag"; index: number }
  | { kind: "part"; index: number; part: number };

const STYLE_KEY = "nai-tag-merge-style";
const chips = computed(() => splitPromptTags(props.modelValue).map((raw) => parseWeightedTag(raw)));
const selected = ref<Set<number>>(new Set());
const mergeStyle = ref<MergeStyle>(localStorage.getItem(STYLE_KEY) === "brace" ? "brace" : "numeric");
const drag = ref<DragState | null>(null);
const dropOn = ref<number | null>(null);
const ghost = ref({ show: false, x: 0, y: 0, text: "" });
const listEl = ref<HTMLElement | null>(null);
const editing = ref<{ index: number; part: number | null; draft: string } | null>(null);

let startX = 0;
let startY = 0;
let moved = false;
let swallowClick = false;

const selectedCount = computed(() => selected.value.size);
const allSelected = computed(() => chips.value.length > 0 && selected.value.size === chips.value.length);
const dragFrom = computed(() => (drag.value?.kind === "tag" ? drag.value.index : null));

watch(
  () => chips.value.length,
  (len) => {
    selected.value = new Set([...selected.value].filter((i) => i < len));
  },
);

function setValue(next: string) {
  emit("update:modelValue", next);
}

function setStyle(next: MergeStyle) {
  mergeStyle.value = next;
  localStorage.setItem(STYLE_KEY, next);
}

function toggleSelect(index: number) {
  const next = new Set(selected.value);
  if (next.has(index)) next.delete(index);
  else next.add(index);
  selected.value = next;
}

function toggleAll() {
  selected.value = allSelected.value ? new Set() : new Set(chips.value.map((_, i) => i));
}

function bumpWeight(index: number, delta: number) {
  setValue(bumpTagWeight(props.modelValue, index, delta));
}

function resetWeight(index: number) {
  setValue(resetTagWeight(props.modelValue, index));
}

function toggleNumeric(index: number) {
  setValue(toggleNumericEmphasis(props.modelValue, index));
}

function mergeNext(index: number) {
  setValue(mergeTagWithNext(props.modelValue, index, mergeStyle.value));
  selected.value = new Set();
}

function mergeSelected() {
  if (selected.value.size < 2) return;
  setValue(mergeTags(props.modelValue, [...selected.value], mergeStyle.value));
  selected.value = new Set();
}

function splitChip(index: number) {
  setValue(splitNumericGroup(props.modelValue, index));
}

function extractPart(index: number, partIndex: number) {
  setValue(extractPartFromGroup(props.modelValue, index, partIndex));
}

function removeChip(index: number) {
  setValue(removeTagFromPrompt(props.modelValue, index));
}

function isEditing(index: number, part: number | null) {
  return editing.value?.index === index && editing.value.part === part;
}

function startEdit(index: number, part: number | null) {
  const tag = chips.value[index];
  if (!tag) return;
  editing.value = { index, part, draft: part == null ? tag.core : (tag.parts[part] ?? "") };
  void nextTick(() => {
    const el = listEl.value?.querySelector<HTMLTextAreaElement>("textarea.edit");
    if (!el) return;
    el.focus();
    el.select();
  });
}

function cancelEdit() {
  editing.value = null;
}

function commitEdit() {
  const ed = editing.value;
  if (!ed) return;
  editing.value = null;
  const next =
    ed.part == null
      ? renameTagInPrompt(props.modelValue, ed.index, ed.draft)
      : renamePartInGroup(props.modelValue, ed.index, ed.part, ed.draft);
  if (next !== props.modelValue) setValue(next);
}

function setDraft(e: Event) {
  if (editing.value) editing.value.draft = (e.target as HTMLTextAreaElement).value;
}

function onEditKey(e: KeyboardEvent) {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    (e.target as HTMLTextAreaElement).blur();
  } else if (e.key === "Escape") {
    e.preventDefault();
    cancelEdit();
  }
}

function ghostText(state: DragState): string {
  if (state.kind === "part") return chips.value[state.index]?.parts[state.part] ?? "";
  return chips.value[state.index]?.core ?? "";
}

function rowFromPoint(x: number, y: number): number | null {
  const els = document.elementsFromPoint(x, y);
  for (const el of els) {
    if (!(el instanceof HTMLElement)) continue;
    const row = el.closest("[data-tag-index]");
    if (!(row instanceof HTMLElement) || !listEl.value?.contains(row)) continue;
    const n = Number(row.dataset.tagIndex);
    return Number.isInteger(n) ? n : null;
  }
  return null;
}

function bindMove() {
  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("pointerup", onPointerUp);
  window.addEventListener("pointercancel", onPointerUp);
}

function unbindMove() {
  window.removeEventListener("pointermove", onPointerMove);
  window.removeEventListener("pointerup", onPointerUp);
  window.removeEventListener("pointercancel", onPointerUp);
}

function beginDrag(e: PointerEvent, state: DragState) {
  if (e.button !== 0) return;
  e.stopPropagation();
  drag.value = state;
  dropOn.value = null;
  startX = e.clientX;
  startY = e.clientY;
  moved = false;
  ghost.value = { show: false, x: e.clientX, y: e.clientY, text: ghostText(state) };
  bindMove();
}

function onTagPointerDown(e: PointerEvent, index: number) {
  const t = e.target as HTMLElement;
  if (t.closest("button, input, textarea, .parts")) return;
  beginDrag(e, { kind: "tag", index });
}

function onPartPointerDown(e: PointerEvent, index: number, part: number) {
  const t = e.target as HTMLElement;
  if (t.closest("button, textarea")) return;
  beginDrag(e, { kind: "part", index, part });
}

function onPointerMove(e: PointerEvent) {
  if (!drag.value) return;
  const dx = e.clientX - startX;
  const dy = e.clientY - startY;
  if (!moved && dx * dx + dy * dy < 25) return;
  moved = true;
  const over = rowFromPoint(e.clientX, e.clientY);
  const self = drag.value.kind === "tag" ? drag.value.index : -1;
  dropOn.value = over != null && over !== self ? over : null;
  ghost.value = { show: true, x: e.clientX, y: e.clientY, text: ghostText(drag.value) };
}

function finishMerge(from: number, to: number) {
  const indices = selected.value.has(from) && selected.value.size > 1 ? [...selected.value, to] : [from, to];
  setValue(mergeTags(props.modelValue, indices, mergeStyle.value, to));
  selected.value = new Set();
}

function onPointerUp(e: PointerEvent) {
  const state = drag.value;
  const to = dropOn.value;
  const didMove = moved;
  unbindMove();
  drag.value = null;
  dropOn.value = null;
  ghost.value = { show: false, x: 0, y: 0, text: "" };
  if (!state) return;
  if (!didMove) {
    startEdit(state.index, state.kind === "part" ? state.part : null);
    return;
  }
  swallowClick = true;
  window.setTimeout(() => {
    swallowClick = false;
  }, 0);
  if (to == null) return;
  if (state.kind === "tag") finishMerge(state.index, to);
  else setValue(extractPartAndMerge(props.modelValue, state.index, state.part, to, mergeStyle.value));
  void e;
}

function onRowClick(e: MouseEvent) {
  if (!swallowClick) return;
  e.preventDefault();
  e.stopPropagation();
}

onBeforeUnmount(unbindMove);
</script>

<template>
  <div ref="listEl" class="list">
    <div class="merge-bar">
      <label class="pick">
        <input type="checkbox" :checked="allSelected" :disabled="!chips.length" @change="toggleAll" />
        多选{{ selectedCount ? ` ${selectedCount}` : "" }}
      </label>
      <span class="lbl">合成格式</span>
      <button type="button" class="wide" :class="{ on: mergeStyle === 'brace' }" title="合成后用 {标签1, 标签2}" @click="setStyle('brace')">{}</button>
      <button type="button" class="wide" :class="{ on: mergeStyle === 'numeric' }" title="合成后用 1.1::标签1,标签2::" @click="setStyle('numeric')">::</button>
      <button type="button" class="act" :disabled="selectedCount < 2" @click="mergeSelected">
        合成所选{{ selectedCount >= 2 ? ` ${selectedCount}` : "" }}
      </button>
    </div>
    <p class="tip">点文字可改词。按住拖到另一条上合并。合成组里每个词都能单独改或拆出。</p>
    <p v-if="!chips.length" class="empty">先输入逗号分隔的提示词</p>
    <div
      v-for="(tag, i) in chips"
      :key="`${tag.raw}-${i}`"
      class="wgt"
      :class="{ sel: selected.has(i), drop: dropOn === i, dragging: dragFrom === i }"
      :data-tag-index="i"
      @pointerdown="onTagPointerDown($event, i)"
      @click.capture="onRowClick"
    >
      <input class="chk" type="checkbox" :checked="selected.has(i)" title="选中后一起合成" @change="toggleSelect(i)" />
      <div class="body">
        <textarea
          v-if="isEditing(i, null)"
          class="edit"
          rows="1"
          :value="editing?.draft"
          @input="setDraft"
          @keydown="onEditKey"
          @blur="commitEdit"
          @pointerdown.stop
        />
        <p v-else class="text">{{ tag.core }}</p>
        <div v-if="tag.parts.length > 1" class="parts">
          <span
            v-for="(part, pi) in tag.parts"
            :key="`${part}-${pi}`"
            class="part"
            @pointerdown.stop="onPartPointerDown($event, i, pi)"
          >
            <textarea
              v-if="isEditing(i, pi)"
              class="edit mini"
              rows="1"
              :value="editing?.draft"
              @input="setDraft"
              @keydown="onEditKey"
              @blur="commitEdit"
              @pointerdown.stop
            />
            <b v-else>{{ part }}</b>
            <button type="button" title="只拆出这个词" @click.stop="extractPart(i, pi)">拆</button>
          </span>
        </div>
        <div class="acts">
          <span>{{ formatTagWeight(tag) }}</span>
          <button type="button" title="减弱" @click="bumpWeight(i, -1)">−</button>
          <button type="button" title="重置权重" @click="resetWeight(i)">0</button>
          <button type="button" title="加强" @click="bumpWeight(i, 1)">+</button>
          <button type="button" class="wide" :class="{ on: tag.numeric != null }" title="切换 2::标签::" @click="toggleNumeric(i)">::</button>
          <button v-if="tag.parts.length > 1" type="button" class="wide" title="全部拆开" @click="splitChip(i)">全拆</button>
          <button v-else type="button" class="wide" title="与下一个合成" :disabled="i >= chips.length - 1" @click="mergeNext(i)">合</button>
          <button type="button" class="del" title="删除这个提示词" @click="removeChip(i)">×</button>
        </div>
      </div>
    </div>
    <Teleport to="body">
      <div v-if="ghost.show" class="ghost" :style="{ left: `${ghost.x + 10}px`, top: `${ghost.y + 10}px` }">
        {{ ghost.text }}
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.list { display: grid; gap: 6px; }
.merge-bar {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px;
}
.pick {
  display: flex;
  align-items: center;
  gap: 5px;
  color: rgba(255,255,255,0.72);
  font-size: 12px;
}
.lbl { color: rgba(255,255,255,0.42); font-size: 11px; }
.tip, .empty { margin: 0; color: rgba(255,255,255,0.42); font-size: 11px; line-height: 1.45; }
.act {
  height: 24px;
  padding: 0 8px;
  border: 0;
  border-radius: 6px;
  background: #2e3152;
  color: var(--heading);
  font-size: 11px;
}
.act:disabled { opacity: 0.4; }
.wgt {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  background: #1c1e36;
  border: 1px solid transparent;
  border-radius: 8px;
  padding: 8px;
  cursor: grab;
  touch-action: none;
  user-select: none;
}
.wgt.sel { background: #262948; }
.wgt.drop { border-color: #7c83d6; background: #2e3152; }
.wgt.dragging { opacity: 0.55; cursor: grabbing; }
.chk {
  margin-top: 2px;
  flex-shrink: 0;
  accent-color: #7c83d6;
}
.body {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.text {
  margin: 0;
  font-size: 12px;
  line-height: 1.45;
  overflow-wrap: anywhere;
  word-break: break-word;
  white-space: normal;
  cursor: text;
  border-radius: 6px;
}
.text:hover { background: rgba(255,255,255,0.04); }
.edit {
  width: 100%;
  min-height: 28px;
  resize: none;
  background: #14162b;
  border: 1px solid #3d4270;
  border-radius: 6px;
  padding: 4px 6px;
  color: #fff;
  font-size: 12px;
  line-height: 1.45;
  cursor: text;
  user-select: text;
}
.part b { font-weight: 400; cursor: text; }
.edit.mini { min-height: 22px; width: 8em; max-width: 42vw; }
.acts {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 4px;
}
.acts > span { font-size: 11px; color: var(--heading); min-width: 2.4em; text-align: right; }
.wgt button, .merge-bar .wide {
  width: 24px;
  height: 24px;
  border: 0;
  border-radius: 6px;
  background: #2a2d4a;
  color: #fff;
}
.wide { width: auto; min-width: 24px; padding: 0 5px; font-size: 11px; }
.wgt button.on, .merge-bar .wide.on { background: #3d4270; color: var(--heading); }
.wgt button:disabled { opacity: 0.35; }
.del { color: var(--danger); }
.wgt button, .wgt input, .part { cursor: pointer; }
.parts {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}
.part {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  max-width: 100%;
  padding: 2px 4px 2px 7px;
  border-radius: 999px;
  background: #14162b;
  color: rgba(255,255,255,0.86);
  font-size: 11px;
  cursor: grab;
  touch-action: none;
  user-select: none;
}
.part button {
  width: 20px;
  height: 20px;
  font-size: 10px;
}
.ghost {
  position: fixed;
  z-index: 120;
  pointer-events: none;
  max-width: 240px;
  padding: 4px 8px;
  border-radius: 8px;
  background: #3d4270;
  color: #fff;
  font-size: 12px;
  box-shadow: 0 8px 20px rgba(0,0,0,0.35);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
