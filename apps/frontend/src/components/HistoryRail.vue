<script setup lang="ts">
import { ref, watch } from "vue";
import { useAppStore } from "@/stores/app";
import { readImageDataUrl } from "@/api/tauri";
import NaiIcon from "@/components/NaiIcon.vue";

const store = useAppStore();
const thumbs = ref<Record<string, string>>({});

async function loadThumbs() {
  const next = { ...thumbs.value };
  for (const item of store.history.slice(0, 32)) {
    if (next[item.id]) continue;
    try {
      next[item.id] = await readImageDataUrl(item.path);
    } catch {
      next[item.id] = "";
    }
  }
  thumbs.value = next;
}

watch(
  () => [store.historyOpen, store.history.map((h) => h.id).join("|")] as const,
  () => {
    if (store.historyOpen) void loadThumbs();
  },
  { immediate: true },
);
</script>

<template>
  <aside class="rail" :class="{ open: store.historyOpen }">
    <button class="tab" type="button" title="History" @click="store.historyOpen = !store.historyOpen">
      History
      <NaiIcon name="history" :size="12" />
    </button>
    <div v-show="store.historyOpen" class="body">
      <header>
        <strong>History</strong>
        <button class="ghost" type="button" @click="store.showSessionDialog = true">Sessions</button>
      </header>
      <p v-if="!store.history.length" class="hint">生成后的图片会出现在这里。</p>
      <div v-for="item in store.history" :key="item.id" class="item">
        <button class="thumb" type="button" @click="store.showHistory(item)" @dblclick="store.applyHistory(item)">
          <img v-if="thumbs[item.id]" :src="thumbs[item.id]" alt="" />
        </button>
        <div class="meta">
          <div class="title">{{ item.width }}×{{ item.height }}</div>
          <div class="sub">{{ item.prompt || "(no prompt)" }}</div>
          <button class="import" type="button" @click.stop="store.importFromHistory(item)">导入元数据</button>
        </div>
      </div>
    </div>
  </aside>
</template>

<style scoped>
.rail {
  position: fixed;
  top: 52px;
  right: 0;
  bottom: 0;
  z-index: 24;
  width: 0;
  pointer-events: none;
}
.tab {
  pointer-events: auto;
  position: absolute;
  top: 12px;
  right: 0;
  z-index: 26;
  writing-mode: vertical-rl;
  transform: rotate(180deg);
  background: var(--bg1);
  color: #fff;
  border: 1px solid var(--bg3);
  border-right: 0;
  border-radius: 0 6px 6px 0;
  padding: 14px 8px;
  letter-spacing: 0.4px;
  display: flex;
  align-items: center;
  gap: 6px;
}
.rail.open .tab { right: 280px; }
.body {
  pointer-events: auto;
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  width: 280px;
  overflow: auto;
  background: var(--bg1);
  border-left: 1px solid var(--bg3);
  padding: 14px 12px;
  box-shadow: -12px 0 28px rgba(0, 0, 0, 0.28);
}
header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px; }
.ghost {
  background: transparent;
  border: 1px solid var(--bg3);
  border-radius: 4px;
  padding: 4px 8px;
  color: var(--muted);
}
.hint { color: var(--muted); font-size: 13px; }
.item {
  display: grid;
  grid-template-columns: 48px 1fr;
  gap: 8px;
  width: 100%;
  text-align: left;
  background: transparent;
  border: 0;
  border-radius: 6px;
  padding: 6px;
  margin-bottom: 6px;
}
.item:hover { background: var(--bg3); }
.thumb {
  padding: 0;
  border: 0;
  background: transparent;
  width: 48px;
  height: 64px;
}
.item img { width: 48px; height: 64px; object-fit: cover; border-radius: 4px; }
.meta { min-width: 0; }
.title { font-size: 13px; }
.sub { color: var(--muted); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.import {
  margin-top: 6px;
  background: #f5f3c2;
  color: #0e0f21;
  border: 0;
  border-radius: 4px;
  padding: 3px 8px;
  font-size: 11px;
  font-weight: 700;
}
</style>
