<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { useAppStore } from "@/stores/app";
import InpaintCanvas from "@/components/InpaintCanvas.vue";
import NaiIcon from "@/components/NaiIcon.vue";

const store = useAppStore();
const paintRef = ref<{
  commitMask: () => boolean;
  restoreMask: () => void;
  clearMask: () => void;
  undo: () => void;
  redo: () => void;
  invertMask: () => void;
  canUndo: { value: boolean };
  canRedo: { value: boolean };
} | null>(null);
const showHelp = ref(false);

function saveAndClose() {
  paintRef.value?.commitMask();
  store.paintMode = true;
  store.paintEditorOpen = false;
  store.status = store.inpaintMask ? "已保存局部重绘蒙版，可在左侧继续编辑" : "未涂抹区域，已关闭编辑器";
}

function discardAndClose() {
  store.paintEditorOpen = false;
  store.status = store.inpaintMask ? "已关闭，保留上次保存的蒙版" : "已关闭涂抹编辑器";
}

function bumpSize(delta: number) {
  const step = store.brushShape === "round" ? 2 : 1;
  store.brushSize = Math.max(4, Math.min(120, store.brushSize + delta * step));
}

function onKey(ev: KeyboardEvent) {
  if (ev.key === "Escape") {
    ev.preventDefault();
    discardAndClose();
    return;
  }
  if (ev.key === "b" || ev.key === "B") store.brushErase = false;
  if (ev.key === "e" || ev.key === "E") store.brushErase = true;
  if (ev.key === "[") bumpSize(-1);
  if (ev.key === "]") bumpSize(1);
  if ((ev.ctrlKey || ev.metaKey) && ev.key.toLowerCase() === "z") {
    ev.preventDefault();
    ev.shiftKey ? paintRef.value?.redo() : paintRef.value?.undo();
  }
  if ((ev.ctrlKey || ev.metaKey) && ev.key.toLowerCase() === "y") {
    ev.preventDefault();
    paintRef.value?.redo();
  }
}

onMounted(() => {
  window.addEventListener("keydown", onKey);
});
onUnmounted(() => {
  window.removeEventListener("keydown", onKey);
});
</script>

<template>
  <div class="editor" role="dialog" aria-label="局部重绘">
    <header class="bar">
      <div class="tools">
        <button type="button" :class="{ on: !store.brushErase }" title="Draw · B" @click="store.brushErase = false">
          <NaiIcon name="brush" :size="16" />
        </button>
        <button type="button" :class="{ on: store.brushErase }" title="Eraser · E" @click="store.brushErase = true">
          <NaiIcon name="eraser" :size="16" />
        </button>
        <i class="swatch" aria-hidden="true" />
        <label class="size">
          Pen Size: {{ store.brushSize }}
          <input v-model.number="store.brushSize" type="range" min="4" max="80" />
        </label>
        <button type="button" :class="{ on: store.brushShape === 'round' }" title="圆形笔刷" @click="store.brushShape = 'round'">
          <i class="shape round" />
        </button>
        <button type="button" :class="{ on: store.brushShape === 'square' }" title="方形笔刷" @click="store.brushShape = 'square'">
          <i class="shape square" />
        </button>
        <button type="button" title="反转蒙版" @click="paintRef?.invertMask()">
          <NaiIcon name="invert" :size="16" />
        </button>
        <button type="button" title="快捷键" :class="{ on: showHelp }" @click="showHelp = !showHelp">
          <NaiIcon name="help" :size="16" />
        </button>
      </div>
      <div class="acts">
        <button type="button" class="save" @click="saveAndClose">Save & Close</button>
        <button type="button" class="x" title="关闭" @click="discardAndClose">
          <NaiIcon name="close" :size="16" />
        </button>
      </div>
    </header>
    <p v-if="showHelp" class="help">B 画笔 · E 橡皮 · [ ] 笔刷大小 · Ctrl+Z 撤销 · Esc 关闭</p>
    <div class="stage">
      <InpaintCanvas ref="paintRef" :active="true" />
    </div>
    <footer class="dock">
      <button type="button" title="撤销" @click="paintRef?.undo()"><NaiIcon name="undo" :size="16" /></button>
      <button type="button" title="重做" @click="paintRef?.redo()"><NaiIcon name="redo" :size="16" /></button>
      <button type="button" title="清除蒙版" @click="paintRef?.clearMask()"><NaiIcon name="trash" :size="16" /></button>
      <button type="button" title="反转" @click="paintRef?.invertMask()"><NaiIcon name="invert" :size="16" /></button>
      <button type="button" title="设为底图" @click="store.useCurrentAsI2i()"><NaiIcon name="clip" :size="16" /></button>
    </footer>
  </div>
</template>

<style scoped>
.editor {
  position: fixed;
  inset: 0;
  z-index: 40;
  display: flex;
  flex-direction: column;
  background: #0b0c16;
}
.bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  min-height: 52px;
  padding: 6px 16px;
  background: #11121f;
  border-bottom: 1px solid #22253f;
}
.tools, .acts, .dock {
  display: flex;
  align-items: center;
  gap: 6px;
}
.tools button, .x, .dock button {
  width: 34px;
  height: 34px;
  display: grid;
  place-items: center;
  background: transparent;
  border: 1px solid transparent;
  border-radius: 8px;
  color: rgba(255,255,255,0.78);
}
.tools button.on, .dock button.on {
  color: var(--heading);
  border-color: var(--heading);
  background: rgba(245, 243, 194, 0.08);
}
.swatch {
  width: 22px;
  height: 22px;
  border-radius: 4px;
  background: #111;
  border: 1px solid #3a3d58;
}
.size {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  margin: 0 6px;
  color: rgba(255,255,255,0.78);
  font-size: 13px;
  white-space: nowrap;
}
.size input { width: 160px; }
.shape {
  display: block;
  width: 12px;
  height: 12px;
  background: currentColor;
}
.shape.round { border-radius: 50%; }
.shape.square { border-radius: 2px; }
.save {
  background: var(--heading);
  color: var(--bg0);
  border: 0;
  border-radius: 999px;
  padding: 8px 16px;
  font-weight: 700;
}
.help {
  margin: 0;
  padding: 8px 16px;
  color: rgba(255,255,255,0.62);
  font-size: 12px;
}
.stage {
  flex: 1;
  min-height: 0;
  display: grid;
  place-items: center;
  overflow: auto;
  padding: 20px;
}
.dock {
  justify-content: center;
  min-height: 48px;
  border-top: 1px solid #22253f;
  background: #11121f;
}
</style>
