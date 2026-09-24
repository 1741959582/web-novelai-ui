<script setup lang="ts">
import { computed, ref } from "vue";
import { useApngStore } from "@/stores/apng";
import { useAppStore } from "@/stores/app";
import { DIRECTOR_TOOLS } from "@/types/nai";
import { quoteAnlas, quoteUpscaleAnlas } from "@/utils/anlas";
import NaiIcon from "@/components/NaiIcon.vue";

const store = useAppStore();
const apng = useApngStore();
const directorOpen = ref(false);
const panel = ref<"" | "vary" | "upscale" | "enhance">("");
const varyCount = ref(1);
const varyStrength = ref(0.6);
const upscaleScale = ref<2 | 4>(4);
const magnitude = ref(2);
const showEnhanceDetail = ref(false);
const enhanceStrength = ref(0.3);
const enhanceNoise = ref(0);
const enhanceScale = ref<1 | 2>(1);

const MAGNITUDES = [
  { strength: 0.2, noise: 0 },
  { strength: 0.3, noise: 0 },
  { strength: 0.4, noise: 0.05 },
  { strength: 0.5, noise: 0.1 },
  { strength: 0.65, noise: 0.15 },
];

const source = computed(() => store.previewUrl || store.i2iImage);
const opus = computed(
  () => Boolean(store.account.hasActiveSubscription && (store.account.tierLevel ?? 0) >= 3),
);
const upscaleCost = computed(() =>
  quoteUpscaleAnlas(store.params.width, store.params.height, upscaleScale.value, opus.value),
);
const varyCost = computed(() =>
  quoteAnlas({
    params: store.params,
    account: store.account,
    batchCount: varyCount.value,
    action: "img2img",
    strength: varyStrength.value,
  }),
);
const enhancePreset = computed(() => MAGNITUDES[magnitude.value - 1] || MAGNITUDES[1]);
const enhanceCost = computed(() => {
  const scale = enhanceScale.value;
  return quoteAnlas({
    params: {
      ...store.params,
      width: Math.min(store.params.width * scale, 4096),
      height: Math.min(store.params.height * scale, 4096),
    },
    account: store.account,
    action: "img2img",
    strength: showEnhanceDetail.value ? enhanceStrength.value : enhancePreset.value.strength,
  });
});

function toggle(name: "vary" | "upscale" | "enhance") {
  panel.value = panel.value === name ? "" : name;
  directorOpen.value = false;
}
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
    <div class="tool">
      <button type="button" title="Variations 变体" :class="{ on: panel === 'vary' }" :disabled="store.busy" @click="toggle('vary')">
        <NaiIcon name="grid" :size="15" />
        <span class="cost"><NaiIcon name="anlas" :size="11" />{{ varyCost }}</span>
      </button>
      <div v-if="panel === 'vary'" class="pop">
        <label>张数 <b>{{ varyCount }}</b>
          <input v-model.number="varyCount" type="range" min="1" max="4" step="1" />
        </label>
        <label>变化强度 <b>{{ varyStrength.toFixed(2) }}</b>
          <input v-model.number="varyStrength" type="range" min="0.2" max="0.85" step="0.05" />
        </label>
        <button type="button" class="go" :disabled="store.busy" @click="panel = ''; store.varyCurrent(varyCount, varyStrength)">生成变体</button>
      </div>
    </div>
    <div class="tool">
      <button type="button" title="Upscale" :class="{ on: panel === 'upscale' }" :disabled="store.busy" @click="toggle('upscale')">
        <NaiIcon name="upscale" :size="15" />
        <span class="cost"><NaiIcon name="anlas" :size="11" />{{ upscaleCost }}</span>
      </button>
      <div v-if="panel === 'upscale'" class="pop">
        <div class="seg">
          <button type="button" :class="{ on: upscaleScale === 2 }" @click="upscaleScale = 2">2×</button>
          <button type="button" :class="{ on: upscaleScale === 4 }" @click="upscaleScale = 4">4×</button>
        </div>
        <p>只放大，不改画面内容。费用 {{ upscaleCost }} Anlas。</p>
        <button type="button" class="go" :disabled="store.busy" @click="panel = ''; store.upscaleCurrent(upscaleScale)">超分</button>
      </div>
    </div>
    <div class="tool">
      <button type="button" title="Enhance 增强" :class="{ on: panel === 'enhance' }" :disabled="store.busy" @click="toggle('enhance')">
        <NaiIcon name="sparkle" :size="15" />
        <span class="cost"><NaiIcon name="anlas" :size="11" />{{ enhanceCost }}</span>
      </button>
      <div v-if="panel === 'enhance'" class="pop">
        <label>Magnitude <b>{{ magnitude }}</b>
          <input v-model.number="magnitude" type="range" min="1" max="5" step="1" :disabled="showEnhanceDetail" />
        </label>
        <label class="check"><input v-model="showEnhanceDetail" type="checkbox" />单独调节 Strength / Noise</label>
        <template v-if="showEnhanceDetail">
          <label>Strength <b>{{ enhanceStrength.toFixed(2) }}</b>
            <input v-model.number="enhanceStrength" type="range" min="0.05" max="0.85" step="0.05" />
          </label>
          <label>Noise <b>{{ enhanceNoise.toFixed(2) }}</b>
            <input v-model.number="enhanceNoise" type="range" min="0" max="0.4" step="0.05" />
          </label>
        </template>
        <div class="seg">
          <button type="button" :class="{ on: enhanceScale === 1 }" @click="enhanceScale = 1">原尺寸</button>
          <button type="button" :class="{ on: enhanceScale === 2 }" @click="enhanceScale = 2">增强时 2×</button>
        </div>
        <button
          type="button"
          class="go"
          :disabled="store.busy"
          @click="panel = ''; store.enhanceCurrent({
            strength: showEnhanceDetail ? enhanceStrength : enhancePreset.strength,
            noise: showEnhanceDetail ? enhanceNoise : enhancePreset.noise,
            scale: enhanceScale,
          })"
        >增强</button>
      </div>
    </div>
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
.tool, .more { position: relative; }
.pop {
  position: absolute;
  top: calc(100% + 8px);
  left: 0;
  width: 220px;
  padding: 10px;
  border-radius: 12px;
  background: #151728;
  border: 1px solid #2a2d48;
  box-shadow: 0 12px 28px rgba(0,0,0,0.4);
  z-index: 6;
  display: grid;
  gap: 8px;
}
.pop label { display: grid; gap: 4px; color: rgba(255,255,255,0.78); font-size: 12px; }
.pop label b { justify-self: end; }
.pop input[type="range"] { width: 100%; }
.pop .check { display: flex; align-items: center; gap: 6px; }
.seg { display: flex; gap: 4px; }
.seg button { flex: 1; height: 28px; background: #10121f; }
.seg button.on { background: #2a2d22; color: #f5f3c2; }
.pop p { margin: 0; color: rgba(255,255,255,0.55); font-size: 12px; }
.go {
  height: 32px;
  background: #f5f3c2 !important;
  color: #0e0f21 !important;
  font-weight: 700;
}
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
