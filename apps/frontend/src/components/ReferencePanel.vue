<script setup lang="ts">
import { computed, ref } from "vue";
import { MAX_PRECISE_REFS, MAX_VIBE_IMAGES, supportsNAIPreciseReference, supportsNAIVibeTransfer, type PreciseReferenceType } from "@/types/nai";
import { useAppStore } from "@/stores/app";
import NaiIcon from "@/components/NaiIcon.vue";

const props = defineProps<{
  onPick: (file: File, kind?: "i2i" | "vibe" | "precise") => void;
}>();

const store = useAppStore();
const vibeMenu = ref(false);
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

function pick(kind: "i2i" | "vibe" | "precise", ev: Event) {
  const input = ev.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = "";
  if (file) props.onPick(file, kind);
}

function shortId(id: string) {
  const clean = id.replace(/-/g, "");
  return `${clean.slice(0, 6)}-${clean.slice(-6)}`;
}
</script>

<template>
  <section class="block">
    <h2>Reference Images</h2>

    <div class="card">
      <div class="head">
        <div class="copy">
          <NaiIcon name="i2i" :size="18" />
          <div>
            <strong>Image2Image</strong>
            <p>Transform your image.</p>
          </div>
        </div>
        <div class="acts">
          <button class="icon-sq tiny" type="button" title="Clear" :class="{ on: !!store.i2iImage }" @click="store.i2iImage = ''">
            <NaiIcon name="crop" :size="14" />
          </button>
          <label class="icon-sq tiny" title="Attach">
            <NaiIcon name="clip" :size="14" />
            <input type="file" accept="image/png,image/jpeg,image/webp" hidden @change="pick('i2i', $event)" />
          </label>
        </div>
      </div>
      <img v-if="store.i2iImage" class="preview" :src="store.i2iImage" alt="" />
      <label v-if="store.i2iImage" class="slide">
        <span>Strength</span><b>{{ store.i2iStrength.toFixed(2) }}</b>
        <input v-model.number="store.i2iStrength" type="range" min="0" max="1" step="0.01" />
      </label>
    </div>

    <div class="card">
      <div class="head">
        <div class="copy">
          <NaiIcon name="vibe" :size="18" />
          <div>
            <strong>Vibe Transfer <em>({{ store.vibeImages.length }}/{{ MAX_VIBE_IMAGES }})</em></strong>
            <p v-if="!vibeOk" class="hint">V5 不支持氛围迁移，请切换到 V4.5。</p>
          </div>
        </div>
        <div class="acts">
          <button v-if="!vibeOk" class="switch" type="button" @click="switchToV45">用 V4.5</button>
          <div class="menu-wrap">
            <button class="icon-sq tiny" type="button" title="More" @click="vibeMenu = !vibeMenu">
              <NaiIcon name="dots" :size="14" />
            </button>
            <div v-if="vibeMenu" class="menu" @mouseleave="vibeMenu = false">
              <button type="button" :disabled="!store.vibeImages.length" @click="store.vibeImages = []; vibeMenu = false">Clear all</button>
            </div>
          </div>
          <label class="icon-sq tiny" title="Add vibe" :class="{ off: !vibeOk || store.vibeImages.length >= MAX_VIBE_IMAGES }">
            <NaiIcon name="plus" :size="14" />
            <input
              type="file"
              accept="image/png,image/jpeg,image/webp"
              hidden
              :disabled="!vibeOk || store.vibeImages.length >= MAX_VIBE_IMAGES"
              @change="pick('vibe', $event)"
            />
          </label>
        </div>
      </div>

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

    <div class="card">
      <div class="head">
        <div class="copy">
          <NaiIcon name="precise" :size="18" />
          <div>
            <strong>Precise Reference</strong>
            <p v-if="!preciseOk" class="hint">精准参考仅 V4.5 可用，V5 首发未开放。</p>
          </div>
        </div>
        <div class="acts">
          <button v-if="!preciseOk" class="switch" type="button" @click="switchToV45">用 V4.5</button>
          <button
            class="icon-sq tiny"
            type="button"
            title="Clear"
            :disabled="!store.preciseReferences.length"
            @click="store.preciseReferences = []"
          >
            <NaiIcon name="crop" :size="14" />
          </button>
          <label class="icon-sq tiny" title="Attach" :class="{ off: !preciseOk || store.preciseReferences.length >= MAX_PRECISE_REFS }">
            <NaiIcon name="clip" :size="14" />
            <input
              type="file"
              accept="image/png,image/jpeg,image/webp"
              hidden
              :disabled="!preciseOk || store.preciseReferences.length >= MAX_PRECISE_REFS"
              @change="pick('precise', $event)"
            />
          </label>
        </div>
      </div>

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
.card { margin-bottom: 14px; }
.head { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.copy { display: flex; align-items: center; gap: 8px; }
.copy p { margin: 0; color: var(--muted); font-size: 12px; font-weight: 400; }
.copy .hint { margin: 2px 0 0; color: #ffb4b4; }
.switch {
  background: var(--heading); color: var(--bg0); border: 0; border-radius: 6px;
  padding: 5px 8px; font-size: 12px; white-space: nowrap;
}
.copy em { font-style: normal; color: var(--muted); font-weight: 600; }
.acts { display: flex; gap: 6px; align-items: center; }
.icon-sq {
  width: 28px; height: 28px; display: grid; place-items: center;
  background: var(--bg2); border: 1px solid var(--bg3); border-radius: 6px; color: #fff;
}
.icon-sq.on { color: var(--heading); border-color: var(--heading); }
.icon-sq.off { opacity: 0.4; }
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
