<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import {
  apngDisguise,
  apngGif,
  apngMosaic,
  apngRestore,
  apngStrip,
  copyImageFiles,
  pickImages,
  type SavedImage,
} from "@/api/tauri";
import { useApngStore, type ApngItem } from "@/stores/apng";
import { useAppStore } from "@/stores/app";

const store = useApngStore();
const app = useAppStore();
const busy = ref(false);
const extra = ref<SavedImage[]>([]);
const coverPreview = ref("");
const frameIndex = ref(0);
const nameDraft = ref("");

const tabs = [
  { id: "disguise", label: "APNG伪装" },
  { id: "gif", label: "合成GIF" },
  { id: "restore", label: "还原真图" },
  { id: "meta", label: "清除元数据" },
  { id: "mosaic", label: "打马赛克" },
] as const;

const selected = computed(() => store.items.find((item) => item.id === store.selectedId) || store.items[0] || null);
const realPreview = computed(() => {
  const item = selected.value;
  if (!item) return "";
  return item.frames[frameIndex.value % Math.max(1, item.frames.length)] || item.dataUrl;
});

function imageSize(src: string) {
  return new Promise<{ w: number; h: number }>((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve({ w: img.naturalWidth || 800, h: img.naturalHeight || 800 });
    img.onerror = () => reject(new Error("读图失败"));
    img.src = src;
  });
}

function fitContain(src: string, width: number, height: number, pad: string) {
  return new Promise<string>((resolve, reject) => {
    const img = new Image();
    img.onload = () => {
      const canvas = document.createElement("canvas");
      canvas.width = Math.max(1, width);
      canvas.height = Math.max(1, height);
      const ctx = canvas.getContext("2d");
      if (!ctx) {
        resolve(src);
        return;
      }
      ctx.fillStyle = pad || "#ffffff";
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      const scale = Math.min(canvas.width / img.width, canvas.height / img.height);
      const nw = img.width * scale;
      const nh = img.height * scale;
      ctx.drawImage(img, (canvas.width - nw) / 2, (canvas.height - nh) / 2, nw, nh);
      resolve(canvas.toDataURL("image/png"));
    };
    img.onerror = () => reject(new Error("封面读取失败"));
    img.src = src;
  });
}

function drawDefaultCover(width: number, height: number, title: string, subtitle: string, pad: string) {
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, width);
  canvas.height = Math.max(1, height);
  const ctx = canvas.getContext("2d");
  if (!ctx) return "";
  ctx.fillStyle = pad || "#ffffff";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  const side = Math.max(1, Math.min(canvas.width, canvas.height));
  const x = (canvas.width - side) / 2;
  const y = (canvas.height - side) / 2;
  const gradient = ctx.createLinearGradient(0, y, 0, y + side);
  gradient.addColorStop(0, "rgb(120, 80, 220)");
  gradient.addColorStop(1, "rgb(240, 120, 190)");
  ctx.fillStyle = gradient;
  ctx.fillRect(x, y, side, side);
  const size = Math.max(24, Math.round(side / 14));
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.font = `700 ${size}px "Microsoft YaHei", "Segoe UI", sans-serif`;
  const cx = canvas.width / 2;
  const cy = canvas.height / 2;
  ctx.fillStyle = "rgba(0,0,0,0.45)";
  ctx.fillText(title, cx + 2, cy - size * 0.15 + 2);
  ctx.fillStyle = "#ffffff";
  ctx.fillText(title, cx, cy - size * 0.15);
  if (subtitle.trim()) {
    ctx.font = `600 ${Math.max(18, Math.round(size * 0.66))}px "Microsoft YaHei", "Segoe UI", sans-serif`;
    ctx.fillStyle = "rgb(255, 240, 180)";
    ctx.fillText(subtitle.trim(), cx, cy + size * 0.7);
  }
  return canvas.toDataURL("image/png");
}

