<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { fetchRemoteImage } from "@/api/tauri";
import { MAX_PRECISE_REFS, MAX_VIBE_IMAGES, supportsNAIPreciseReference, supportsNAIVibeTransfer, type PreciseReferenceType } from "@/types/nai";
import { useAppStore } from "@/stores/app";
import NaiIcon from "@/components/NaiIcon.vue";

const props = defineProps<{
  onPick: (file: File, kind?: "i2i" | "vibe" | "precise") => void;
}>();

const store = useAppStore();
const vibeMenu = ref(false);
const wantInpaint = ref(false);
const vibeOk = computed(() => supportsNAIVibeTransfer(store.params.model));
const preciseOk = computed(() => supportsNAIPreciseReference(store.params.model));

function switchToV45() {
  store.params.model = store.params.model.includes("curated")
    ? "nai-diffusion-4-5-curated"
    : "nai-diffusion-4-5-full";
}
const PRECISE_TYPES: { value: PreciseReferenceType; label: string }[] = [
  { value: "character&style", label: "Character & Style" },
  { value: "character", label: "Character" },
  { value: "style", label: "Style" },
];

function pick(kind: "i2i" | "vibe" | "precise") {
  const input = document.createElement("input");
  input.type = "file";
  input.accept = "image/png,image/jpeg,image/webp";
  input.onchange = () => {
    const file = input.files?.[0];
    if (file) props.onPick(file, kind);
  };
  input.click();
}

function onCardDragOver(ev: DragEvent) {
  ev.preventDefault();
  ev.stopPropagation();
  (ev.currentTarget as HTMLElement).classList.add("over");
}

function onCardDragLeave(ev: DragEvent) {
  (ev.currentTarget as HTMLElement).classList.remove("over");
}

