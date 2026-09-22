<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useAppStore } from "@/stores/app";

const props = defineProps<{
  active?: boolean;
}>();

const store = useAppStore();
const wrap = ref<HTMLDivElement | null>(null);
const canvas = ref<HTMLCanvasElement | null>(null);
const drawing = ref(false);
const last = ref<{ x: number; y: number } | null>(null);
const cursor = ref({ x: 0, y: 0, on: false, scale: 1 });
const undoStack = ref<ImageData[]>([]);
const redoStack = ref<ImageData[]>([]);
const canUndo = computed(() => undoStack.value.length > 0);
const canRedo = computed(() => redoStack.value.length > 0);

const source = computed(() => store.i2iImage || store.previewUrl);
const paintOn = computed(() => (props.active ?? store.paintEditorOpen) && Boolean(source.value));
const cursorSize = computed(() => Math.max(6, store.brushSize * cursor.value.scale));

function point(ev: PointerEvent) {
  const el = canvas.value;
  if (!el) return null;
  const rect = el.getBoundingClientRect();
  if (!rect.width || !rect.height) return null;
  cursor.value = {
    x: ev.clientX - rect.left,
    y: ev.clientY - rect.top,
    on: true,
    scale: rect.width / el.width,
  };
  return {
    x: ((ev.clientX - rect.left) / rect.width) * el.width,
    y: ((ev.clientY - rect.top) / rect.height) * el.height,
  };
}

function stamp(ctx: CanvasRenderingContext2D, x: number, y: number) {
  const radius = store.brushSize / 2;
  if (store.brushShape === "square") {
    ctx.fillRect(x - radius, y - radius, store.brushSize, store.brushSize);
    return;
  }
  ctx.beginPath();
  ctx.arc(x, y, radius, 0, Math.PI * 2);
  ctx.fill();
}

function snapshot() {
  const el = canvas.value;
  const ctx = el?.getContext("2d");
  if (!el || !ctx) return;
  undoStack.value.push(ctx.getImageData(0, 0, el.width, el.height));
  if (undoStack.value.length > 40) undoStack.value.shift();
  redoStack.value = [];
}

function undo() {
  const el = canvas.value;
  const ctx = el?.getContext("2d");
  if (!el || !ctx || !undoStack.value.length) return;
  redoStack.value.push(ctx.getImageData(0, 0, el.width, el.height));
  ctx.putImageData(undoStack.value.pop()!, 0, 0);
}

function redo() {
  const el = canvas.value;
  const ctx = el?.getContext("2d");
  if (!el || !ctx || !redoStack.value.length) return;
  undoStack.value.push(ctx.getImageData(0, 0, el.width, el.height));
  ctx.putImageData(redoStack.value.pop()!, 0, 0);
}

function invertMask() {
  const el = canvas.value;
  const ctx = el?.getContext("2d");
  if (!el || !ctx) return;
  snapshot();
  const data = ctx.getImageData(0, 0, el.width, el.height);
  for (let i = 0; i < data.data.length; i += 4) {
    const on = data.data[i + 3] > 8;
    data.data[i] = on ? 0 : 124;
    data.data[i + 1] = on ? 0 : 108;
    data.data[i + 2] = on ? 0 : 232;
    data.data[i + 3] = on ? 0 : 255;
  }
  ctx.putImageData(data, 0, 0);
}

function stroke(from: { x: number; y: number }, to: { x: number; y: number }) {
  const el = canvas.value;
  const ctx = el?.getContext("2d");
  if (!el || !ctx) return;
  ctx.save();
  if (store.brushErase) {
    ctx.globalCompositeOperation = "destination-out";
    ctx.fillStyle = "rgba(0,0,0,1)";
  } else {
    ctx.globalCompositeOperation = "source-over";
    ctx.fillStyle = "#7c6ce8";
  }
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const dist = Math.hypot(dx, dy);
  const gap = Math.max(1, store.brushSize / 4);
  const steps = Math.max(1, Math.ceil(dist / gap));
  for (let i = 0; i <= steps; i++) {
    stamp(ctx, from.x + (dx * i) / steps, from.y + (dy * i) / steps);
  }
  ctx.restore();
}

function commitMask() {
  const el = canvas.value;
  if (!el) {
    store.inpaintMask = "";
    return false;
  }
  const ctx = el.getContext("2d");
  if (!ctx) return false;
  const data = ctx.getImageData(0, 0, el.width, el.height);
  let any = false;
  const binary = new Uint8ClampedArray(data.data.length);
  for (let i = 0; i < data.data.length; i += 4) {
    const on = data.data[i + 3] > 8;
    const v = on ? 255 : 0;
    binary[i] = v;
    binary[i + 1] = v;
    binary[i + 2] = v;
    binary[i + 3] = v;
    any ||= on;
  }
  if (!any) {
    store.inpaintMask = "";
    return false;
  }
  const out = document.createElement("canvas");
  out.width = el.width;
  out.height = el.height;
  out.getContext("2d")?.putImageData(new ImageData(binary, el.width, el.height), 0, 0);
  store.inpaintMask = out.toDataURL("image/png");
  return true;
}

