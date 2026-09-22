<script setup lang="ts">
import { computed } from "vue";
import { useAppStore } from "@/stores/app";

const store = useAppStore();
const PHI = 0.38196601125;

const lines = computed(() => {
  const mode = store.overlayGuide;
  if (mode === "none") return { v: [] as number[], h: [] as number[] };
  if (mode === "thirds") return { v: [1 / 3, 2 / 3], h: [1 / 3, 2 / 3] };
  if (mode === "phi") return { v: [PHI, 1 - PHI], h: [PHI, 1 - PHI] };
  const cols = Math.max(2, Math.min(12, store.overlayCols));
  const rows = Math.max(2, Math.min(12, store.overlayRows));
  return {
    v: Array.from({ length: cols - 1 }, (_, i) => (i + 1) / cols),
    h: Array.from({ length: rows - 1 }, (_, i) => (i + 1) / rows),
  };
});
</script>

<template>
  <svg v-if="store.overlayGuide !== 'none'" class="guide" viewBox="0 0 100 100" preserveAspectRatio="none">
    <line v-for="x in lines.v" :key="`v${x}`" :x1="x * 100" y1="0" :x2="x * 100" y2="100" />
    <line v-for="y in lines.h" :key="`h${y}`" x1="0" :y1="y * 100" x2="100" :y2="y * 100" />
  </svg>
</template>

<style scoped>
.guide {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  pointer-events: none;
  z-index: 2;
}
line {
  stroke: rgba(255, 255, 255, 0.42);
  stroke-width: 0.35;
}
</style>
