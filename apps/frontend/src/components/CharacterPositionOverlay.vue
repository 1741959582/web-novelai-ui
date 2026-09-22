<script setup lang="ts">
import { computed, ref } from "vue";
import { useAppStore } from "@/stores/app";

const store = useAppStore();
const dragging = ref<string | null>(null);
const activeId = ref(store.characters[0]?.id ?? "");

const active = computed(() => store.characters.find((c) => c.id === activeId.value) ?? store.characters[0]);

function chipLabel(index: number, prompt: string) {
  const token = prompt.split(/[,\n]/)[0]?.trim() || `Character ${index + 1}`;
  return `${index + 1} ${token}`;
}

function setFromEvent(ev: PointerEvent, id: string) {
  const el = ev.currentTarget as HTMLElement;
  const rect = el.getBoundingClientRect();
  if (!rect.width || !rect.height) return;
  store.updateCharacter(id, {
    x: Math.min(1, Math.max(0, (ev.clientX - rect.left) / rect.width)),
    y: Math.min(1, Math.max(0, (ev.clientY - rect.top) / rect.height)),
    useCoords: true,
  });
}

function onDown(ev: PointerEvent) {
  const id = (ev.target as HTMLElement).dataset.charId || active.value?.id;
  if (!id) return;
  activeId.value = id;
  dragging.value = id;
  setFromEvent(ev, id);
  (ev.currentTarget as HTMLElement).setPointerCapture(ev.pointerId);
}

function onMove(ev: PointerEvent) {
  if (!dragging.value) return;
  setFromEvent(ev, dragging.value);
}

function onUp() {
  dragging.value = null;
}
</script>

<template>
  <div
    class="overlay"
    @pointerdown="onDown"
    @pointermove="onMove"
    @pointerup="onUp"
    @pointercancel="onUp"
  >
    <div class="dim" />
    <button
      v-for="(c, i) in store.characters"
      :key="c.id"
      type="button"
      class="pin"
      :class="{ on: c.id === active?.id }"
      :data-char-id="c.id"
      :style="{ left: `${c.x * 100}%`, top: `${c.y * 100}%` }"
    >
      {{ i + 1 }}
    </button>
    <div class="chips">
      <button
        v-for="(c, i) in store.characters"
        :key="c.id"
        type="button"
        :class="{ on: c.id === active?.id }"
        @pointerdown.stop="activeId = c.id"
      >
        {{ chipLabel(i, c.prompt) }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.overlay {
  position: absolute;
  inset: 0;
  z-index: 3;
  touch-action: none;
  user-select: none;
}
.dim {
  position: absolute;
  inset: 0;
  background: rgba(10, 12, 24, 0.18);
  pointer-events: none;
}
.pin {
  position: absolute;
  width: 28px;
  height: 28px;
  transform: translate(-50%, -50%);
  border-radius: 50%;
  border: 2px solid #fff;
  background: rgba(255,255,255,0.92);
  color: #1a1c2e;
  font-weight: 800;
  font-size: 13px;
  box-shadow: 0 0 0 1px rgba(0,0,0,0.25);
  z-index: 4;
}
.pin.on { background: #fff; box-shadow: 0 0 0 3px rgba(255,255,255,0.35); }
.chips {
  position: absolute;
  top: 12px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  gap: 8px;
  pointer-events: auto;
  z-index: 5;
}
.chips button {
  max-width: 220px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  border: 0;
  border-radius: 999px;
  padding: 6px 12px;
  background: #fff;
  color: #1a1c2e;
  font-size: 12px;
  font-weight: 700;
}
.chips button.on { outline: 2px solid #f5f3c2; }
</style>
