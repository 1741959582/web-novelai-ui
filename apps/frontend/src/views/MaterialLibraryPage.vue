<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useAppStore } from "@/stores/app";
import { useMaterialLibrary } from "@/stores/materialLibrary";
import { appendPromptChunk } from "@/utils/textUtils";
import NaiIcon from "@/components/NaiIcon.vue";

const route = useRoute();
const router = useRouter();
const app = useAppStore();
const lib = useMaterialLibrary();
const kind = computed(() => (route.meta.library as "blocks" | "vibes" | "precise") || "blocks");
const forI2i = computed(() => route.query.for === "i2i");
const draft = reactive({ name: "", positive: "", negative: "" });
const fileInput = ref<HTMLInputElement | null>(null);

const title = computed(() => (kind.value === "blocks" ? "提示词块" : kind.value === "vibes" ? "Vibe 库" : "精准参考库"));
const hint = computed(() => {
  if (kind.value === "blocks") return "把常用的一段提示词存成一块，点一下就加进当前草稿。";
  if (kind.value === "vibes") return "存下风格参考图。生成页的风格迁移可以从这里加回去。";
  return forI2i.value ? "选一张图，把它设成图生图底图。" : "存下角色或风格参考。生成页的精准参考可以从这里加回去。";
});

function readFile(file: File, done: (url: string) => void) {
  const reader = new FileReader();
  reader.onload = () => done(String(reader.result || ""));
  reader.readAsDataURL(file);
}

function saveBlock() {
  if (!draft.positive.trim() && !draft.negative.trim()) return;
  lib.addBlock({
    name: draft.name.trim() || "未命名提示词块",
    positive: draft.positive.trim(),
    negative: draft.negative.trim(),
  });
  draft.name = "";
  draft.positive = "";
  draft.negative = "";
}

function useBlock(id: string) {
  const block = lib.blocks.find((item) => item.id === id);
  if (!block) return;
  if (block.positive) app.params.positivePrompt = appendPromptChunk(app.params.positivePrompt, block.positive);
  if (block.negative) app.params.negativePrompt = appendPromptChunk(app.params.negativePrompt, block.negative);
  app.status = `已插入提示词块「${block.name}」`;
  void router.push("/");
}

function onUpload(event: Event) {
  const file = (event.target as HTMLInputElement).files?.[0];
  if (!file) return;
  readFile(file, (url) => {
    const name = file.name.replace(/\.[^.]+$/, "");
    if (kind.value === "vibes") lib.addVibe({ name, previewUrl: url, infoExtracted: 0.7, strength: 0.6 });
    else lib.addPrecise({ name, previewUrl: url, type: "character&style", strength: 1, fidelity: 1 });
  });
  (event.target as HTMLInputElement).value = "";
}

function saveCurrent() {
  if (kind.value === "vibes") {
    for (const vibe of app.vibeImages) {
      lib.addVibe({
        name: "当前风格",
        previewUrl: vibe.previewUrl,
        infoExtracted: vibe.infoExtracted,
        strength: vibe.strength,
      });
    }
    app.status = app.vibeImages.length ? "已把当前风格迁移存进库" : "当前没有风格迁移图";
    return;
  }
  for (const item of app.preciseReferences) {
    lib.addPrecise({
      name: "当前精准参考",
      previewUrl: item.previewUrl,
      type: item.type,
      strength: item.strength,
      fidelity: item.fidelity,
    });
  }
  app.status = app.preciseReferences.length ? "已把当前精准参考存进库" : "当前没有精准参考";
}

function useVibe(id: string) {
  const item = lib.vibes.find((vibe) => vibe.id === id);
  if (!item || !app.useImageAs("vibe", item.previewUrl)) return;
  const added = app.vibeImages[app.vibeImages.length - 1];
  if (added) app.updateVibeImage(added.id, { infoExtracted: item.infoExtracted, strength: item.strength });
  void router.push("/");
}

function usePrecise(id: string) {
  const item = lib.precise.find((entry) => entry.id === id);
  if (!item) return;
  if (forI2i.value) {
    app.useImageAs("i2i", item.previewUrl);
    app.status = "已设为图生图底图";
    void router.push("/");
    return;
  }
  if (!app.useImageAs("precise", item.previewUrl)) return;
  const added = app.preciseReferences[app.preciseReferences.length - 1];
  if (added) app.updatePreciseReference(added.id, { type: item.type, strength: item.strength, fidelity: item.fidelity });
  void router.push("/");
}
</script>

