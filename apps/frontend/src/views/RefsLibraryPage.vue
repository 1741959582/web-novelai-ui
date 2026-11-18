<script setup lang="ts">
import { computed, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useAppStore } from "@/stores/app";
import { useMaterialLibrary, type SavedPrecise } from "@/stores/materialLibrary";
import NaiIcon from "@/components/NaiIcon.vue";

const route = useRoute();
const router = useRouter();
const app = useAppStore();
const lib = useMaterialLibrary();
const kind = computed(() => (route.meta.library === "precise" ? "precise" : "vibes"));
const forI2i = computed(() => route.query.for === "i2i");
const view = ref<"all" | "favorite" | "recent" | "active" | "type" | "category">("all");
const typeFilter = ref<SavedPrecise["type"] | "">("");
const categoryId = ref("");
const query = ref("");
const importType = ref<SavedPrecise["type"]>("character&style");
const fileInput = ref<HTMLInputElement | null>(null);
const status = ref("");

const title = computed(() => (kind.value === "vibes" ? "Vibe 库" : "精准参考库"));

const shown = computed(() => {
  const q = query.value.trim().toLowerCase();
  if (kind.value === "vibes") {
    return lib.vibes.filter((item) => {
      if (view.value === "favorite" && !item.favorite) return false;
      if (view.value === "recent" && !item.usedAt) return false;
      if (view.value === "active" && !app.vibeImages.some((vibe) => vibe.previewUrl === item.previewUrl)) return false;
      if (view.value === "category" && item.categoryId !== categoryId.value) return false;
      return !q || item.name.toLowerCase().includes(q);
    });
  }
  return lib.precise.filter((item) => {
    if (view.value === "favorite" && !item.favorite) return false;
    if (view.value === "recent" && !item.usedAt) return false;
    if (view.value === "active" && !app.preciseReferences.some((entry) => entry.previewUrl === item.previewUrl)) return false;
    if (view.value === "type" && typeFilter.value && item.type !== typeFilter.value) return false;
    return !q || item.name.toLowerCase().includes(q);
  });
});

function readFile(file: File) {
  if (!file.type.startsWith("image/")) {
    status.value = `${file.name} 不是图片。Vibe 编码文件先导出成图片再入库。`;
    return;
  }
  const reader = new FileReader();
  reader.onload = () => {
    const url = String(reader.result || "");
    if (kind.value === "vibes") lib.addVibe({ name: file.name.replace(/\.[^.]+$/, ""), previewUrl: url, categoryId: categoryId.value });
    else lib.addPrecise({ name: file.name.replace(/\.[^.]+$/, ""), previewUrl: url, type: importType.value });
    status.value = "";
  };
  reader.readAsDataURL(file);
}

function onFiles(files: FileList | File[] | null | undefined) {
  for (const file of files || []) readFile(file);
}

function onDrop(event: DragEvent) {
  event.preventDefault();
  onFiles(event.dataTransfer?.files);
}

async function pasteClipboard() {
  try {
    const items = await navigator.clipboard.read();
    let found = false;
    for (const item of items) {
      const type = item.types.find((value) => value.startsWith("image/"));
      if (!type) continue;
      const blob = await item.getType(type);
      readFile(new File([blob], "clipboard.png", { type }));
      found = true;
    }
    if (!found) status.value = "剪贴板里没有图片";
  } catch {
    status.value = "浏览器没有允许读取剪贴板";
  }
}

function useItem(id: string) {
  if (kind.value === "vibes") {
    const item = lib.vibes.find((entry) => entry.id === id);
    if (!item || !app.useImageAs("vibe", item.previewUrl)) return;
    const added = app.vibeImages[app.vibeImages.length - 1];
    if (added) app.updateVibeImage(added.id, { infoExtracted: item.infoExtracted, strength: item.strength });
    lib.touchVibe(id);
  } else {
    const item = lib.precise.find((entry) => entry.id === id);
    if (!item) return;
    if (forI2i.value) {
      app.useImageAs("i2i", item.previewUrl);
      app.status = "已设为图生图底图";
    } else if (!app.useImageAs("precise", item.previewUrl)) return;
    else {
      const added = app.preciseReferences[app.preciseReferences.length - 1];
      if (added) app.updatePreciseReference(added.id, { type: item.type, strength: item.strength, fidelity: item.fidelity });
    }
    lib.touchPrecise(id);
  }
  void router.push("/");
}