function paintPurpleFromMask(img: HTMLImageElement) {
  const el = canvas.value;
  const ctx = el?.getContext("2d");
  if (!el || !ctx) return;
  ctx.clearRect(0, 0, el.width, el.height);
  ctx.drawImage(img, 0, 0, el.width, el.height);
  ctx.globalCompositeOperation = "source-in";
  ctx.fillStyle = "#7c6ce8";
  ctx.fillRect(0, 0, el.width, el.height);
  ctx.globalCompositeOperation = "source-over";
}

function restoreMask() {
  const el = canvas.value;
  const ctx = el?.getContext("2d");
  if (!el || !ctx) return;
  ctx.clearRect(0, 0, el.width, el.height);
  if (!store.inpaintMask) return;
  const img = new Image();
  img.onload = () => paintPurpleFromMask(img);
  img.src = store.inpaintMask;
}

function fitCanvas() {
  const el = canvas.value;
  const img = wrap.value?.querySelector("img");
  if (!el || !img) return;
  const width = img.naturalWidth || store.params.width || 832;
  const height = img.naturalHeight || store.params.height || 1216;
  if (el.width !== width || el.height !== height) {
    el.width = width;
    el.height = height;
  }
  restoreMask();
}

function onDown(ev: PointerEvent) {
  if (!paintOn.value) return;
  ev.preventDefault();
  snapshot();
  drawing.value = true;
  last.value = point(ev);
  if (last.value) stroke(last.value, last.value);
  (ev.currentTarget as HTMLElement).setPointerCapture(ev.pointerId);
}

function onMove(ev: PointerEvent) {
  const next = point(ev);
  if (!drawing.value || !next || !last.value) return;
  stroke(last.value, next);
  last.value = next;
}

function onUp() {
  drawing.value = false;
  last.value = null;
}

function onLeave() {
  if (!drawing.value) cursor.value.on = false;
}

function clearMask() {
  const el = canvas.value;
  const ctx = el?.getContext("2d");
  if (el && ctx) {
    snapshot();
    ctx.clearRect(0, 0, el.width, el.height);
  }
  store.inpaintMask = "";
}

watch(source, async () => {
  await nextTick();
  requestAnimationFrame(fitCanvas);
});

watch(
  () => store.paintEditorOpen,
  async (open) => {
    if (!open) return;
    await nextTick();
    requestAnimationFrame(fitCanvas);
  },
);

onMounted(() => {
  window.addEventListener("pointerup", onUp);
  requestAnimationFrame(fitCanvas);
});
onUnmounted(() => {
  window.removeEventListener("pointerup", onUp);
});

defineExpose({ clearMask, fitCanvas, commitMask, restoreMask, undo, redo, invertMask, canUndo, canRedo });
</script>

<template>
  <div v-if="source" ref="wrap" class="stage" :class="{ paint: paintOn }">
    <div class="frame">
      <img :src="source" alt="" @load="fitCanvas" />
      <canvas
        ref="canvas"
        class="mask"
        :class="{ active: paintOn }"
        @pointerdown="onDown"
        @pointermove="onMove"
        @pointerleave="onLeave"
      />
      <i
        v-if="paintOn && cursor.on"
        class="cursor"
        :class="{ erase: store.brushErase }"
        :style="{
          width: `${cursorSize}px`,
          height: `${cursorSize}px`,
          transform: `translate(${cursor.x - cursorSize / 2}px, ${cursor.y - cursorSize / 2}px)`,
          borderRadius: store.brushShape === 'square' ? '3px' : '50%',
        }"
      />
    </div>
  </div>
</template>

<style scoped>
.stage {
  max-width: 100%;
  max-height: 100%;
  display: grid;
  place-items: center;
}
.frame { position: relative; display: inline-block; max-width: 100%; max-height: 100%; }
.stage img {
  max-width: min(92vw, 1100px);
  max-height: calc(100vh - 140px);
  object-fit: contain;
  display: block;
}
.mask {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  opacity: 0.42;
}
.mask.active {
  pointer-events: auto;
  cursor: none;
}
.cursor {
  position: absolute;
  left: 0;
  top: 0;
  border: 1.5px solid rgba(255,255,255,0.92);
  border-radius: 50%;
  box-shadow: 0 0 0 1px rgba(0,0,0,0.35);
  pointer-events: none;
  background: rgba(124, 108, 232, 0.16);
}
.cursor.erase {
  background: rgba(255,255,255,0.08);
  border-color: #fff;
}
.paint { user-select: none; touch-action: none; }

</style>