<template>
  <div class="page">
    <header>
      <div>
        <h2>{{ title }}</h2>
        <p>{{ hint }}</p>
      </div>
      <div class="acts">
        <button v-if="kind !== 'blocks'" class="btn" type="button" @click="saveCurrent">保存当前</button>
        <button v-if="kind !== 'blocks'" class="btn primary" type="button" @click="fileInput?.click()">上传图片</button>
        <input ref="fileInput" type="file" accept="image/png,image/jpeg,image/webp" hidden @change="onUpload" />
      </div>
    </header>
    <p v-if="lib.error" class="err">{{ lib.error }}</p>

    <form v-if="kind === 'blocks'" class="editor" @submit.prevent="saveBlock">
      <input v-model="draft.name" placeholder="名称，例如：夕光教室" />
      <textarea v-model="draft.positive" rows="3" placeholder="正面提示词" />
      <textarea v-model="draft.negative" rows="2" placeholder="负面提示词，可空" />
      <button class="btn primary" type="submit">保存这块</button>
    </form>

    <div v-if="kind === 'blocks'" class="grid">
      <p v-if="!lib.blocks.length" class="empty">还没有提示词块。</p>
      <article v-for="block in lib.blocks" :key="block.id">
        <strong>{{ block.name }}</strong>
        <p>{{ block.positive || "（没有正面）" }}</p>
        <small v-if="block.negative">负面 {{ block.negative }}</small>
        <div>
          <button class="btn primary" type="button" @click="useBlock(block.id)">插入草稿</button>
          <button class="btn" type="button" @click="lib.removeBlock(block.id)"><NaiIcon name="trash" :size="14" /></button>
        </div>
      </article>
    </div>

    <div v-else-if="kind === 'vibes'" class="grid">
      <p v-if="!lib.vibes.length" class="empty">还没有风格图。可以上传，或把生成页里正在用的存进来。</p>
      <article v-for="item in lib.vibes" :key="item.id">
        <img :src="item.previewUrl" alt="" />
        <strong>{{ item.name }}</strong>
        <small>提取 {{ item.infoExtracted.toFixed(2) }} · 强度 {{ item.strength.toFixed(2) }}</small>
        <div>
          <button class="btn primary" type="button" @click="useVibe(item.id)">加入风格迁移</button>
          <button class="btn" type="button" @click="lib.removeVibe(item.id)"><NaiIcon name="trash" :size="14" /></button>
        </div>
      </article>
    </div>

    <div v-else class="grid">
      <p v-if="!lib.precise.length" class="empty">还没有精准参考。可以上传，或把生成页里正在用的存进来。</p>
      <article v-for="item in lib.precise" :key="item.id">
        <img :src="item.previewUrl" alt="" />
        <strong>{{ item.name }}</strong>
        <small>{{ item.type }} · 强度 {{ item.strength.toFixed(2) }}</small>
        <div>
          <button class="btn primary" type="button" @click="usePrecise(item.id)">
            {{ forI2i ? "用作图生图" : "加入精准参考" }}
          </button>
          <button class="btn" type="button" @click="lib.removePrecise(item.id)"><NaiIcon name="trash" :size="14" /></button>
        </div>
      </article>
    </div>
  </div>
</template>

<style scoped>
.page { height: 100%; overflow: auto; padding: 18px 20px 28px; }
header { display: flex; align-items: flex-start; justify-content: space-between; gap: 16px; margin-bottom: 14px; }
h2 { margin: 0 0 4px; font-size: 22px; }
p { margin: 0; color: var(--muted); font-size: 13px; }
.acts { display: flex; gap: 8px; }
.err { color: #ffb4b4; }
.editor, article {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 12px;
  border-radius: 12px;
  background: #1a1e38;
  border: 1px solid rgba(255, 255, 255, 0.08);
}
.editor { margin-bottom: 14px; }
.editor textarea, .editor input { width: 100%; }
.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 12px; }
article img { width: 100%; aspect-ratio: 1; object-fit: cover; border-radius: 8px; background: #10111c; }
article strong { font-size: 14px; }
article p, article small { color: var(--muted); font-size: 12px; }
article p { display: -webkit-box; -webkit-line-clamp: 3; -webkit-box-orient: vertical; overflow: hidden; }
article div { display: flex; gap: 8px; }
.empty { grid-column: 1 / -1; }
</style>