async function refreshCover() {
  const item = selected.value;
  if (!item) {
    coverPreview.value = "";
    return;
  }
  if (store.coverMode === "custom" && store.customCover) {
    try {
      const size = await imageSize(item.dataUrl);
      coverPreview.value = await fitContain(store.customCover, size.w, size.h, store.padColor);
    } catch {
      coverPreview.value = store.customCover;
    }
    return;
  }
  try {
    const size = await imageSize(item.dataUrl);
    coverPreview.value = drawDefaultCover(
      size.w,
      size.h,
      store.coverTitle.trim() || "点击查看原图 ~",
      store.coverSubtitle,
      store.padColor,
    );
  } catch {
    coverPreview.value = "";
  }
}

watch(selected, (item) => {
  frameIndex.value = 0;
  nameDraft.value = item?.name || "";
});

watch(
  [selected, () => store.coverMode, () => store.coverTitle, () => store.coverSubtitle, () => store.customCover, () => store.padColor],
  () => {
    void refreshCover();
  },
  { immediate: true },
);

function readFile(file: File) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result || ""));
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}

async function run<T>(fn: () => Promise<T>) {
  busy.value = true;
  try {
    return await fn();
  } catch (e) {
    app.status = e instanceof Error ? e.message : String(e);
    throw e;
  } finally {
    busy.value = false;
  }
}

async function fromPicker(target: "cover" | "reals" | "gif" | "restore" | "meta" | "mosaic" | "disguise") {
  const files = await pickImages();
  if (!files.length) return;
  if (target === "cover") await store.setCover(files[0].dataUrl);
  if (target === "reals") {
    for (const file of files) await store.addReal(file.dataUrl, file.path.split(/[/\\]/).pop() || "真图");
    app.status = `已加入 ${files.length} 张，提示词和隐写已清掉`;
  }
  if (target === "gif") {
    for (const file of files) await store.addGif(file.dataUrl, file.path.split(/[/\\]/).pop() || "帧");
  }
  if (target === "restore") extra.value = files;
  if (target === "meta" || target === "mosaic") {
    const cleaned = await Promise.all(files.map(async (file) => ({ ...file, dataUrl: await cleanPreview(file.dataUrl) })));
    extra.value = cleaned;
  }
}

async function cleanPreview(dataUrl: string) {
  const { apngClean } = await import("@/api/tauri");
  try {
    return await apngClean(dataUrl);
  } catch {
    return dataUrl;
  }
}

async function fromFiles(files: FileList | null, target: "cover" | "reals" | "gif" | "restore" | "meta" | "mosaic" | "disguise") {
  if (!files?.length) return;
  const list = Array.from(files);
  if (target === "restore") {
    extra.value = await Promise.all(list.map(async (file) => ({ path: file.name, dataUrl: await readFile(file) })));
    return;
  }
  for (const file of list) {
    const dataUrl = await readFile(file);
    if (target === "cover") await store.setCover(dataUrl);
    if (target === "reals") await store.addReal(dataUrl, file.name);
    if (target === "gif") await store.addGif(dataUrl, file.name);
    if (target === "meta" || target === "mosaic") extra.value = [{ path: file.name, dataUrl: await cleanPreview(dataUrl) }];
  }
  if (target === "reals") app.status = `已加入 ${list.length} 张，提示词和隐写已清掉`;
}

async function onPaste(ev: ClipboardEvent) {
  const files = Array.from(ev.clipboardData?.files || []).filter((file) => file.type.startsWith("image/"));
  if (!files.length) return;
  ev.preventDefault();
  if (store.tab === "disguise") await fromFiles(asFileList(files), "reals");
  else if (store.tab === "gif") await fromFiles(asFileList(files), "gif");
  else if (store.tab === "restore") await fromFiles(asFileList(files), "restore");
  else if (store.tab === "meta") await fromFiles(asFileList(files), "meta");
  else if (store.tab === "mosaic") await fromFiles(asFileList(files), "mosaic");
}

function asFileList(files: File[]) {
  const transfer = new DataTransfer();
  files.forEach((file) => transfer.items.add(file));
  return transfer.files;
}

