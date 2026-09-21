<script setup lang="ts">
import { ref } from "vue";
import { inspectImage, inspectImageBytes, pickOutputDir } from "@/api/tauri";
import { useAppStore } from "@/stores/app";
import type { MetadataReport } from "@/types/nai";

const store = useAppStore();
const report = ref<MetadataReport | null>(null);
const error = ref("");

async function onFile(ev: Event) {
  const file = (ev.target as HTMLInputElement).files?.[0];
  if (!file) return;
  error.value = "";
  const dataUrl = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
  try {
    report.value = await inspectImageBytes(dataUrl);
  } catch (e) {
    error.value = String(e);
  }
}

function apply() {
  if (!report.value) return;
  if (report.value.prompt) store.params.positivePrompt = report.value.prompt;
  if (report.value.negative) store.params.negativePrompt = report.value.negative;
  if (report.value.model) store.params.model = report.value.model;
  if (report.value.seed != null) {
    store.params.seed = report.value.seed;
    store.params.seedMode = "fixed";
  }
  if (report.value.width) store.params.width = report.value.width;
  if (report.value.height) store.params.height = report.value.height;
  if (report.value.steps) store.params.steps = report.value.steps;
  if (report.value.sampler) store.params.sampler = report.value.sampler;
  store.status = "已把兼容参数应用到生成页";
}

async function pickFromDisk() {
  const dir = await pickOutputDir();
  if (!dir) return;
  store.status = `当前选择的是目录 ${dir}，请用上方文件选择器打开具体图片。`;
}

async function inspectHistory(path: string) {
  try {
    report.value = await inspectImage(path);
    error.value = "";
  } catch (e) {
    error.value = String(e);
  }
}
</script>

<template>
  <div class="wrap">
    <div class="card">
      <h2>原数据</h2>
      <p class="hint">本地读取 PNG 中的 NovelAI / SD WebUI / ComfyUI 元数据，再把兼容参数套回生成页。</p>
      <div class="field">
        <label>选择图片</label>
        <input type="file" accept="image/png,image/jpeg,image/webp" @change="onFile" />
      </div>
      <button class="btn" type="button" @click="pickFromDisk">打开输出目录选择器</button>
      <div v-if="store.history.length" class="hist">
        <button v-for="h in store.history.slice(0, 12)" :key="h.id" class="btn" type="button" @click="inspectHistory(h.path)">
          读取历史 {{ h.seed }}
        </button>
      </div>
      <p v-if="error" class="err">{{ error }}</p>
    </div>
    <div v-if="report" class="card">
      <p>类型：{{ report.kind }} · {{ report.software }}</p>
      <p>模型：{{ report.model || "—" }} · seed {{ report.seed ?? "—" }} · {{ report.width }}×{{ report.height }}</p>
      <div class="field"><label>正面</label><textarea :value="report.prompt" readonly /></div>
      <div class="field"><label>负面</label><textarea :value="report.negative" readonly /></div>
      <button class="btn primary" type="button" @click="apply">应用到生成页</button>
      <pre class="raw">{{ report.rawText }}</pre>
    </div>
  </div>
</template>

<style scoped>
.wrap { padding: 20px; display: grid; gap: 16px; max-width: 980px; }
.err { color: var(--danger); }
.raw { white-space: pre-wrap; font-size: 12px; color: var(--muted); max-height: 240px; overflow: auto; }
.hist { display: flex; flex-wrap: wrap; gap: 8px; margin-top: 12px; }
</style>