function pickType(type: SavedPrecise["type"]) {
  typeFilter.value = type;
  importType.value = type;
  view.value = "type";
}

function addCategory() {
  const name = window.prompt("分类名称");
  if (!name?.trim()) return;
  const category = lib.addCategory(name);
  categoryId.value = category.id;
  view.value = "category";
}
</script>

<template>
  <div class="lib">
    <aside>
      <header>
        <NaiIcon :name="kind === 'vibes' ? 'vibe' : 'precise'" :size="16" />
        <strong>{{ title }}</strong>
      </header>
      <button type="button" :class="{ on: view === 'all' }" @click="view = 'all'"><NaiIcon name="grid" :size="14" /> 全部</button>
      <button type="button" :class="{ on: view === 'favorite' }" @click="view = 'favorite'"><NaiIcon name="star" :size="14" /> 收藏</button>
      <button type="button" :class="{ on: view === 'recent' }" @click="view = 'recent'"><NaiIcon name="history" :size="14" /> 最近使用</button>
      <button type="button" :class="{ on: view === 'active' }" @click="view = 'active'"><NaiIcon name="check" :size="14" /> 在生图页</button>
      <template v-if="kind === 'precise'">
        <p class="label">参考类型</p>
        <button type="button" :class="{ on: view === 'type' && typeFilter === 'character' }" @click="pickType('character')">角色</button>
        <button type="button" :class="{ on: view === 'type' && typeFilter === 'style' }" @click="pickType('style')">风格</button>
        <button type="button" :class="{ on: view === 'type' && typeFilter === 'character&style' }" @click="pickType('character&style')">角色+风格</button>
        <p class="note">导入的图片默认按 {{ importType === "character" ? "角色" : importType === "style" ? "风格" : "角色+风格" }} 存。选中左边某个类型再导入，就按那个类型存。</p>
      </template>
      <template v-else>
        <div class="label-row">
          <span>分类</span>
          <button type="button" title="新建分类" @click="addCategory"><NaiIcon name="plus" :size="13" /></button>
        </div>
        <p v-if="!lib.categories.length" class="note">还没有分类。导入时可以顺手建一个。</p>
        <button v-for="item in lib.categories" :key="item.id" type="button" :class="{ on: view === 'category' && categoryId === item.id }" @click="categoryId = item.id; view = 'category'">
          {{ item.name }}
        </button>
      </template>
    </aside>
    <section>
      <header class="bar">
        <label class="search">
          <NaiIcon name="search" :size="14" />
          <input v-model="query" :placeholder="kind === 'vibes' ? '搜索名称、标签' : '搜索名称'" />
        </label>
        <button class="primary" type="button" @click="fileInput?.click()"><NaiIcon name="plus" :size="14" /> 导入</button>
        <input ref="fileInput" hidden type="file" accept="image/png,image/jpeg,image/webp,.naiv4vibe,.naiv4vibebundle" multiple @change="onFiles(($event.target as HTMLInputElement).files)" />
      </header>
      <div class="stage" @dragover.prevent @drop="onDrop">
        <div v-if="!shown.length" class="drop">
          <NaiIcon name="upload" :size="22" />
          <strong>{{ kind === "vibes" ? "把图片或 Vibe 文件拖到这里" : "把参考图拖到这里" }}</strong>
          <p v-if="kind === 'vibes'">支持 PNG（内含 Vibe）、.naiv4vibe、.naiv4vibebundle 和普通图片。普通图片先存原图，第一次用在生图时才编码。</p>
          <p v-else>角色设定图、风格样张都行，一次可以拖很多张。存进来时记住类型、强度和保真度，发送时直接套用。</p>
          <div class="acts">
            <button class="primary" type="button" @click="fileInput?.click()">选择文件</button>
            <button type="button" @click="pasteClipboard">从剪贴板粘贴</button>
          </div>
          <p class="hint">生图结果右键可以存进这个库，本地图库也可以。</p>
          <p v-if="status" class="warn">{{ status }}</p>
        </div>
        <div v-else class="grid">
          <article v-for="item in shown" :key="item.id">
            <img :src="item.previewUrl" alt="" />
            <header>
              <strong>{{ item.name }}</strong>
              <button type="button" :class="{ on: item.favorite }" @click="kind === 'vibes' ? lib.updateVibe(item.id, { favorite: !item.favorite }) : lib.updatePrecise(item.id, { favorite: !item.favorite })">
                <NaiIcon name="star" :size="14" />
              </button>
            </header>
            <small v-if="kind === 'vibes'">提取 {{ (item as any).infoExtracted.toFixed(2) }} · 强度 {{ (item as any).strength.toFixed(2) }}</small>
            <small v-else>{{ (item as SavedPrecise).type }} · 强度 {{ (item as SavedPrecise).strength.toFixed(2) }}</small>
            <div class="acts">
              <button class="primary" type="button" @click="useItem(item.id)">{{ forI2i ? "用作图生图" : "发到生图" }}</button>
              <button type="button" @click="kind === 'vibes' ? lib.removeVibe(item.id) : lib.removePrecise(item.id)"><NaiIcon name="trash" :size="14" /></button>
            </div>
          </article>
        </div>
        <aside class="help">
          <strong>Vibe 和精准参考有什么区别？</strong>
          <p>Vibe：借整体氛围和画风，可以叠很多张（V4 / V4.5，V5 没有）。精准参考：照着一张图的人物或画风来画，更准，每张 5 Anlas（只在 V4.5）。两者不能同时生效。</p>
        </aside>
      </div>
    </section>
  </div>