async function coverSource(item: ApngItem) {
  if (store.coverMode === "custom" && store.customCover) return store.customCover;
  if (coverPreview.value && selected.value?.id === item.id) return coverPreview.value;
  const size = await imageSize(item.dataUrl);
  return drawDefaultCover(size.w, size.h, store.coverTitle.trim() || "点击查看原图 ~", store.coverSubtitle, store.padColor);
}

async function disguiseOne(item: ApngItem) {
  const cover = await coverSource(item);
  if (!cover) throw new Error("封面还没准备好");
  const saved = await apngDisguise({
    cover,
    reals: item.frames,
    delayMs: item.frames.length > 1 ? item.delayMs : 100,
    padColor: store.padColor,
    name: item.name.endsWith(".png") ? item.name : `${item.name}.png`,
  });
  store.patchItem(item.id, { status: "done", outPath: saved.path, outUrl: saved.dataUrl, error: "" });
  return saved;
}

async function disguiseCurrent(copy = false) {
  const item = selected.value;
  if (!item) {
    app.status = "先把真图放进队列";
    return;
  }
  const saved = await run(() => disguiseOne(item));
  if (copy) {
    await copyImageFiles([saved.path]);
    app.status = "已伪装并复制文件，去聊天窗口粘贴的是原文件";
    return;
  }
  app.status = `已伪装：缩略图是封面，点开才是真图。${saved.path}`;
}

async function disguiseAll() {
  if (!store.items.length) return;
  let ok = 0;
  await run(async () => {
    for (const item of store.items) {
      try {
        await disguiseOne(item);
        ok += 1;
      } catch (e) {
        store.patchItem(item.id, { status: "error", error: e instanceof Error ? e.message : String(e) });
      }
    }
  });
  app.status = `全部伪装完成 ${ok}/${store.items.length}`;
}

async function copyCurrent() {
  const item = selected.value;
  if (!item) return;
  if (item.status !== "done" || !item.outPath) {
    await disguiseCurrent(true);
    return;
  }
  await run(() => copyImageFiles([item.outPath]));
  app.status = "已复制图片文件，粘贴出去的是文件本身";
}

function applyName() {
  const item = selected.value;
  const name = nameDraft.value.trim();
  if (!item || !name) return;
  store.patchItem(item.id, { name, status: "pending", outPath: "", outUrl: "" });
}

async function makeGif() {
  const saved = await run(() => apngGif({
    frames: store.gifFrames.map((item) => item.dataUrl),
    delayMs: store.delayMs,
    padColor: store.padColor,
    fitFirst: store.fitFirst,
  }));
  extra.value = [saved];
  app.status = `已合成 GIF：${saved.path}`;
}

async function sendGifToDisguise() {
  if (store.gifFrames.length < 2) {
    app.status = "至少 2 帧才能加入伪装";
    return;
  }
  const ok = await store.addAnimation(store.gifFrames.map((item) => item.dataUrl), "循环动画", store.delayMs);
  app.status = ok ? `已把 ${store.gifFrames.length} 帧放进伪装队列` : "伪装队列已满";
}

function sendQueueToGif() {
  const frames = store.items.flatMap((item) => item.frames);
  if (!frames.length) return;
  frames.forEach((url, i) => {
    store.gifFrames.push({ id: crypto.randomUUID(), name: `帧${i + 1}`, dataUrl: url });
  });
  store.tab = "gif";
  app.status = `已把队列里的 ${frames.length} 帧放进 GIF`;
}

async function restore() {
  const src = extra.value[0]?.dataUrl;
  if (!src) {
    app.status = "先选一张伪装图，要原文件，不能是截图";
    return;
  }
  const outs = await run(() => apngRestore(src));
  extra.value = outs;
  app.status = outs[0] ? `已还原到 ${outs[0].path}` : "没有拆出隐藏帧";
}

