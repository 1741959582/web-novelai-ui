<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { fetchRemoteImage, readImageDataUrl } from "@/api/tauri";
import { useAppStore } from "@/stores/app";
import { useMaterialLibrary, type SavedPrecise } from "@/stores/materialLibrary";
import { PRECISE_TYPE_OPTIONS, preciseTypeForGroup, preciseTypeGroup, preciseTypeLabel, type PreciseReferenceGroup } from "@/types/nai";
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
    if (view.value === "type" && typeFilter.value && preciseTypeGroup(item.type) !== preciseTypeGroup(typeFilter.value)) return false;
    return !q || item.name.toLowerCase().includes(q);
  });
});

function isImageFile(file: File) {
  if (file.type.startsWith("image/")) return true;
  return /\.(png|jpe?g|webp|gif|bmp)$/i.test(file.name);
}

function baseName(path: string) {
  const name = path.split(/[\\/]/).pop() || "";
  return name.replace(/\.[^.]+$/, "");
}

function libraryName(raw: string) {
  const name = baseName(raw).trim();
  const fallback = kind.value === "vibes" ? "Vibe" : "精准参考";
  if (!name || /^[0-9a-f]{12,}$/i.test(name)) return fallback;
  return name;
}

async function storePreview(name: string, previewUrl: string) {
  const saved =
    kind.value === "vibes"
      ? await lib.addVibe({ name, previewUrl, categoryId: categoryId.value })
      : await lib.addPrecise({ name, previewUrl, type: importType.value, strength: 1, fidelity: 1 });
  if (!saved) {
    status.value = lib.error || "这张图没有存进去";
    return false;
  }
  return true;
}

function readFile(file: File) {
  if (!isImageFile(file)) {
    status.value = `${file.name || "这个文件"} 不是图片。`;
    return Promise.resolve(false);
  }
  return new Promise<boolean>((resolve) => {
    const reader = new FileReader();
    reader.onload = () => {
      const url = String(reader.result || "");
      if (!url.startsWith("data:image/")) {
        resolve(false);
        return;
      }
      void storePreview(libraryName(file.name || ""), url).then(resolve);
    };
    reader.onerror = () => {
      status.value = "读不到这张图片";
      resolve(false);
    };
    reader.readAsDataURL(file);
  });
}

async function onFiles(files: FileList | File[] | null | undefined) {
  const list = [...(files || [])];
  if (!list.length) return;
  let added = 0;
  for (const file of list) {
    if (await readFile(file)) added += 1;
  }
  if (added) status.value = added === 1 ? "已加入 1 张" : `已加入 ${added} 张`;
}

function onPicked(event: Event) {
  const input = event.target as HTMLInputElement;
  void onFiles(input.files);
  input.value = "";
}

function isHttpUrl(value: string) {
  return /^https?:\/\//i.test(value.trim());
}

function isLocalPath(value: string) {
  const path = value.trim();
  return /^(?:\\\\\?\\)?([a-zA-Z]:[\\/]|\\\\|\/)/.test(path) && !isHttpUrl(path);
}

