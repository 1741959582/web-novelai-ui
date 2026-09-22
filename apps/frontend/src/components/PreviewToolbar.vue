<script setup lang="ts">
import { computed, ref } from "vue";
import { useApngStore } from "@/stores/apng";
import { useAppStore } from "@/stores/app";
import { DIRECTOR_TOOLS } from "@/types/nai";
import { quoteUpscaleAnlas } from "@/utils/anlas";
import NaiIcon from "@/components/NaiIcon.vue";

const store = useAppStore();
const apng = useApngStore();
const directorOpen = ref(false);

const source = computed(() => store.previewUrl || store.i2iImage);
const opus = computed(
  () => Boolean(store.account.hasActiveSubscription && (store.account.tierLevel ?? 0) >= 3),
);
const upscaleCost = computed(() =>
  quoteUpscaleAnlas(store.params.width, store.params.height, 4, opus.value),
);
const pinned = computed(() => Boolean(store.pinnedUrl) && store.pinnedUrl === source.value);

function run(fn: () => unknown) {
  directorOpen.value = false;
  return fn();
}

async function addToApng() {
  const src = source.value;
  if (!src) return;
  const ok = await apng.addReal(src, store.currentItem?.id || "生成图");
  store.status = ok
    ? `已清掉元数据并加入伪装队列（${apng.items.length}）`
    : "伪装队列已满";
}
</script>

<template>
  <div v-if="source && !store.busy" class="bar">
    <button type="button" title="Enhance 增强" :disabled="store.busy" @click="store.enhanceCurrent()">
      <NaiIcon name="sparkle" :size="15" />
    </button>
    <button type="button" title="Upscale 4x" :disabled="store.busy" @click="store.upscaleCurrent()">
      <NaiIcon name="grid" :size="15" />
      <span class="cost"><NaiIcon name="anlas" :size="11" />{{ upscaleCost }}</span>
    </button>
    <button type="button" title="复制图片" @click="store.copyCurrentImage()">
      <NaiIcon name="copy" :size="15" />
      <b>1</b>
    </button>
    <button type="button" title="固定对比" :class="{ on: pinned }" @click="store.togglePin()">
      <NaiIcon name="thumbtack" :size="15" />
    </button>
    <button type="button" title="设为图生图底图" @click="store.useCurrentAsI2i()">
      <NaiIcon name="clip" :size="15" />
    </button>
    <button type="button" title="下载" @click="store.downloadCurrentImage()">
      <NaiIcon name="download" :size="15" />
    </button>
    <button type="button" title="添加到 APNG" @click="addToApng()">
      <NaiIcon name="layers" :size="15" />
    </button>
    <div class="more">
      <button type="button" title="Director Tools" :class="{ on: directorOpen }" @click="directorOpen = !directorOpen">
        <NaiIcon name="arrow" :size="15" />
      </button>
      <div v-if="directorOpen" class="menu">
        <button
          v-for="tool in DIRECTOR_TOOLS"
          :key="tool.value"
          type="button"
          :disabled="store.busy"
          @click="run(() => store.runDirector(tool.value))"
        >
          <span>{{ tool.label }}</span>
          <em v-if="tool.cost">{{ tool.cost }} Anlas</em>
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.bar {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  padding: 4px 6px;
  border-radius: 999px;
  background: rgba(16, 18, 32, 0.92);
  border: 1px solid rgba(255,255,255,0.08);
  box-shadow: 0 10px 28px rgba(0,0,0,0.35);
}
button {
  height: 32px;
  min-width: 32px;
  padding: 0 8px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 5px;
  border: 0;
  background: transparent;
  color: rgba(255,255,255,0.82);
  border-radius: 999px;
}
button:hover { background: rgba(255,255,255,0.08); color: #fff; }
button.on, button:disabled { color: var(--heading); }
.cost, b { font-size: 12px; font-weight: 700; }
.cost { display: inline-flex; align-items: center; gap: 2px; color: #f3e27a; }
.more { position: relative; }
.menu {
  position: absolute;
  top: calc(100% + 8px);
  right: 0;
  min-width: 168px;
  padding: 6px;
  border-radius: 12px;
  background: #151728;
  border: 1px solid #2a2d48;
  box-shadow: 0 12px 28px rgba(0,0,0,0.4);
  z-index: 5;
}
.menu button {
  width: 100%;
  height: 34px;
  justify-content: space-between;
  border-radius: 8px;
  color: #fff;
}
.menu em { font-style: normal; color: #f3e27a; font-size: 11px; }
</style>
