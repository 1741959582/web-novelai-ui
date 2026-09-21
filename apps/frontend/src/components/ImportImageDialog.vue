<script setup lang="ts">
import { computed, reactive } from "vue";
import { supportsNAIPreciseReference, supportsNAIVibeTransfer, type MetadataReport } from "@/types/nai";
import { useAppStore } from "@/stores/app";
import NaiIcon from "@/components/NaiIcon.vue";

const props = defineProps<{
  preview: string;
  report: MetadataReport;
}>();

const emit = defineEmits<{ close: [] }>();
const store = useAppStore();
const hasMeta = computed(() =>
  Boolean(
    props.report.hasMetadata ||
      props.report.prompt ||
      props.report.negative ||
      props.report.characterCaptions?.length ||
      /novelai/i.test(`${props.report.software} ${props.report.model}`),
  ),
);
const vibeOk = computed(() => supportsNAIVibeTransfer(store.params.model));
const preciseOk = computed(() => supportsNAIPreciseReference(store.params.model));
const opts = reactive({
  prompt: true,
  uc: true,
  characters: true,
  append: false,
  settings: true,
  seed: false,
  clean: true,
});

function useAs(kind: "i2i" | "vibe" | "precise") {
  if (kind === "vibe" && !vibeOk.value) return;
  if (kind === "precise" && !preciseOk.value) return;
  if (kind === "i2i") store.i2iImage = props.preview;
  if (kind === "vibe") store.addVibeFromDataUrl(props.preview);
  if (kind === "precise") store.addPreciseFromDataUrl(props.preview);
  store.status =
    kind === "i2i" ? "已设为 Image2Image 参考图" : kind === "vibe" ? "已设为 Vibe Transfer 参考图" : "已设为 Precise Reference";
  emit("close");
}

function importMeta() {
  store.applyImportedMetadata(props.report, { ...opts });
  emit("close");
}
</script>

<template>
  <div class="back" @click.self="emit('close')">
    <section class="panel" role="dialog" aria-labelledby="import-title">
      <header>
        <h2 id="import-title">What do you want to do with this image?</h2>
        <button class="ghost" type="button" @click="emit('close')"><NaiIcon name="close" :size="16" /></button>
      </header>
      <div class="preview">
        <img :src="preview" alt="" />
      </div>
      <div class="actions">
        <button type="button" class="cream" @click="useAs('i2i')">
          <NaiIcon name="i2i" :size="16" /> Image2Image
        </button>
        <button type="button" class="cream" :disabled="!vibeOk" :title="vibeOk ? '' : 'Vibe Transfer 需要 V3 / V4 / V4.5'" @click="useAs('vibe')">
          <NaiIcon name="vibe" :size="16" /> Vibe Transfer
        </button>
        <button type="button" class="cream" :disabled="!preciseOk" :title="preciseOk ? '' : 'Precise Reference 仅 V4.5'" @click="useAs('precise')">
          <NaiIcon name="precise" :size="16" /> Precise Reference
        </button>
      </div>

      <template v-if="hasMeta">
        <p class="meta-title">This image has metadata!<br />Did you want to import that instead?</p>
        <div class="grid">
          <label><input v-model="opts.prompt" type="checkbox" /> Prompt</label>
          <span />
          <label><input v-model="opts.uc" type="checkbox" /> Undesired Content</label>
          <button type="button" class="cream import" @click="importMeta">Import Metadata</button>
          <label><input v-model="opts.characters" type="checkbox" /> Characters</label>
          <label class="right"><input v-model="opts.clean" type="checkbox" /> Clean Imports</label>
          <label><input v-model="opts.append" type="checkbox" /> Append</label>
          <span />
          <label><input v-model="opts.settings" type="checkbox" /> Settings</label>
          <span />
          <label class="off"><input v-model="opts.seed" type="checkbox" /> Seed</label>
        </div>
      </template>
    </section>
  </div>
</template>

<style scoped>
.back {
  position: fixed;
  inset: 0;
  z-index: 50;
  background: rgba(14, 15, 33, 0.72);
  display: grid;
  place-items: center;
  padding: 24px;
}
.panel {
  width: min(520px, 100%);
  background: #191b31;
  border: 1px solid #22253f;
  border-radius: 12px;
  padding: 22px 24px 20px;
  box-shadow: 0 18px 50px rgba(0, 0, 0, 0.45);
}
header { display: flex; align-items: flex-start; justify-content: space-between; gap: 12px; }
h2 {
  margin: 0;
  font-family: Eczar, serif;
  color: #f5f3c2;
  font-size: 26px;
  font-weight: 600;
  line-height: 1.25;
}
.ghost { background: none; border: 0; color: #fff; }
.preview {
  margin: 18px auto 16px;
  width: 220px;
  height: 160px;
  background: #0e0f21;
  border-radius: 8px;
  display: grid;
  place-items: center;
  overflow: hidden;
}
.preview img { max-width: 100%; max-height: 100%; object-fit: contain; }
.actions { display: flex; flex-wrap: wrap; gap: 10px; justify-content: center; }
.cream {
  background: #f5f3c2;
  color: #0e0f21;
  border: 0;
  border-radius: 6px;
  padding: 8px 12px;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-weight: 700;
}
.cream:disabled { opacity: 0.4; cursor: not-allowed; }
.meta-title {
  margin: 22px 0 12px;
  color: #fff;
  font-size: 15px;
  line-height: 1.45;
}
.grid {
  display: grid;
  grid-template-columns: 1fr auto;
  gap: 10px 16px;
  align-items: center;
}
.grid label { display: flex; align-items: center; gap: 8px; color: #fff; }
.grid input { accent-color: #f5f3c2; width: 16px; height: 16px; }
.right { justify-content: flex-end; }
.off { opacity: 0.85; }
.import { justify-self: end; }
</style>