function fileUrlToPath(value: string) {
  const raw = value.trim();
  if (!/^file:/i.test(raw)) return "";
  try {
    const url = new URL(raw);
    let path = decodeURIComponent(url.pathname);
    if (/^\/[a-zA-Z]:/.test(path)) path = path.slice(1);
    return path.replace(/\//g, "\\");
  } catch {
    return raw.replace(/^file:\/+/i, "").replace(/\//g, "\\");
  }
}

function pathFromTransferText(value: string) {
  const item = value.trim();
  if (!item || item.startsWith("#")) return "";
  if (isHttpUrl(item)) return "";
  if (/^file:/i.test(item)) return fileUrlToPath(item);
  if (isLocalPath(item)) return item;
  return "";
}

function safeTransferText(dt: DataTransfer | null | undefined, type: string) {
  try {
    return dt?.getData(type) || "";
  } catch {
    return "";
  }
}

function dropSourcesFromTransfer(dt: DataTransfer | null | undefined) {
  const urls: string[] = [];
  const paths: string[] = [];
  if (!dt) return { urls, paths };
  const uri = `${safeTransferText(dt, "text/uri-list")}\n${safeTransferText(dt, "text/plain")}`;
  for (const line of uri.split(/\r?\n/)) {
    const item = line.trim();
    if (!item || item.startsWith("#")) continue;
    if (isHttpUrl(item)) urls.push(item);
    else {
      const path = pathFromTransferText(item);
      if (path) paths.push(path);
    }
  }
  const html = safeTransferText(dt, "text/html");
  for (const match of html.matchAll(/<img[^>]+src=["']([^"']+)/gi)) {
    const src = match[1];
    if (src?.startsWith("data:image/") && src.length < 12_000_000) urls.push(src);
    else if (src && isHttpUrl(src)) urls.push(src);
    else if (src) {
      const path = pathFromTransferText(src);
      if (path) paths.push(path);
    }
  }
  return { urls: [...new Set(urls)], paths: [...new Set(paths)] };
}

async function addFromSources(urls: string[], paths: string[]) {
  const total = urls.length + paths.length;
  if (!total) {
    status.value = "拖入的不是图片。可以从文件夹拖图片，或在网页里复制图片后再粘贴。";
    return;
  }
  let added = 0;
  for (const path of paths) {
    try {
      if (storePreview(libraryName(path), await readImageDataUrl(path))) added += 1;
    } catch (error) {
      status.value = error instanceof Error ? error.message : String(error);
    }
  }
  for (const url of urls) {
    try {
      const image = url.startsWith("data:image/") ? url : await fetchRemoteImage(url);
      if (storePreview(libraryName(url.split("?")[0]), image)) added += 1;
    } catch (error) {
      status.value = error instanceof Error ? error.message : String(error);
    }
  }
  if (added) status.value = added === 1 ? "已加入 1 张" : `已加入 ${added} 张`;
}

function imagesFromClipboard(data: DataTransfer | null) {
  if (!data) return [] as File[];
  const files: File[] = [];
  for (const item of data.items) {
    if (item.kind !== "file") continue;
    const file = item.getAsFile();
    if (file && isImageFile(file)) files.push(file);
  }
  if (!files.length) {
    for (const file of data.files) if (isImageFile(file)) files.push(file);
  }
  return files;
}

function editingText(target: EventTarget | null) {
  const el = target instanceof HTMLElement ? target : null;
  if (!el) return false;
  return el.tagName === "INPUT" || el.tagName === "TEXTAREA" || el.isContentEditable;
}

async function pasteClipboard() {
  if (!navigator.clipboard?.read) {
    status.value = "这里读不到剪贴板，请直接按 Ctrl+V";
    return;
  }
  try {
    const items = await navigator.clipboard.read();
    const files: File[] = [];
    for (const item of items) {
      const type = item.types.find((value) => value.startsWith("image/"));
      if (!type) continue;
      const blob = await item.getType(type);
      const ext = type.split("/")[1]?.replace("jpeg", "jpg") || "png";
      files.push(new File([blob], `粘贴图.${ext}`, { type }));
    }
    if (!files.length) {
      status.value = "剪贴板里没有图片";
      return;
    }
    await onFiles(files);
  } catch {
    status.value = "没有读到剪贴板图片。可以拖入文件，或先复制图片再按 Ctrl+V。";
  }
}

let pasteSeen = false;

function onPaste(ev: ClipboardEvent) {
  pasteSeen = true;
  const files = imagesFromClipboard(ev.clipboardData);
  if (files.length) {
    ev.preventDefault();
    void onFiles(files);
    return;
  }
  if (editingText(ev.target)) return;
  const text = ev.clipboardData?.getData("text/plain") ?? "";
  const urls: string[] = [];
  const paths: string[] = [];
  for (const line of text.split(/\r?\n/)) {
    const item = line.trim();
    if (!item) continue;
    if (item.startsWith("data:image/")) urls.push(item);
    else if (isHttpUrl(item)) urls.push(item);
    else {
      const path = pathFromTransferText(item);
      if (path) paths.push(path);
    }
  }
  if (urls.length || paths.length) {
    ev.preventDefault();
    void addFromSources(urls, paths);
    return;
  }
  const types = [...(ev.clipboardData?.types ?? [])];
  if (!types.length || types.some((entry) => entry === "Files" || entry.startsWith("image/"))) {
    ev.preventDefault();
    void pasteClipboard();
  }
}

function onPasteKey(ev: KeyboardEvent) {
  if (ev.repeat || ev.altKey || ev.shiftKey) return;
  if (!(ev.ctrlKey || ev.metaKey) || ev.key.toLowerCase() !== "v") return;
  if (editingText(ev.target)) return;
  pasteSeen = false;
  window.setTimeout(() => {
    if (pasteSeen) return;
    void pasteClipboard();
  }, 60);
}

function hasImageDrag(ev: DragEvent) {
  const types = [...(ev.dataTransfer?.types ?? [])];
  return types.includes("Files") || types.includes("text/uri-list") || types.includes("text/html") || types.includes("text/plain");
}

let dragDepth = 0;
let tauriDropHandled = false;

function waitForTauriDrop(ms = 280) {
  return new Promise<void>((resolve) => {
    if (tauriDropHandled) {
      resolve();
      return;
    }
    const started = Date.now();
    const timer = window.setInterval(() => {
      if (tauriDropHandled || Date.now() - started >= ms) {
        window.clearInterval(timer);
        resolve();
      }
    }, 20);
  });
}

function onWinDragEnter(ev: DragEvent) {
  if (!hasImageDrag(ev)) return;
  ev.preventDefault();
  dragDepth += 1;
}

function onWinDragOver(ev: DragEvent) {
  if (!hasImageDrag(ev)) return;
  ev.preventDefault();
  if (ev.dataTransfer) ev.dataTransfer.dropEffect = "copy";
}

function onWinDragLeave(ev: DragEvent) {
  if (!hasImageDrag(ev)) return;
  dragDepth = Math.max(0, dragDepth - 1);
}

function onWinDrop(ev: DragEvent) {
  ev.preventDefault();
  ev.stopPropagation();
  dragDepth = 0;
  const types = [...(ev.dataTransfer?.types ?? [])];
  const nativeFiles = types.includes("Files");
  const sources = dropSourcesFromTransfer(ev.dataTransfer);
  void (async () => {
    if (nativeFiles) {
      await waitForTauriDrop();
      if (tauriDropHandled) return;
    }
    await addFromSources(sources.urls, sources.paths);
  })();
}

let unlistenTauri: (() => void) | undefined;
onMounted(() => {
  window.addEventListener("paste", onPaste);
  window.addEventListener("keydown", onPasteKey);
  window.addEventListener("dragenter", onWinDragEnter, true);
  window.addEventListener("dragover", onWinDragOver, true);
  window.addEventListener("dragleave", onWinDragLeave, true);
  window.addEventListener("drop", onWinDrop, true);
  void getCurrentWebview()
    .onDragDropEvent((event) => {
      const kind = event.payload.type;
      if (kind !== "drop") return;
      const raws = (event.payload.paths ?? [])
        .map((item) => item.trim())
        .filter((item) => item && (isHttpUrl(item) || isLocalPath(item) || /^file:/i.test(item)));
      if (!raws.length) return;
      tauriDropHandled = true;
      window.setTimeout(() => {
        tauriDropHandled = false;
      }, 800);
      const urls: string[] = [];
      const paths: string[] = [];
      for (const raw of raws) {
        const value = /^file:/i.test(raw) ? fileUrlToPath(raw) || raw : raw;
        if (isHttpUrl(value)) urls.push(value);
        else paths.push(value);
      }
      void addFromSources(urls, paths);
    })
    .then((fn) => {
      unlistenTauri = fn;
    })
    .catch(() => {
      /* browser preview */
    });
});

onUnmounted(() => {
  window.removeEventListener("paste", onPaste);
  window.removeEventListener("keydown", onPasteKey);
  window.removeEventListener("dragenter", onWinDragEnter, true);
  window.removeEventListener("dragover", onWinDragOver, true);
  window.removeEventListener("dragleave", onWinDragLeave, true);
  window.removeEventListener("drop", onWinDrop, true);
  unlistenTauri?.();
});

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
      if (added) app.updatePreciseReference(added.id, { type: item.type, strength: 1, fidelity: 1 });
      app.status = `已发送到精准参考，类型是${preciseTypeLabel(item.type)}，强度 1`;
    }
    lib.touchPrecise(id);
  }
  void router.push("/");
}

const editingId = ref("");
const draftName = ref("");

function startRename(id: string, name: string) {
  editingId.value = id;
  draftName.value = name;
}

function commitRename(id: string) {
  if (editingId.value !== id) return;
  const name = draftName.value.trim();
  editingId.value = "";
  if (!name) return;
  if (kind.value === "vibes") lib.updateVibe(id, { name });
  else lib.updatePrecise(id, { name });
}

function setPreciseKind(id: string, group: PreciseReferenceGroup) {
  lib.updatePrecise(id, { type: preciseTypeForGroup(group) });
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
        <button type="button" :class="{ on: view === 'type' && typeFilter === 'costume' }" @click="pickType('costume')">服装</button>
        <p class="note">导入的图片默认按 {{ preciseTypeLabel(importType) }} 存。选中左边某个类型再导入，就按那个类型存。</p>
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
        <input ref="fileInput" hidden type="file" accept="image/png,image/jpeg,image/webp,image/gif,image/bmp" multiple @change="onPicked" />
      </header>
      <p v-if="status" class="warn bar-note">{{ status }}</p>
      <div class="stage">
        <div v-if="!shown.length" class="drop">
          <NaiIcon name="upload" :size="22" />
          <strong>{{ kind === "vibes" ? "把图片或 Vibe 文件拖到这里" : "把参考图拖到这里" }}</strong>
          <p v-if="kind === 'vibes'">支持 PNG（内含 Vibe）、.naiv4vibe、.naiv4vibebundle 和普通图片。普通图片先存原图，第一次用在生图时才编码。</p>
          <p v-else>角色设定图、风格样张都行，一次可以拖很多张。存进来时记住类型、强度和保真度，发送时直接套用。</p>
          <div class="acts">
            <button class="primary" type="button" @click="fileInput?.click()">选择文件</button>
            <button type="button" @click="pasteClipboard">从剪贴板粘贴</button>
          </div>
          <p class="hint">也可以直接把图片拖进来，或复制图片后按 Ctrl+V。</p>
        </div>
        <div v-else class="grid">
          <article v-for="item in shown" :key="item.id">
            <img :src="item.previewUrl" alt="" />
            <header>
              <input
                v-if="editingId === item.id"
                v-model="draftName"
                class="rename"
                maxlength="40"
                @blur="commitRename(item.id)"
                @keydown.enter="($event.target as HTMLInputElement).blur()"
              />
              <button v-else type="button" class="name" :title="`${item.name}，点击修改`" @click="startRename(item.id, item.name)">{{ item.name }}</button>
              <button type="button" :class="{ on: item.favorite }" @click="kind === 'vibes' ? lib.updateVibe(item.id, { favorite: !item.favorite }) : lib.updatePrecise(item.id, { favorite: !item.favorite })">
                <NaiIcon name="star" :size="14" />
              </button>
            </header>
            <small v-if="kind === 'vibes'">提取 {{ (item as any).infoExtracted.toFixed(2) }} · 强度 {{ (item as any).strength.toFixed(2) }}</small>
            <template v-else>
              <select :value="preciseTypeGroup((item as SavedPrecise).type)" @change="setPreciseKind(item.id, ($event.target as HTMLSelectElement).value as PreciseReferenceGroup)">
                <option v-for="opt in PRECISE_TYPE_OPTIONS" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
              </select>
              <small>强度 1</small>
            </template>
            <div class="acts">
              <button class="primary" type="button" @click="useItem(item.id)">{{ forI2i ? "用作图生图" : kind === "precise" ? "发送到精准参考" : "发到生图" }}</button>
              <button type="button" @click="kind === 'vibes' ? lib.removeVibe(item.id) : lib.removePrecise(item.id)"><NaiIcon name="trash" :size="14" /></button>
            </div>
          </article>
        </div>
        <aside class="help">
          <strong>Vibe 和精准参考有什么区别？</strong>
          <p>Vibe：借整体氛围和画风，可以叠很多张（V4 / V4.5，V5 没有）。精准参考：照着一张图的人物、画风或服装来画，更准，每张参考图 5 Anlas（V4.5 和 V5）。V5 没有保真滑条，强度到 5，角色和角色风格可以优先同一套服装。两者不能同时生效。</p>
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
.bar-note { margin: 0; padding: 8px 16px 0; font-size: 13px; }
.acts { display: flex; gap: 8px; flex-wrap: wrap; justify-content: center; }
.acts button, article button:not(.name) { height: 32px; padding: 0 10px; border-radius: 8px; border: 1px solid rgba(255,255,255,.12); background: rgba(255,255,255,.04); color: #fff; }
.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 12px; }
article { display: flex; flex-direction: column; gap: 8px; padding: 8px; border-radius: 12px; background: #1a1e38; border: 1px solid rgba(255,255,255,.08); }
article img { width: 100%; aspect-ratio: 1; object-fit: cover; border-radius: 8px; background: #10111c; }
article header { justify-content: space-between; min-width: 0; }
article header .name {
  flex: 1; min-width: 0; height: auto; padding: 0; border: 0; background: transparent; color: #fff;
  text-align: left; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
}
article header .rename { flex: 1; min-width: 0; height: 28px; padding: 0 8px; border-radius: 6px; border: 1px solid rgba(255,255,255,.16); background: rgba(0,0,0,.28); color: #fff; }
article header button { border: 0; background: transparent; color: rgba(255,255,255,.4); }
article header button.on { color: #f5c16c; }
article select { height: 32px; border-radius: 8px; border: 1px solid rgba(255,255,255,.12); background: #14172c; color: #fff; padding: 0 8px; }
.acts .primary { flex: 1; justify-content: center; white-space: nowrap; }
article small { color: rgba(255,255,255,.5); }
.help { padding: 12px 14px; border-radius: 12px; border: 1px solid rgba(255,255,255,.08); background: rgba(255,255,255,.03); }
</style>
