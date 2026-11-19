<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { fetchRemoteImage } from "@/api/tauri";
import { MAX_PRECISE_REFS, MAX_VIBE_IMAGES, supportsNAIPreciseReference, supportsNAIVibeTransfer, type PreciseReferenceType } from "@/types/nai";
import { useAppStore } from "@/stores/app";
import { useMaterialLibrary } from "@/stores/materialLibrary";
import NaiIcon from "@/components/NaiIcon.vue";

const props = defineProps<{
  onPick: (file: File, kind?: "i2i" | "vibe" | "precise") => void;
}>();

const store = useAppStore();
const library = useMaterialLibrary();
const router = useRouter();
const wantInpaint = ref(false);
const i2iOpen = ref(true);
const vibeOpen = ref(true);
const preciseOpen = ref(true);
const sketchOpen = ref(false);
const sketchCanvas = ref<HTMLCanvasElement | null>(null);
const vibeOk = computed(() => supportsNAIVibeTransfer(store.params.model));
const preciseOk = computed(() => supportsNAIPreciseReference(store.params.model));
const vibeOn = computed(() => store.vibeImages.some((item) => item.enabled));
const preciseOn = computed(() => store.preciseReferences.some((item) => item.enabled));
const PRECISE_TYPES: { value: PreciseReferenceType; label: string }[] = [
  { value: "character&style", label: "角色和风格" },
  { value: "character", label: "角色" },
  { value: "style", label: "风格" },
];

