<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { readImageDataUrl } from "@/api/tauri";
import { useAppStore } from "@/stores/app";
import type { SessionRecord } from "@/types/nai";

const store = useAppStore();
const thumbs = ref<Record<string, string>>({});

const items = computed(() => store.sessions);

function formatSaved(iso: string) {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  let h = d.getHours();
  const min = String(d.getMinutes()).padStart(2, "0");
  const ampm = h >= 12 ? "pm" : "am";
  h = h % 12 || 12;
  return `${y}/${m}/${day} @ ${String(h).padStart(2, "0")}:${min}${ampm}`;
}

async function loadThumbs(list: SessionRecord[]) {
  for (const s of list) {
    if (!s.thumbnailPath || thumbs.value[s.id]) continue;
    try {
      thumbs.value[s.id] = await readImageDataUrl(s.thumbnailPath);
    } catch {
      thumbs.value[s.id] = "";
    }
  }
}

onMounted(() => {
  void loadThumbs(items.value);
});

watch(items, (list) => {
  void loadThumbs(list);
});

function dismiss() {
  if (!store.currentSessionId) {
    void store.startNewSession();
    return;
  }
  store.showSessionDialog = false;
}

async function onLoad(id: string) {
  await store.loadSession(id);
}

async function onDelete(id: string) {
  await store.removeSession(id);
  delete thumbs.value[id];
}
</script>

<template>
  <div class="backdrop" @click.self="dismiss">
    <section class="panel" role="dialog" aria-labelledby="session-title">
      <h1 id="session-title">Load a previous session?</h1>
      <p class="hint">
        Sessions are stored locally on this computer and stay available after closing the app.
      </p>
      <div v-if="!items.length" class="empty">还没有本地会话。</div>
      <ul class="list">
        <li v-for="s in items" :key="s.id">
          <div class="thumb">
            <img v-if="thumbs[s.id]" :src="thumbs[s.id]" alt="" />
            <span v-else />
          </div>
          <div class="meta">
            <strong>History ({{ s.imageCount }} Images)</strong>
            <span>Last Saved On: {{ formatSaved(s.savedAt) }}</span>
            <div class="acts">
              <button type="button" class="btn" @click="onDelete(s.id)">Delete</button>
              <button type="button" class="btn primary" @click="onLoad(s.id)">Load</button>
            </div>
          </div>
        </li>
      </ul>
      <button type="button" class="skip" @click="store.startNewSession()">Start a new session</button>
    </section>
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 40;
  background: rgba(14, 15, 33, 0.72);
  display: flex;
  align-items: flex-start;
  justify-content: flex-start;
  padding: 64px 0 0 16px;
}
.panel {
  width: min(420px, calc(100vw - 36px));
  max-height: calc(100vh - 96px);
  overflow: auto;
  background: #191b31;
  border: 1px solid #22253f;
  border-radius: 12px;
  padding: 22px 20px 16px;
  box-shadow: 0 18px 50px rgba(0, 0, 0, 0.45);
}
h1 {
  margin: 0 0 8px;
  font-family: Eczar, "Times New Roman", serif;
  font-weight: 600;
  letter-spacing: 0.5px;
  color: #f5f3c2;
  font-size: 26px;
}
.hint {
  margin: 0 0 18px;
  color: #9aa3c7;
  font-size: 13px;
  line-height: 1.5;
}
.list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: grid;
  gap: 16px;
}
.list li {
  display: grid;
  grid-template-columns: 72px 1fr;
  gap: 14px;
  align-items: start;
}
.thumb {
  width: 72px;
  height: 96px;
  border-radius: 8px;
  overflow: hidden;
  background: #12132a;
  border: 1px solid #3d3f68;
}
.thumb img,
.thumb span {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}
.meta {
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}
.meta strong {
  font-size: 15px;
}
.meta span {
  color: #9aa3c7;
  font-size: 13px;
}
.acts {
  display: flex;
  gap: 8px;
  margin-top: 8px;
}
.acts .btn {
  min-width: 72px;
}
.skip {
  margin-top: 16px;
  background: transparent;
  border: 0;
  color: #9aa3c7;
  padding: 0;
}
.skip:hover { color: #f3e6b8; }
.empty { color: #9aa3c7; }
</style>