function onCardDrop(kind: "i2i" | "vibe" | "precise", ev: DragEvent) {
  ev.preventDefault();
  ev.stopPropagation();
  (ev.currentTarget as HTMLElement).classList.remove("over");
  const html = ev.dataTransfer?.getData("text/html") || "";
  const uri = `${ev.dataTransfer?.getData("text/uri-list") || ""}\n${ev.dataTransfer?.getData("text/plain") || ""}`;
  const fromHtml = html.match(/<img[^>]+src=["']([^"']+)/i)?.[1];
  const fromUri = uri.split(/\r?\n/).map((line) => line.trim()).find((line) => /^https?:\/\//i.test(line));
  const url = fromHtml || fromUri;
  if (url?.startsWith("data:image/")) {
    store.useImageAs(kind, url);
    return;
  }
  if (url && /^https?:\/\//i.test(url)) {
    store.status = "正在下载外站图片…";
    void fetchRemoteImage(url)
      .then((preview) => store.useImageAs(kind, preview))
      .catch((e) => {
        store.status = e instanceof Error ? e.message : String(e);
      });
  }
}

function useCurrent(kind: "i2i" | "vibe" | "precise") {
  store.useImageAs(kind, store.currentSourceImage());
}

function setI2iTab(tab: "i2i" | "inpaint") {
  if (tab === "inpaint") {
    if (!store.i2iImage && !store.previewUrl) {
      wantInpaint.value = true;
      pick("i2i");
      return;
    }
    if (!store.i2iImage) store.useImageAs("i2i", store.currentSourceImage());
    store.paintMode = true;
    if (!store.inpaintMask) store.startInpaint();
    return;
  }
  store.paintMode = false;
  store.paintEditorOpen = false;
}

watch(
  () => store.i2iImage,
  (value) => {
    if (value && wantInpaint.value) {
      wantInpaint.value = false;
      store.startInpaint();
    }
  },
);

function onEmptyI2iClick() {
  if (store.currentSourceImage()) {
    store.useImageAs("i2i", store.currentSourceImage());
    if (store.paintMode) store.startInpaint();
    return;
  }
  pick("i2i");
}

function shortId(id: string) {
  const clean = id.replace(/-/g, "");
  return `${clean.slice(0, 6)}-${clean.slice(-6)}`;
}
</script>

<template>
  <section class="block">
    <h2>Reference Images</h2>

    <div class="card i2i-card" data-ref-drop="i2i" @dragover="onCardDragOver" @dragleave="onCardDragLeave" @drop="onCardDrop('i2i', $event)">
      <div v-if="!(store.paintMode && store.i2iImage)" class="i2i-tabs">
        <button type="button" :class="{ on: !store.paintMode }" @click="setI2iTab('i2i')">
          <NaiIcon name="i2i" :size="14" />
          图生图底图 <em>(Image-to-Image)</em>
        </button>
        <button type="button" :class="{ on: store.paintMode }" @click="setI2iTab('inpaint')">
          <NaiIcon name="brush" :size="14" />
          局部重绘
          <span v-if="store.paintMode && store.inpaintMask" class="active">Active</span>
        </button>
      </div>

      <button v-if="!store.i2iImage" class="i2i-empty" type="button" @click="onEmptyI2iClick">
        <NaiIcon name="upload" :size="22" />
        <strong>{{ store.paintMode ? "局部重绘底图" : "图生图底图" }}</strong>
        <span>点击上传底图，或拖入图片</span>
      </button>

      <template v-else-if="store.paintMode">
        <div class="inpaint-card">
          <button class="inpaint-thumb" type="button" title="编辑蒙版" @click="store.startInpaint()">
            <img :src="store.i2iImage" alt="" />
            <i
              v-if="store.inpaintMask"
              class="wash"
              :style="{
                WebkitMaskImage: `url(${store.inpaintMask})`,
                maskImage: `url(${store.inpaintMask})`,
              }"
            />
          </button>
          <div class="inpaint-meta">
            <button class="back" type="button" @click="setI2iTab('i2i')">
              <NaiIcon name="up" :size="14" />
              局部重绘
            </button>
            <div class="inpaint-acts">
              <button type="button" title="编辑蒙版" @click="store.startInpaint()"><NaiIcon name="clip" :size="15" /></button>
              <button type="button" title="更换底图" @click="pick('i2i')"><NaiIcon name="upload" :size="15" /></button>
              <span class="grow" />
              <button type="button" title="删除" @click="store.clearI2i()"><NaiIcon name="trash" :size="15" /></button>
            </div>
          </div>
        </div>
        <label class="slide">
          <span>影响强度</span><b>{{ store.i2iStrength.toFixed(2) }}</b>
          <input v-model.number="store.i2iStrength" type="range" min="0" max="1" step="0.01" />
        </label>
      </template>

      <template v-else>
        <div class="i2i-row">
          <img class="thumb" :src="store.i2iImage" alt="" @click="pick('i2i')" />
          <div class="i2i-meta">
            <strong>图生图底图</strong>
            <span>点击上传底图</span>
          </div>
          <div class="i2i-acts">
            <button type="button" @click="pick('i2i')">更换底图</button>
            <button type="button" @click="store.startInpaint()">
              <NaiIcon name="brush" :size="13" /> 局部重绘
            </button>
            <button class="x" type="button" title="清除底图" @click="store.clearI2i()"><NaiIcon name="close" :size="14" /></button>
          </div>
        </div>
        <label class="slide">
          <span>影响强度</span><b>{{ store.i2iStrength.toFixed(2) }}</b>
          <input v-model.number="store.i2iStrength" type="range" min="0" max="1" step="0.01" />
        </label>
        <label class="slide">
          <span>噪点强度 (Noise)</span><b>{{ store.i2iNoise.toFixed(2) }}</b>
          <input v-model.number="store.i2iNoise" type="range" min="0" max="1" step="0.01" />
        </label>
      </template>
    </div>

    <div class="card" data-ref-drop="vibe" @dragover="onCardDragOver" @dragleave="onCardDragLeave" @drop="onCardDrop('vibe', $event)">
      <div class="head">
        <div class="title">
          <NaiIcon name="vibe" :size="18" />
          <strong>Vibe Transfer</strong>
          <em>({{ store.vibeImages.length }}/{{ MAX_VIBE_IMAGES }})</em>
        </div>
        <div class="acts">
          <div class="menu-wrap">
            <button class="icon-sq tiny" type="button" title="More" @click="vibeMenu = !vibeMenu">
              <NaiIcon name="dots" :size="14" />
            </button>
            <div v-if="vibeMenu" class="menu" @mouseleave="vibeMenu = false">
              <button type="button" :disabled="!store.vibeImages.length" @click="store.vibeImages = []; vibeMenu = false">Clear all</button>
            </div>
          </div>
          <button class="icon-sq tiny" type="button" title="用当前图" :disabled="!vibeOk" @click="useCurrent('vibe')">
            <NaiIcon name="image" :size="14" />
          </button>
          <button
            class="icon-sq tiny"
            type="button"
            title="Add vibe"
            :class="{ off: !vibeOk || store.vibeImages.length >= MAX_VIBE_IMAGES }"
            :disabled="!vibeOk || store.vibeImages.length >= MAX_VIBE_IMAGES"
            @click="pick('vibe')"
          >
            <NaiIcon name="plus" :size="14" />
          </button>
        </div>
      </div>
      <p v-if="!vibeOk" class="banner">
        V5 不支持氛围迁移
        <button type="button" class="link" @click="switchToV45">切换到 V4.5</button>
      </p>

      <label class="check">
        <input v-model="store.normalizeVibe" type="checkbox" />
        Normalize Reference Strength Values
      </label>

      <article v-for="vibe in store.vibeImages" :key="vibe.id" class="ref" :class="{ off: !vibe.enabled }">
        <div class="row">
          <img :src="vibe.previewUrl" alt="" />
          <div class="meta">
            <span class="id">{{ shortId(vibe.id) }}</span>
            <span class="cost"><NaiIcon name="anlas" :size="12" /> 2</span>
          </div>
        </div>
        <label class="slide">
          <span>Information Extracted</span><b>{{ vibe.infoExtracted.toFixed(1) }}</b>
          <input
            :value="vibe.infoExtracted"
            type="range"
            min="0"
            max="1"
            step="0.01"
            @input="store.updateVibeImage(vibe.id, { infoExtracted: Number(($event.target as HTMLInputElement).value) })"
          />
        </label>
        <label class="slide">
          <span>Reference Strength</span><b>{{ vibe.strength.toFixed(1) }}</b>
          <input
            :value="vibe.strength"
            type="range"
            min="0"
            max="1"
            step="0.01"
            @input="store.updateVibeImage(vibe.id, { strength: Number(($event.target as HTMLInputElement).value) })"
          />
        </label>
        <div class="row-acts">
          <button class="ghost" type="button" @click="store.removeVibeImage(vibe.id)"><NaiIcon name="trash" :size="14" /></button>
          <button class="ghost" type="button" @click="store.removeVibeImage(vibe.id)"><NaiIcon name="x" :size="14" /></button>
        </div>
        <p class="note">Encoding required. This will cost 2 Anlas on the next generation.</p>
      </article>
    </div>

    <div class="card" data-ref-drop="precise" @dragover="onCardDragOver" @dragleave="onCardDragLeave" @drop="onCardDrop('precise', $event)">
      <div class="head">
        <div class="title">
          <NaiIcon name="precise" :size="18" />
          <strong>Precise Reference</strong>
        </div>
        <div class="acts">
          <button
            class="icon-sq tiny"
            type="button"
            title="Clear"
            :disabled="!store.preciseReferences.length"
            @click="store.preciseReferences = []"
          >
            <NaiIcon name="crop" :size="14" />
          </button>
          <button class="icon-sq tiny" type="button" title="用当前图" :disabled="!preciseOk" @click="useCurrent('precise')">
            <NaiIcon name="image" :size="14" />
          </button>
          <button
            class="icon-sq tiny"
            type="button"
            title="Attach"
            :class="{ off: !preciseOk || store.preciseReferences.length >= MAX_PRECISE_REFS }"
            :disabled="!preciseOk || store.preciseReferences.length >= MAX_PRECISE_REFS"
            @click="pick('precise')"
          >
            <NaiIcon name="clip" :size="14" />
          </button>
        </div>
      </div>
      <p v-if="!preciseOk" class="banner">
        精准参考仅 V4.5 可用
        <button type="button" class="link" @click="switchToV45">切换到 V4.5</button>
      </p>

      <article v-for="ref in store.preciseReferences" :key="ref.id" class="ref" :class="{ off: !ref.enabled }">
        <div class="row">
          <img :src="ref.previewUrl" alt="" />
          <select
            class="type"
            :value="ref.type"
            @change="store.updatePreciseReference(ref.id, { type: ($event.target as HTMLSelectElement).value as PreciseReferenceType })"
          >
            <option v-for="opt in PRECISE_TYPES" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
          </select>
        </div>
        <label class="slide">
          <span>Strength</span><b>{{ ref.strength.toFixed(0) }}</b>
          <input
            :value="ref.strength"
            type="range"
            min="0"
            max="1"
            step="0.01"
            @input="store.updatePreciseReference(ref.id, { strength: Number(($event.target as HTMLInputElement).value) })"
          />
        </label>
        <label class="slide">
          <span>Fidelity</span><b>{{ ref.fidelity.toFixed(0) }}</b>
          <input
            :value="ref.fidelity"
            type="range"
            min="0"
            max="1"
            step="0.01"
            @input="store.updatePreciseReference(ref.id, { fidelity: Number(($event.target as HTMLInputElement).value) })"
          />
        </label>
        <div class="row-acts">
          <button class="ghost" type="button" @click="store.removePreciseReference(ref.id)"><NaiIcon name="trash" :size="14" /></button>
          <button class="ghost" :class="{ on: ref.enabled }" type="button" @click="store.updatePreciseReference(ref.id, { enabled: !ref.enabled })">
            <NaiIcon name="check" :size="14" />
          </button>
        </div>
      </article>
    </div>
  </section>
</template>

<style scoped>
.block h2 { font-size: 20px; margin: 0 0 10px; }
.card { margin-bottom: 14px; border-radius: 10px; transition: box-shadow 0.15s ease, outline-color 0.15s ease; outline: 2px solid transparent; }
.card.over { outline-color: var(--heading); box-shadow: 0 0 0 1px var(--heading); }
.i2i-tabs {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-bottom: 12px;
}
.i2i-tabs button {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  background: var(--bg2);
  border: 1px solid var(--bg3);
  color: rgba(255,255,255,0.78);
  border-radius: 999px;
  padding: 6px 10px;
  font-size: 12px;
}
.i2i-tabs button em { font-style: normal; color: var(--muted); }
.i2i-tabs button.on { color: var(--heading); border-color: var(--heading); }
.i2i-tabs .active {
  background: #2d8a4a;
  color: #fff;
  border-radius: 999px;
  padding: 1px 6px;
  font-size: 10px;
}
.i2i-empty {
  width: 100%;
  display: grid;
  justify-items: center;
  gap: 4px;
  padding: 22px 12px;
  border: 1px dashed var(--bg3);
  border-radius: 10px;
  background: var(--bg0);
  color: var(--muted);
}
.i2i-empty strong { color: #fff; font-size: 13px; }
.i2i-empty span { font-size: 12px; }
.inpaint-card {
  display: grid;
  grid-template-columns: 72px 1fr;
  gap: 10px 12px;
  align-items: start;
  margin-bottom: 8px;
}
.inpaint-thumb {
  position: relative;
  width: 72px;
  height: 72px;
  overflow: hidden;
  padding: 0;
  border: 0;
  border-radius: 8px;
  background: var(--bg0);
}
.inpaint-thumb img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}
.inpaint-thumb .wash,
.preview-wash {
  position: absolute;
  inset: 0;
  background: rgba(122, 108, 230, 0.46);
  pointer-events: none;
  -webkit-mask-size: 100% 100%;
  mask-size: 100% 100%;
  -webkit-mask-repeat: no-repeat;
  mask-repeat: no-repeat;
}
.inpaint-meta { min-width: 0; display: grid; gap: 10px; }
.back {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: none;
  border: 0;
  color: #fff;
  padding: 0;
  font-size: 14px;
}
.back :deep(.nai-icon) { transform: rotate(-90deg); }
.inpaint-acts {
  display: flex;
  align-items: center;
  gap: 8px;
}
.inpaint-acts button {
  background: none;
  border: 0;
  color: rgba(255,255,255,0.72);
  padding: 2px;
}
.inpaint-acts .grow { flex: 1; }
.i2i-row {
  display: grid;
  grid-template-columns: 52px 1fr auto;
  align-items: center;
  gap: 10px;
  margin-bottom: 8px;
}
.thumb {
  width: 52px;
  height: 52px;
  object-fit: cover;
  border-radius: 8px;
  background: var(--bg0);
  cursor: pointer;
}
.i2i-meta { min-width: 0; }
.i2i-meta strong { display: block; font-size: 13px; }
.i2i-meta span { color: var(--muted); font-size: 12px; }
.i2i-acts { display: flex; align-items: center; gap: 6px; }
.i2i-acts button {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: transparent;
  border: 0;
  color: rgba(255,255,255,0.78);
  padding: 4px;
  font-size: 12px;
  white-space: nowrap;
}
.i2i-acts button.on { color: var(--heading); }
.i2i-acts .x { color: rgba(255,255,255,0.55); }
.head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  min-width: 0;
}
.title {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  color: var(--heading);
}
.title strong {
  overflow: hidden;
  color: var(--heading);
  font-size: 16px;
  font-weight: 600;
  line-height: 1.2;
  white-space: nowrap;
  text-overflow: ellipsis;
}
.title em {
  flex: none;
  color: var(--muted);
  font-style: normal;
  font-weight: 600;
  white-space: nowrap;
}
.acts { display: flex; flex: none; gap: 6px; align-items: center; }
.banner {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px;
  margin: 8px 0 0;
  color: #ffb4b4;
  font-size: 12px;
  line-height: 1.4;
}
.banner .link {
  background: none;
  border: 0;
  color: var(--heading);
  padding: 0;
  font-size: 12px;
  font-weight: 600;
  text-decoration: underline;
  text-underline-offset: 2px;
  white-space: nowrap;
}
.icon-sq {
  width: 28px; height: 28px; display: grid; place-items: center;
  background: var(--bg2); border: 1px solid var(--bg3); border-radius: 6px; color: #fff;
}
.icon-sq.on { color: var(--heading); border-color: var(--heading); }
.icon-sq.off,
.icon-sq:disabled { opacity: 0.4; }
.preview { width: 100%; margin-top: 8px; border-radius: 6px; max-height: 160px; object-fit: contain; background: var(--bg0); }
.check {
  display: flex; align-items: center; gap: 8px;
  margin: 10px 0 8px; font-size: 13px; color: #fff;
}
.check input { accent-color: var(--heading); width: 16px; height: 16px; }
.ref { margin-top: 10px; padding-top: 8px; }
.ref.off { opacity: 0.45; }
.row { display: flex; align-items: center; gap: 10px; }
.row img { width: 56px; height: 72px; object-fit: cover; border-radius: 6px; background: var(--bg0); }
.meta { display: flex; align-items: center; justify-content: space-between; flex: 1; gap: 8px; }
.id { font-size: 13px; color: #fff; }
.cost {
  display: inline-flex; align-items: center; gap: 4px;
  background: var(--bg0); border-radius: 6px; padding: 4px 8px; font-size: 12px;
}
.type {
  flex: 1;
  background: var(--bg0);
  border: 1px solid var(--bg3);
  border-radius: 6px;
  padding: 8px 10px;
}
.slide { display: grid; grid-template-columns: 1fr auto; gap: 4px 8px; margin: 8px 0; font-size: 13px; color: rgba(255,255,255,0.8); }
.slide input[type="range"] { grid-column: 1 / -1; width: 100%; }
.slide b {
  background: var(--bg0);
  border-radius: 6px;
  padding: 2px 8px;
  min-width: 28px;
  text-align: center;
  font-variant-numeric: tabular-nums;
}
.row-acts { display: flex; gap: 6px; }
.ghost {
  background: transparent; border: 0; color: rgba(255,255,255,0.7); padding: 2px; display: grid; place-items: center;
}
.ghost.on { color: var(--heading); }
.note { margin: 6px 0 0; color: var(--muted); font-size: 12px; font-weight: 400; }
.menu-wrap { position: relative; }
.menu {
  position: absolute; right: 0; top: 32px; z-index: 4;
  background: var(--bg2); border: 1px solid var(--bg3); border-radius: 6px; min-width: 120px; padding: 4px;
}
.menu button {
  width: 100%; text-align: left; background: none; border: 0; color: #fff; padding: 6px 8px; border-radius: 4px;
}
.menu button:hover { background: var(--bg3); }
.menu button:disabled { opacity: 0.4; }
</style>