</template>

<style scoped>
.lib { height: 100%; display: grid; grid-template-columns: 240px 1fr; background: #12141f; color: #fff; }
aside { display: flex; flex-direction: column; gap: 2px; padding: 8px; border-right: 1px solid rgba(255,255,255,.08); background: #16182c; overflow: auto; }
header { display: flex; align-items: center; gap: 8px; }
aside > header { height: 40px; padding: 0 8px; }
.label, .note { margin: 12px 8px 4px; color: rgba(255,255,255,.45); font-size: 12px; line-height: 1.45; }
.label-row { display: flex; align-items: center; justify-content: space-between; margin: 12px 8px 4px; color: rgba(255,255,255,.55); font-size: 12px; }
.label-row button, aside > button {
  display: flex; align-items: center; gap: 8px; width: 100%; height: 34px; padding: 0 8px;
  border: 0; border-radius: 8px; background: transparent; color: rgba(255,255,255,.82); text-align: left;
}
.label-row button { width: 28px; justify-content: center; }
aside > button.on { background: rgba(255,255,255,.08); }
section { min-width: 0; display: flex; flex-direction: column; }
.bar { display: flex; align-items: center; gap: 12px; height: 56px; padding: 0 16px; border-bottom: 1px solid rgba(255,255,255,.08); }
.search { flex: 1; display: flex; align-items: center; gap: 8px; height: 34px; padding: 0 10px; border-radius: 8px; background: rgba(255,255,255,.06); color: rgba(255,255,255,.45); }
.search input { flex: 1; height: 100%; border: 0; background: transparent; color: #fff; }
.primary { border: 0; border-radius: 8px; background: #3b82f6; color: #fff; height: 34px; padding: 0 12px; display: inline-flex; align-items: center; gap: 6px; }
.stage { flex: 1; min-height: 0; overflow: auto; padding: 28px 24px; display: flex; flex-direction: column; gap: 16px; }
.drop {
  margin: auto; width: min(640px, 100%); padding: 36px 28px; text-align: center;
  border: 1px dashed rgba(255,255,255,.2); border-radius: 16px; display: grid; justify-items: center; gap: 10px;
}
.drop p, .help p { margin: 0; color: rgba(255,255,255,.55); line-height: 1.55; }
.hint { font-size: 12px; }
.warn { color: #e39b4a; }
.acts { display: flex; gap: 8px; flex-wrap: wrap; justify-content: center; }
.acts button, article button { height: 32px; padding: 0 10px; border-radius: 8px; border: 1px solid rgba(255,255,255,.12); background: rgba(255,255,255,.04); color: #fff; }
.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 12px; }
article { display: flex; flex-direction: column; gap: 8px; padding: 8px; border-radius: 12px; background: #1a1e38; border: 1px solid rgba(255,255,255,.08); }
article img { width: 100%; aspect-ratio: 1; object-fit: cover; border-radius: 8px; background: #10111c; }
article header { justify-content: space-between; }
article header button { border: 0; background: transparent; color: rgba(255,255,255,.4); }
article header button.on { color: #f5c16c; }
article small { color: rgba(255,255,255,.5); }
.help { padding: 12px 14px; border-radius: 12px; border: 1px solid rgba(255,255,255,.08); background: rgba(255,255,255,.03); }
</style>
