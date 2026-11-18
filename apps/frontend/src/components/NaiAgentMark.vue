<script setup lang="ts">
import { onMounted, ref, watch } from "vue";

const props = withDefaults(defineProps<{
  expanded?: boolean;
  button?: boolean;
}>(), {
  expanded: false,
  button: true,
});

defineEmits<{ toggle: [] }>();

const canvas = ref<HTMLCanvasElement | null>(null);

const SPRITE = [
  "....#....",
  "...+#=...",
  "..+###=..",
  ".+#####=.",
  "#+######=",
  "##wo#wo#=",
  "##oo#oo#=",
  "=#######=",
  ".=======.",
];

function paint() {
  const el = canvas.value;
  if (!el) return;
  const size = 56;
  el.width = size;
  el.height = size;
  const ctx = el.getContext("2d");
  if (!ctx) return;
  ctx.clearRect(0, 0, size, size);
  const cell = 4;
  const grid = 9 * cell;
  const ox = Math.floor((size - grid) / 2);
  const oy = Math.floor((size - grid) / 2);
  if (props.expanded) {
    ctx.fillStyle = "#2f7bff";
    ctx.beginPath();
    const cut = 6;
    const edge = size - 2;
    ctx.moveTo(2 + cut, 2);
    ctx.lineTo(edge - cut, 2);
    ctx.lineTo(edge, 2 + cut);
    ctx.lineTo(edge, edge - cut);
    ctx.lineTo(edge - cut, edge);
    ctx.lineTo(2 + cut, edge);
    ctx.lineTo(2, edge - cut);
    ctx.lineTo(2, 2 + cut);
    ctx.closePath();
    ctx.fill();
  }
  const body = props.expanded ? "#ffffff" : "#3d8bff";
  const highlight = props.expanded ? "#ffffff" : "#a5d4ff";
  const shadow = props.expanded ? "#bcd4ff" : "#1d58d6";
  const eyes = props.expanded ? "#0040b0" : "#04070d";
  for (let y = 0; y < SPRITE.length; y += 1) {
    const row = SPRITE[y];
    for (let x = 0; x < row.length; x += 1) {
      const ink = row[x];
      const color = ink === "#" ? body
        : ink === "+" ? highlight
          : ink === "=" ? shadow
            : ink === "o" ? eyes
              : ink === "w" ? "#ffffff"
                : "";
      if (!color) continue;
      ctx.fillStyle = color;
      ctx.fillRect(ox + x * cell, oy + y * cell, cell, cell);
    }
  }
}

onMounted(paint);
watch(() => props.expanded, paint);
</script>

<template>
  <button
    v-if="button"
    type="button"
    class="mark"
    :class="{ on: expanded }"
    title="NAI Agent"
    aria-label="NAI Agent"
    :aria-pressed="expanded"
    @click="$emit('toggle')"
  >
    <canvas ref="canvas" />
  </button>
  <span v-else class="mark static" aria-hidden="true">
    <canvas ref="canvas" />
  </span>
</template>

<style scoped>
.mark {
  width: 28px;
  height: 28px;
  padding: 0;
  border: 0;
  border-radius: 6px;
  background: transparent;
  display: inline-grid;
  place-items: center;
  flex: none;
}
.mark:hover { background: rgba(47, 123, 255, 0.16); }
.mark.on { background: transparent; }
.mark canvas {
  width: 28px;
  height: 28px;
  image-rendering: pixelated;
}
.static { pointer-events: none; }
</style>