function switchToV45() {
  store.params.model = store.params.model.includes("curated")
    ? "nai-diffusion-4-5-curated"
    : "nai-diffusion-4-5-full";
}

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
      .catch((error) => {
        store.status = error instanceof Error ? error.message : String(error);
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

let drawing = false;
function sketchPoint(ev: PointerEvent) {
  const canvas = sketchCanvas.value;
  if (!canvas) return null;
  const rect = canvas.getBoundingClientRect();
  return {
    x: ((ev.clientX - rect.left) / rect.width) * canvas.width,
    y: ((ev.clientY - rect.top) / rect.height) * canvas.height,
  };
}

function drawSketch(ev: PointerEvent) {
  if (!drawing) return;
  const canvas = sketchCanvas.value;
  const ctx = canvas?.getContext("2d");
  const point = sketchPoint(ev);
  if (!ctx || !point) return;
  ctx.lineTo(point.x, point.y);
  ctx.stroke();
}

function startSketch(ev: PointerEvent) {
  const canvas = sketchCanvas.value;
  const ctx = canvas?.getContext("2d");
  const point = sketchPoint(ev);
  if (!ctx || !point) return;
  drawing = true;
  canvas?.setPointerCapture(ev.pointerId);
  ctx.beginPath();
  ctx.moveTo(point.x, point.y);
}

function endSketch() {
  drawing = false;
}

function prepareSketch() {
  const canvas = sketchCanvas.value;
  const ctx = canvas?.getContext("2d");
  if (!canvas || !ctx) return;
  ctx.fillStyle = "#f4f1e8";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.strokeStyle = "#1a1c2e";
  ctx.lineWidth = 4;
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
}

function openSketch() {
  sketchOpen.value = true;
  void nextTick().then(prepareSketch);
}

function useSketch() {
  const canvas = sketchCanvas.value;
  if (!canvas) return;
  store.useImageAs("i2i", canvas.toDataURL("image/png"));
  sketchOpen.value = false;
}

function saveVibes() {
  for (const vibe of store.vibeImages) {
    library.addVibe({
      name: "Vibe",
      previewUrl: vibe.previewUrl,
      infoExtracted: vibe.infoExtracted,
      strength: vibe.strength,
    });
  }
  store.status = library.error || "已保存到 Vibe 库";
}

function savePrecise() {
  for (const item of store.preciseReferences) {
    library.addPrecise({
      name: "精准参考",
      previewUrl: item.previewUrl,
      type: item.type,
      strength: item.strength,
      fidelity: item.fidelity,
    });
  }
  store.status = library.error || "已保存到精准参考库";
}
</script>

<template>
  <section class="refs">
    <h2>参考图</h2>

    <div class="group" data-ref-drop="i2i" @dragover="onCardDragOver" @dragleave="onCardDragLeave" @drop="onCardDrop('i2i', $event)">
      <div class="row" @click="i2iOpen = !i2iOpen">
        <NaiIcon name="i2i" :size="18" />
        <b>图生图</b>
        <span>{{ store.i2iImage ? (store.paintMode ? "局部重绘" : `强度 ${store.i2iStrength.toFixed(2)}`) : "基于原图生成变体" }}</span>
        <button v-if="!store.i2iImage" class="ghost" type="button" @click.stop="pick('i2i')">
          <NaiIcon name="upload" :size="14" /> 上传
        </button>
        <NaiIcon class="chev" :class="{ open: i2iOpen }" name="down" :size="14" />
      </div>
      <div v-if="i2iOpen" class="body">
        <div v-if="!store.i2iImage" class="drop" @click="pick('i2i')">
          <span class="drop-hint"><NaiIcon name="upload" :size="15" /> 拖一张图进来，或者</span>
          <span class="drop-acts" @click.stop>
            <button type="button" @click="pick('i2i')"><NaiIcon name="upload" :size="13" /> 上传</button>
            <button type="button" @click="router.push({ path: '/precise-library', query: { for: 'i2i' } })">
              <NaiIcon name="folder" :size="13" /> 从精准参考库
            </button>
            <button type="button" @click="openSketch"><NaiIcon name="brush" :size="13" /> 画草图</button>
            <button type="button" @click="setI2iTab('inpaint')"><NaiIcon name="brush" :size="13" /> 局部重绘</button>
          </span>
        </div>
        <template v-else>
          <div class="shot">
            <img :src="store.i2iImage" alt="" />
            <div>
              <div class="shot-acts">
                <button type="button" @click="pick('i2i')">更换</button>
                <button type="button" @click="setI2iTab('inpaint')">局部重绘</button>
                <button type="button" class="danger" @click="store.clearI2i()">删除</button>
              </div>
              <p v-if="store.paintMode && store.inpaintMask" class="note">蒙版已就绪，生成时只重绘涂过的地方。</p>
            </div>
          </div>
          <label class="slide">
            <span>影响强度</span><b>{{ store.i2iStrength.toFixed(2) }}</b>
            <input v-model.number="store.i2iStrength" type="range" min="0" max="1" step="0.01" />
          </label>
          <label class="slide">
            <span>噪点</span><b>{{ store.i2iNoise.toFixed(2) }}</b>
            <input v-model.number="store.i2iNoise" type="range" min="0" max="1" step="0.01" />
          </label>
        </template>
      </div>
    </div>

    <div class="group" data-ref-drop="vibe" @dragover="onCardDragOver" @dragleave="onCardDragLeave" @drop="onCardDrop('vibe', $event)">
      <div class="row" @click="vibeOpen = !vibeOpen">
        <NaiIcon name="vibe" :size="18" />
        <b>风格迁移</b>
        <em v-if="store.vibeImages.length">{{ store.vibeImages.length }}/{{ MAX_VIBE_IMAGES }}</em>
        <span :class="{ warn: preciseOn && vibeOn }">{{ preciseOn && vibeOn ? "和精准参考同时开启时，这次不发送风格" : "改变图像，保留视觉风格" }}</span>
        <button
          v-if="!store.vibeImages.length"
          class="icon"
          type="button"
          title="添加"
          :disabled="!vibeOk"
          @click.stop="pick('vibe')"
        >
          <NaiIcon name="plus" :size="14" />
        </button>
        <NaiIcon class="chev" :class="{ open: vibeOpen }" name="down" :size="14" />
      </div>
      <div v-if="vibeOpen" class="body">
        <p v-if="!vibeOk" class="banner">
          V5 不支持氛围迁移
          <button type="button" @click="switchToV45">切换到 V4.5</button>
        </p>
        <p v-else-if="preciseOn && vibeOn" class="banner">和精准参考同时开启时，这次不发送风格</p>
        <div v-if="!store.vibeImages.length" class="drop">
          <span class="drop-hint"><NaiIcon name="upload" :size="15" /> 拖图片或 .naiv4vibe 进来，或者</span>
          <span class="drop-acts">
            <button type="button" :disabled="!vibeOk" @click="pick('vibe')"><NaiIcon name="upload" :size="13" /> 从文件添加</button>
            <button type="button" @click="router.push('/vibe-library')"><NaiIcon name="folder" :size="13" /> 从 Vibe 库添加</button>
          </span>
        </div>
        <template v-else>
          <article v-for="vibe in store.vibeImages" :key="vibe.id" class="item" :class="{ off: !vibe.enabled }">
            <img :src="vibe.previewUrl" alt="" />
            <div class="item-main">
              <label class="slide compact">
                <span>参考强度</span><b>{{ vibe.strength.toFixed(2) }}</b>
                <input
                  :value="vibe.strength"
                  type="range"
                  min="0"
                  max="1"
                  step="0.01"
                  @input="store.updateVibeImage(vibe.id, { strength: Number(($event.target as HTMLInputElement).value) })"
                />
              </label>
              <label class="slide compact">
                <span>提取信息</span><b>{{ vibe.infoExtracted.toFixed(2) }}</b>
                <input
                  :value="vibe.infoExtracted"
                  type="range"
                  min="0"
                  max="1"
                  step="0.01"
                  @input="store.updateVibeImage(vibe.id, { infoExtracted: Number(($event.target as HTMLInputElement).value) })"
                />
              </label>
            </div>
            <div class="item-side">
              <button type="button" :class="{ on: vibe.enabled }" title="启用" @click="store.updateVibeImage(vibe.id, { enabled: !vibe.enabled })">
                <NaiIcon name="check" :size="14" />
              </button>
              <button type="button" title="删除" @click="store.removeVibeImage(vibe.id)"><NaiIcon name="close" :size="14" /></button>
            </div>
          </article>
          <div class="foot">
            <button type="button" :disabled="!vibeOk || store.vibeImages.length >= MAX_VIBE_IMAGES" @click="pick('vibe')">
              <NaiIcon name="plus" :size="13" /> 添加
            </button>
            <button class="text" type="button" @click="saveVibes">保存到库</button>
            <button class="text" type="button" :disabled="!vibeOk" @click="useCurrent('vibe')">用当前图</button>
            <label class="check">
              归一化
              <input v-model="store.normalizeVibe" type="checkbox" />
            </label>
            <button class="icon" type="button" title="清空全部" @click="store.vibeImages = []"><NaiIcon name="trash" :size="14" /></button>
          </div>
        </template>
      </div>
    </div>

    <div class="group" data-ref-drop="precise" @dragover="onCardDragOver" @dragleave="onCardDragLeave" @drop="onCardDrop('precise', $event)">
      <div class="row" @click="preciseOpen = !preciseOpen">
        <NaiIcon name="precise" :size="18" />
        <b>精准参考</b>
        <em v-if="store.preciseReferences.length">{{ store.preciseReferences.length }}</em>
        <span>将生成绑定到角色或风格参考</span>
        <button class="icon" type="button" title="保存到精准参考库" :disabled="!store.preciseReferences.length" @click.stop="savePrecise">
          <NaiIcon name="folder" :size="14" />
        </button>
        <button class="icon" type="button" title="加入" :disabled="!preciseOk || store.preciseReferences.length >= MAX_PRECISE_REFS" @click.stop="pick('precise')">
          <NaiIcon name="plus" :size="14" />
        </button>
        <NaiIcon class="chev" :class="{ open: preciseOpen }" name="down" :size="14" />
      </div>
      <div v-if="preciseOpen" class="body">
        <p v-if="!preciseOk" class="banner">
          精准参考仅 V4.5 可用
          <button type="button" @click="switchToV45">切换到 V4.5</button>
        </p>
        <div v-if="!store.preciseReferences.length" class="drop">
          <span class="drop-hint"><NaiIcon name="upload" :size="15" /> 拖角色或风格参考图进来，或者</span>
          <span class="drop-acts">
            <button type="button" :disabled="!preciseOk" @click="pick('precise')"><NaiIcon name="upload" :size="13" /> 从文件添加</button>
            <button type="button" @click="router.push('/precise-library')"><NaiIcon name="folder" :size="13" /> 从精准参考库添加</button>
          </span>
        </div>
        <template v-else>
          <article v-for="item in store.preciseReferences" :key="item.id" class="item" :class="{ off: !item.enabled }">
            <img class="tall" :src="item.previewUrl" alt="" />
            <div class="item-main">
              <select
                :value="item.type"
                @change="store.updatePreciseReference(item.id, { type: ($event.target as HTMLSelectElement).value as PreciseReferenceType })"
              >
                <option v-for="opt in PRECISE_TYPES" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
              </select>
              <label class="slide compact">
                <span>强度</span><b>{{ item.strength.toFixed(2) }}</b>
                <input
                  :value="item.strength"
                  type="range"
                  min="0"
                  max="1"
                  step="0.01"
                  @input="store.updatePreciseReference(item.id, { strength: Number(($event.target as HTMLInputElement).value) })"
                />
              </label>
              <label class="slide compact">
                <span>保真</span><b>{{ item.fidelity.toFixed(2) }}</b>
                <input
                  :value="item.fidelity"
                  type="range"
                  min="0"
                  max="1"
                  step="0.01"
                  @input="store.updatePreciseReference(item.id, { fidelity: Number(($event.target as HTMLInputElement).value) })"
                />
              </label>
            </div>
            <div class="item-side">
              <button type="button" :class="{ on: item.enabled }" title="启用" @click="store.updatePreciseReference(item.id, { enabled: !item.enabled })">
                <NaiIcon name="check" :size="14" />
              </button>
              <button type="button" title="删除" @click="store.removePreciseReference(item.id)"><NaiIcon name="close" :size="14" /></button>
            </div>
          </article>
          <div class="foot">
            <button class="text" type="button" :disabled="!preciseOk" @click="useCurrent('precise')">用当前图</button>
            <span class="grow" />
            <button class="icon" type="button" title="清空全部" @click="store.preciseReferences = []"><NaiIcon name="trash" :size="14" /></button>
          </div>
        </template>
      </div>
    </div>

    <div v-if="sketchOpen" class="mask" @click.self="sketchOpen = false">
      <div class="sketch">
        <header>
          <strong>画草图</strong>
          <button type="button" @click="sketchOpen = false"><NaiIcon name="close" :size="14" /></button>
        </header>
        <canvas
          ref="sketchCanvas"
          width="768"
          height="512"
          @pointerdown="startSketch"
          @pointermove="drawSketch"
          @pointerup="endSketch"
          @pointercancel="endSketch"
        />
        <div class="sketch-acts">
          <button type="button" @click="prepareSketch">清空</button>
          <button class="go" type="button" @click="useSketch">用作图生图</button>
        </div>
      </div>
    </div>
  </section>
</template>

<style scoped>
.refs {
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 12px;
  padding: 12px 12px 0;
  background: rgba(255, 255, 255, 0.02);
}
.refs h2 { margin: 0 0 6px; font-size: 14px; font-family: inherit; letter-spacing: 0; font-weight: 650; color: #fff; }
.group { border-top: 1px solid rgba(255, 255, 255, 0.08); }
.group.over { background: rgba(245, 243, 194, 0.06); }
.row {
  height: 46px;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 2px 0 0;
  color: rgba(255, 255, 255, 0.78);
  cursor: pointer;
}
.row b { font-size: 13px; font-weight: 500; color: #fff; flex: none; }
.row em {
  flex: none;
  height: 18px;
  padding: 0 7px;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.08);
  color: var(--muted);
  font-size: 11px;
  font-style: normal;
  line-height: 18px;
}
.row span { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--muted); font-size: 12px; }
.row span.warn { color: #e39b4a; }
.ghost {
  border: 0;
  background: transparent;
  color: #7eb6ff;
  height: 28px;
  padding: 0 4px;
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 13px;
  font-weight: 650;
}
.icon, .drop-acts button, .shot-acts button, .foot button, .item-side button, .sketch-acts button, .sketch header button {
  border: 1px solid rgba(255, 255, 255, 0.08);
  background: rgba(255, 255, 255, 0.06);
  color: rgba(255, 255, 255, 0.86);
  border-radius: 6px;
  height: 28px;
  padding: 0 10px;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 12px;
  font-weight: 500;
}
.icon, .item-side button, .sketch header button { width: 28px; padding: 0; justify-content: center; color: rgba(255, 255, 255, 0.7); }
.foot .text { border-color: transparent; background: transparent; color: rgba(255, 255, 255, 0.72); }
.icon:disabled, .ghost:disabled, .drop-acts button:disabled, .foot button:disabled { opacity: 0.4; }
.chev { color: rgba(255, 255, 255, 0.45); transition: transform 0.18s ease; }
.chev.open { transform: rotate(180deg); }
.body { display: flex; flex-direction: column; gap: 10px; padding: 2px 0 12px; }
.drop {
  width: 100%;
  border: 1px dashed rgba(255, 255, 255, 0.18);
  border-radius: 8px;
  background: transparent;
  color: var(--muted);
  padding: 14px 12px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
}
.drop-hint { display: inline-flex; align-items: center; justify-content: center; gap: 6px; font-size: 12px; text-align: center; }
.drop-acts { display: flex; flex-wrap: wrap; justify-content: center; gap: 6px; }
.shot { display: flex; gap: 12px; align-items: flex-start; }
.shot img { width: 96px; max-height: 140px; object-fit: cover; border-radius: 6px; background: #10111c; }
.shot-acts { display: flex; flex-wrap: wrap; gap: 6px; }
.shot-acts .danger { color: #ff8d8d; }
.slide { display: grid; grid-template-columns: 1fr auto; gap: 2px 8px; color: #fff; font-size: 12px; }
.slide.compact { gap: 0 8px; color: var(--muted); font-size: 11px; }
.slide input { grid-column: 1 / -1; }
.slide b { font-variant-numeric: tabular-nums; color: #fff; }
.foot { display: flex; align-items: center; gap: 6px; }
.foot .grow { flex: 1; }
.check { display: inline-flex; align-items: center; gap: 6px; margin-left: auto; color: var(--muted); font-size: 12px; }
.check input { width: 14px; height: 14px; min-width: 0; }
.banner {
  margin: 0;
  padding: 7px 10px;
  border-radius: 6px;
  background: rgba(227, 155, 74, 0.12);
  color: #e39b4a;
  font-size: 12px;
}
.banner button { border: 0; background: transparent; color: var(--heading); padding: 0 0 0 6px; }
.item {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  padding: 8px;
  border-radius: 8px;
  background: rgba(255, 255, 255, 0.04);
}
.item.off { opacity: 0.55; }
.item img { width: 44px; height: 44px; object-fit: cover; border-radius: 6px; flex: none; }
.item img.tall { height: 58px; }
.item-main { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px; }
.item-main select { height: 26px; padding: 0 8px; font-size: 12px; }
.item-side { display: flex; flex-direction: column; gap: 2px; }
.item-side button { width: 24px; height: 24px; border: 0; background: transparent; }
.item-side button.on { color: #fff; }
.note { margin: 6px 0 0; color: var(--muted); font-size: 12px; }
.mask { position: fixed; inset: 0; z-index: 50; background: rgba(0, 0, 0, 0.45); display: grid; place-items: center; padding: 20px; }
.sketch { width: min(820px, 100%); padding: 12px; border-radius: 12px; background: #12141f; border: 1px solid rgba(255, 255, 255, 0.08); }
.sketch header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px; }
.sketch header button { width: 28px; padding: 0; justify-content: center; }
.sketch canvas { width: 100%; height: auto; border-radius: 8px; background: #f4f1e8; touch-action: none; cursor: crosshair; }
.sketch-acts { display: flex; justify-content: flex-end; gap: 8px; margin-top: 8px; }
.sketch-acts .go { background: var(--heading); color: #12141f; }
</style>