async function strip() {
  const src = extra.value[0]?.dataUrl;
  if (!src) return;
  const saved = await run(() => apngStrip(src));
  extra.value = [saved];
  app.status = `已清除元数据：${saved.path}`;
}

async function mosaic() {
  const src = extra.value[0]?.dataUrl;
  if (!src) return;
  const saved = await run(() => apngMosaic(src, store.mosaicBlock));
  extra.value = [saved];
  app.status = `已打马赛克：${saved.path}`;
}

let timer = 0;
let lastTick = 0;
onMounted(() => {
  window.addEventListener("paste", onPaste);
  timer = window.setInterval(() => {
    const item = selected.value;
    if (!item || item.frames.length < 2) return;
    const now = Date.now();
    if (now - lastTick < Math.max(50, item.delayMs)) return;
    lastTick = now;
    frameIndex.value = (frameIndex.value + 1) % item.frames.length;
  }, 40);
});
onUnmounted(() => {
  window.removeEventListener("paste", onPaste);
  window.clearInterval(timer);
});
</script>

<template>
  <div class="page">
    <div class="tabs">
      <button v-for="item in tabs" :key="item.id" type="button" :class="{ on: store.tab === item.id }" @click="store.tab = item.id">{{ item.label }}</button>
    </div>

    <template v-if="store.tab === 'disguise'">
      <div class="toolbar">
        <button type="button" :class="{ on: store.coverMode === 'default' }" @click="store.coverMode = 'default'; store.invalidate()">内置封面</button>
        <button type="button" :class="{ on: store.coverMode === 'custom' }" @click="store.coverMode = 'custom'">自定义封面</button>
        <input v-if="store.coverMode === 'default'" v-model="store.coverTitle" class="grow" placeholder="主标题" @change="store.invalidate()" />
        <input v-if="store.coverMode === 'default'" v-model="store.coverSubtitle" class="grow" placeholder="副标题，可空" @change="store.invalidate()" />
        <button v-else type="button" @click="fromPicker('cover')">选封面</button>
        <label v-if="store.coverMode === 'custom'" class="file">本地封面<input type="file" accept="image/*" @change="fromFiles(($event.target as HTMLInputElement).files, 'cover')" /></label>
        <label class="color">多余边 <input v-model="store.padColor" type="color" @change="store.invalidate()" /></label>
        <input v-model="nameDraft" class="name" placeholder="导出文件名" @change="applyName" />
        <button type="button" @click="fromPicker('reals')">加真图</button>
        <label class="file">本地真图<input type="file" accept="image/*" multiple @change="fromFiles(($event.target as HTMLInputElement).files, 'reals')" /></label>
        <span class="spacer" />
        <button type="button" class="primary" :disabled="busy" @click="disguiseCurrent(false)">伪装当前</button>
        <button type="button" class="primary" :disabled="busy" @click="copyCurrent">复制图片</button>
        <button type="button" :disabled="busy || !store.items.length" @click="disguiseAll">全部伪装</button>
      </div>

      <div class="stage">
        <article class="pane">
          <b>对方视角 · 不点开时</b>
          <div class="view">
            <img v-if="coverPreview" :src="coverPreview" alt="" />
            <p v-else>队列里选一张真图后，这里显示聊天缩略图</p>
          </div>
        </article>
        <article class="pane">
          <b>点开效果 · {{ selected && selected.frames.length > 1 ? `${selected.frames.length} 帧` : "真图" }}</b>
          <div class="view">
            <img v-if="realPreview" :src="realPreview" alt="" />
            <p v-else>Ctrl+V 或从生成页丢进来。进来就会清掉提示词和隐写</p>
          </div>
        </article>
      </div>

      <div class="queue-bar">
        <span>队列 {{ store.items.length }}/150</span>
        <button type="button" :disabled="!selected" @click="selected && store.moveItem(selected.id, -1)">上移</button>
        <button type="button" :disabled="!selected" @click="selected && store.moveItem(selected.id, 1)">下移</button>
        <button type="button" :disabled="!selected" @click="selected && store.removeItem(selected.id)">删除</button>
        <button type="button" :disabled="!store.items.length" @click="store.clearItems()">清空</button>
        <button type="button" :disabled="!store.items.length" @click="sendQueueToGif">队列合成 GIF</button>
        <em v-if="selected?.outPath">{{ selected.outPath }}</em>
        <em v-else-if="selected?.error">{{ selected.error }}</em>
      </div>
      <div class="queue">
        <button
          v-for="(item, i) in store.items"
          :key="item.id"
          type="button"
          class="shot"
          :class="[item.status, { on: selected?.id === item.id }]"
          @click="store.selectedId = item.id"
        >
          <img :src="item.dataUrl" alt="" />
          <i>{{ i + 1 }}{{ item.frames.length > 1 ? ` · ${item.frames.length}帧` : "" }}</i>
        </button>
        <p v-if="!store.items.length" class="empty">还没有真图</p>
      </div>
    </template>

    <template v-else-if="store.tab === 'gif'">
      <div class="toolbar">
        <button type="button" @click="fromPicker('gif')">加帧</button>
        <label class="file">本地帧<input type="file" accept="image/*" multiple @change="fromFiles(($event.target as HTMLInputElement).files, 'gif')" /></label>
        <label>间隔 <input v-model.number="store.delayMs" type="number" min="50" step="50" /> ms</label>
        <label class="color">多余边 <input v-model="store.padColor" type="color" /></label>
        <button type="button" :class="{ on: !store.fitFirst }" @click="store.fitFirst = false">按最大图</button>
        <button type="button" :class="{ on: store.fitFirst }" @click="store.fitFirst = true">按第一张</button>
        <span class="spacer" />
        <button type="button" class="primary" :disabled="busy" @click="makeGif">导出 GIF</button>
        <button type="button" @click="sendGifToDisguise">加入伪装</button>
        <button type="button" @click="store.clearGif()">清空</button>
      </div>
      <div class="stage single">
        <article class="pane">
          <b>循环预览 · {{ store.gifFrames.length }} 帧</b>
          <div class="view">
            <img v-if="store.gifFrames[0]" :src="store.gifFrames[0].dataUrl" alt="" />
            <p v-else>至少 2 张图。尺寸不同会等比放进同一画布，空边用多余边颜色</p>
          </div>
        </article>
      </div>
      <div class="queue">
        <div v-for="item in store.gifFrames" :key="item.id" class="shot static">
          <img :src="item.dataUrl" alt="" />
          <span>
            <button type="button" @click="store.moveGif(item.id, -1)">↑</button>
            <button type="button" @click="store.moveGif(item.id, 1)">↓</button>
            <button type="button" @click="store.removeGif(item.id)">删</button>
          </span>
        </div>
      </div>
    </template>

    <template v-else>
      <div class="toolbar">
        <button type="button" @click="fromPicker(store.tab)">选图片</button>
        <label class="file">本地文件<input type="file" accept="image/*" @change="fromFiles(($event.target as HTMLInputElement).files, store.tab)" /></label>
        <label v-if="store.tab === 'mosaic'">块 <input v-model.number="store.mosaicBlock" type="number" min="4" max="64" /></label>
        <span class="spacer" />
        <button v-if="store.tab === 'restore'" type="button" class="primary" :disabled="busy" @click="restore">还原当前</button>
        <button v-else-if="store.tab === 'meta'" type="button" class="primary" :disabled="busy" @click="strip">清除并保存</button>
        <button v-else type="button" class="primary" :disabled="busy" @click="mosaic">打马赛克</button>
      </div>
      <div class="stage" :class="{ single: store.tab !== 'restore' }">
        <article class="pane">
          <b>{{ store.tab === "restore" ? "伪装图 · 要原文件" : store.tab === "meta" ? "已去元数据的预览" : "当前图片" }}</b>
          <div class="view">
            <img v-if="extra[0]" :src="extra[0].dataUrl" alt="" />
            <p v-else>{{ store.tab === "restore" ? "拖进来或 Ctrl+V 的必须是伪装 PNG 文件" : "Ctrl+V 可以贴图。生成图放进来会清掉提示词和透明通道隐写" }}</p>
          </div>
        </article>
        <article v-if="store.tab === 'restore' && extra.length > 1" class="pane">
          <b>还原后</b>
          <div class="view">
            <img :src="extra[extra.length - 1].dataUrl" alt="" />
          </div>
        </article>
      </div>
      <p v-if="extra[0]" class="path">{{ extra[0].path }}</p>
    </template>
  </div>
</template>

<style scoped>
.page {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--bg0);
}
.tabs, .toolbar, .queue-bar, .queue {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: none;
}
.tabs { padding: 10px 12px 0; }
.toolbar, .queue-bar { padding: 8px 12px; overflow-x: auto; flex-wrap: nowrap; }
.toolbar > *, .queue-bar > * { flex: none; white-space: nowrap; }
button, .file, label {
  border: 0;
  border-radius: 8px;
  background: #2e3152;
  color: var(--heading);
  padding: 8px 10px;
}
button.on { background: #3d4270; color: #fff; }
.file { position: relative; overflow: hidden; }
.file input { position: absolute; inset: 0; opacity: 0; cursor: pointer; }
input[type="text"], input:not([type]), .grow, .name {
  background: #16182d;
  border: 0;
  border-radius: 8px;
  color: #fff;
  padding: 8px 10px;
}
.grow { width: 180px; }
.name { width: 160px; }
.color input, label input[type="number"] {
  margin-left: 6px;
  background: #16182d;
  border: 0;
  color: #fff;
  border-radius: 6px;
  width: 72px;
  padding: 4px 6px;
}
.color input { width: 36px; height: 24px; padding: 0; }
.primary { background: #f5f3c2; color: #0e0f21; font-weight: 700; }
.spacer { flex: 1 0 12px; }
.stage {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 10px;
  padding: 0 12px;
}
.stage.single { grid-template-columns: 1fr; }
.pane {
  min-width: 0;
  min-height: 0;
  display: flex;
  flex-direction: column;
  background: var(--bg1);
  border-radius: 12px;
  overflow: hidden;
}
.pane b {
  flex: none;
  padding: 8px 12px 0;
  color: rgba(255,255,255,0.55);
  font-size: 12px;
  font-weight: 600;
}
.view {
  flex: 1;
  min-height: 0;
  display: grid;
  place-items: center;
  padding: 8px;
}
.view img { max-width: 100%; max-height: 100%; object-fit: contain; }
.view p, .empty, .path {
  margin: 0;
  color: rgba(255,255,255,0.45);
  font-size: 13px;
  text-align: center;
}
.queue-bar { color: rgba(255,255,255,0.55); font-size: 12px; }
.queue-bar em {
  font-style: normal;
  max-width: 42vw;
  overflow: hidden;
  text-overflow: ellipsis;
}
.queue {
  height: 112px;
  padding: 0 12px 12px;
  overflow-x: auto;
  overflow-y: hidden;
}
.shot {
  width: 84px;
  height: 96px;
  padding: 0;
  position: relative;
  overflow: hidden;
  background: #16182d;
  flex: none;
}
.shot.on { outline: 2px solid var(--heading); }
.shot.done { outline-color: var(--ok); }
.shot.error { outline-color: var(--danger); }
.shot img { width: 100%; height: 100%; object-fit: cover; }
.shot i {
  position: absolute;
  left: 4px;
  top: 4px;
  font-style: normal;
  font-size: 11px;
  background: rgba(0,0,0,0.55);
  color: #fff;
  border-radius: 99px;
  padding: 1px 6px;
}
.shot.static { display: flex; flex-direction: column; }
.shot.static img { height: 68px; }
.shot.static span { display: flex; }
.shot.static button { flex: 1; border-radius: 0; padding: 2px 0; font-size: 11px; }
.path { padding: 0 12px 10px; text-align: left; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
