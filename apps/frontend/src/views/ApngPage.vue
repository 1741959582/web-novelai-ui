<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import {
  apngDisguise,
  apngGif,
  apngRestore,
  apngStrip,
  censorApply,
  censorSavePng,
  censorDownload,
  censorOpenDir,
  censorStatus,
  fileCleanedImages,
  copyNumberedImages,
  copyImageFiles,
  pickImageFolder,
  pickImages,
  pickOutputDir,
  type CensorEngineInfo,
  type CensorStatus,
  type SavedImage,
} from "@/api/tauri";
import { useApngStore, type ApngItem } from "@/stores/apng";
import { useAppStore } from "@/stores/app";
import NaiIcon from "@/components/NaiIcon.vue";

const store = useApngStore();
const app = useAppStore();
const busy = ref(false);
type QueueImage = SavedImage & { sourcePath?: string; sourceDataUrl?: string; id: string; censored?: boolean; block?: number };
const extra = ref<QueueImage[]>([]);
const metaActive = ref(0);
const metaPicked = ref<string[]>([]);
const QUEUE_DB = "nai-apng-queue";
const QUEUE_KEY = "session";
const TAB_KEY = "nai-apng-tab";
let queueReady = false;
let queueTimer = 0;

let queueDb: Promise<IDBDatabase> | null = null;
let queueDatabase: IDBDatabase | null = null;

function openQueueDb() {
  if (!queueDb) {
    queueDb = new Promise<IDBDatabase>((resolve, reject) => {
      const request = indexedDB.open(QUEUE_DB, 1);
      request.onupgradeneeded = () => {
        if (!request.result.objectStoreNames.contains("kv")) request.result.createObjectStore("kv");
      };
      request.onsuccess = () => {
        queueDatabase = request.result;
        resolve(request.result);
      };
      request.onerror = () => {
        queueDb = null;
        reject(request.error);
      };
    });
  }
  return queueDb;
}

function writeQueueNow(value: { tab: string; active: number; picked: string[]; items: QueueImage[] } | null) {
  if (!queueDatabase) {
    void writeQueue(value);
    return;
  }
  const store = queueDatabase.transaction("kv", "readwrite").objectStore("kv");
  if (value?.items.length) store.put(value, QUEUE_KEY);
  else store.delete(QUEUE_KEY);
}

function writeQueue(value: { tab: string; active: number; picked: string[]; items: QueueImage[] } | null) {
  return openQueueDb().then((db) => new Promise<void>((resolve, reject) => {
    const tx = db.transaction("kv", "readwrite");
    const store = tx.objectStore("kv");
    if (value?.items.length) store.put(value, QUEUE_KEY);
    else store.delete(QUEUE_KEY);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  }));
}

function readQueue() {
  return openQueueDb().then((db) => new Promise<{ tab: string; active: number; picked: string[]; items: QueueImage[] } | null>((resolve, reject) => {
    const request = db.transaction("kv", "readonly").objectStore("kv").get(QUEUE_KEY);
    request.onsuccess = () => resolve(request.result || null);
    request.onerror = () => reject(request.error);
  }));
}

function queueSnapshot() {
  return {
    tab: store.tab,
    active: metaActive.value,
    picked: [...metaPicked.value],
    items: extra.value.map((item) => ({
      id: item.id,
      path: item.path,
      sourcePath: item.sourcePath || item.path,
      dataUrl: item.dataUrl,
      sourceDataUrl: item.sourceDataUrl && item.sourceDataUrl !== item.dataUrl ? item.sourceDataUrl : "",
      censored: item.censored,
      block: item.block,
    })),
  };
}

function flushQueue() {
  if (!queueReady || !extra.value.length) return;
  localStorage.setItem(TAB_KEY, store.tab);
  window.clearTimeout(queueTimer);
  writeQueueNow(queueSnapshot());
}

function rememberQueue() {
  if (!queueReady || !extra.value.length) return;
  localStorage.setItem(TAB_KEY, store.tab);
  window.clearTimeout(queueTimer);
  queueTimer = window.setTimeout(() => {
    if (!extra.value.length) return;
    writeQueueNow(queueSnapshot());
  }, 0);
}

function isApngTab(tab: string | null | undefined): tab is "disguise" | "gif" | "restore" | "meta" | "mosaic" {
  return tab === "disguise" || tab === "gif" || tab === "restore" || tab === "meta" || tab === "mosaic";
}

async function restoreQueue() {
  let saved: { tab: string; active: number; picked: string[]; items: QueueImage[] } | null = null;
  try {
    saved = await readQueue();
  } catch {
    saved = null;
  }
  queueReady = true;
  if (saved?.items?.length && !extra.value.length) {
    extra.value = saved.items.map((item) => ({
      ...item,
      sourcePath: item.sourcePath || item.path,
      sourceDataUrl: item.sourceDataUrl || item.dataUrl,
    }));
    metaActive.value = Math.min(saved.active || 0, extra.value.length - 1);
    metaPicked.value = (saved.picked || []).filter((id) => extra.value.some((item) => item.id === id));
    if (isApngTab(saved.tab)) store.tab = saved.tab;
    app.status = `已恢复上次的 ${extra.value.length} 张，点清空才会丢掉`;
    return;
  }
  const savedTab = localStorage.getItem(TAB_KEY);
  if (isApngTab(savedTab)) store.tab = savedTab;
}

function clearExtra() {
  extra.value = [];
  metaActive.value = 0;
  metaPicked.value = [];
  window.clearTimeout(queueTimer);
  writeQueueNow(null);
}
const metaSorting = ref(false);
const dragFrom = ref(-1);
const dragOver = ref(-1);
let metaAnchor = 0;
let metaWasPicked = false;
let metaDownCtrl = false;
let metaDownShift = false;
const metaQueue = ref<HTMLElement | null>(null);
let metaPointer = -1;
let metaStartX = 0;
let metaStartY = 0;
let metaMoved = false;
const readyToFile = ref(false);
const saveDir = ref(localStorage.getItem("nai-clean-save-dir") || "");
const mosaicSaveDir = ref(localStorage.getItem("nai-mosaic-save-dir") || "");
const mosaicOutDir = ref(localStorage.getItem("nai-mosaic-out-dir") || "");
const saveDirLabel = computed(() => dirLabel(saveDir.value));
const mosaicSaveLabel = computed(() => dirLabel(mosaicSaveDir.value));
const mosaicOutLabel = computed(() => dirLabel(mosaicOutDir.value));

function dirLabel(path: string) {
  return path.split(/[/\\]/).filter(Boolean).pop() || path;
}
const coverPreview = ref("");
const frameIndex = ref(0);
const nameDraft = ref("");

const partOptions = [
  { id: "female_genital", label: "女性性器" },
  { id: "male_genital", label: "男性性器" },
  { id: "anus", label: "肛门" },
  { id: "xray", label: "透视" },
  { id: "nipple", label: "乳头" },
  { id: "female_genital_covered", label: "隔衣" },
  { id: "anus_covered", label: "肛门隔衣" },
  { id: "breast", label: "胸部" },
  { id: "buttocks", label: "臀部" },
  { id: "sex", label: "性交" },
];
const censorKey = "nai-censor-settings";
const censorEngines = ref<CensorEngineInfo[]>([]);
const censorParts = ref<string[]>(["female_genital", "male_genital", "anus", "xray"]);
const censorPrecise = ref(true);
const censorFace = ref(true);
const censorFaceMale = ref(false);
const censorShape = ref("fit");
const censorMode = ref("mosaic");
const censorDilate = ref(15);
const censorStrength = ref(200);
const censorMinBlock = ref(6);
const faceReady = ref(false);
const censorDir = ref("");
const enginePref = ref<Record<string, { on: boolean; conf: number }>>({});
let censorReady = false;

function defaultEngineOn(id: string) {
  return id === "deepghs_s" || id.includes("ntd11") || id.includes("anime_nsfw");
}

function loadCensorPrefs() {
  try {
    const saved = JSON.parse(localStorage.getItem(censorKey) || "");
    if (Array.isArray(saved.parts) && saved.parts.length) censorParts.value = saved.parts;
    if (typeof saved.precise === "boolean") censorPrecise.value = saved.precise;
    if (typeof saved.faceGuard === "boolean") censorFace.value = saved.faceGuard;
    if (typeof saved.faceGuardMale === "boolean") censorFaceMale.value = saved.faceGuardMale;
    if (saved.shape) censorShape.value = saved.shape;
    if (saved.mode) censorMode.value = saved.mode;
    if (saved.dilate) censorDilate.value = saved.dilate;
    if (saved.strength) censorStrength.value = saved.strength;
    if (saved.minBlock) censorMinBlock.value = saved.minBlock;
    if (saved.engines && typeof saved.engines === "object") enginePref.value = saved.engines;
  } catch {
    /* 第一次打开用和自动打码工具相同的默认值 */
  }
  censorReady = true;
}

function persistCensor() {
  if (!censorReady) return;
  localStorage.setItem(
    censorKey,
    JSON.stringify({
      engines: enginePref.value,
      parts: censorParts.value,
      precise: censorPrecise.value,
      faceGuard: censorFace.value,
      faceGuardMale: censorFaceMale.value,
      shape: censorShape.value,
      mode: censorMode.value,
      dilate: censorDilate.value,
      strength: censorStrength.value,
      minBlock: censorMinBlock.value,
    }),
  );
}

function applyCensorStatus(status: CensorStatus) {
  faceReady.value = status.faceReady;
  censorDir.value = status.dir;
  const next = { ...enginePref.value };
  for (const engine of status.engines) {
    if (!next[engine.id]) {
      next[engine.id] = {
        on: defaultEngineOn(engine.id),
        conf: defaultEngineOn(engine.id) ? 0.4 : engine.defaultConf,
      };
    }
  }
  enginePref.value = next;
  censorEngines.value = status.engines;
}

async function refreshCensor() {
  try {
    applyCensorStatus(await censorStatus());
  } catch (error) {
    app.status = error instanceof Error ? error.message : String(error);
  }
}

async function openCensorDir() {
  try {
    await censorOpenDir();
  } catch (error) {
    app.status = error instanceof Error ? error.message : String(error);
  }
}

async function downloadCensor(id: string) {
  await run(async () => {
    app.status = "正在下载检测模型…";
    applyCensorStatus(await censorDownload(id));
    app.status = "模型已下载";
  });
}

loadCensorPrefs();
watch(
  [censorParts, censorPrecise, censorFace, censorFaceMale, censorShape, censorMode, censorDilate, censorStrength, censorMinBlock, enginePref],
  persistCensor,
  { deep: true },
);
watch(
  () => store.tab,
  (tab) => {
    if (tab === "mosaic") void refreshCensor();
  },
  { immediate: true },
);

const tabs = [
  { id: "disguise", label: "APNG伪装" },
  { id: "gif", label: "合成GIF" },
  { id: "restore", label: "还原真图" },
  { id: "meta", label: "清除元数据" },
  { id: "mosaic", label: "打马赛克" },
] as const;

const selected = computed(() => store.items.find((item) => item.id === store.selectedId) || store.items[0] || null);
const previewFile = computed(() => {
  if (!extra.value.length) return null;
  if (store.tab === "meta" || store.tab === "mosaic") {
    return extra.value[Math.min(metaActive.value, extra.value.length - 1)] || extra.value[0];
  }
  return extra.value[0];
});

function numberedName(index: number, total: number) {
  return String(index + 1).padStart(String(Math.max(total, 1)).length, "0");
}

function fileTitle(path: string) {
  return path.split(/[/\\]/).pop() || path;
}

function byFileName<T extends { path: string }>(files: T[]) {
  return [...files].sort((a, b) => fileTitle(a.path).localeCompare(fileTitle(b.path), "zh", { numeric: true, sensitivity: "base" }));
}

function withId(file: SavedImage & { sourcePath?: string; sourceDataUrl?: string }): QueueImage {
  return {
    ...file,
    id: crypto.randomUUID(),
    sourcePath: file.sourcePath || file.path,
    sourceDataUrl: file.sourceDataUrl || file.dataUrl,
  };
}
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
  const files = byFileName(await pickImages());
  if (!files.length) return;
  if (target === "cover") await store.setCover(files[0].dataUrl);
  if (target === "reals") {
    for (const file of files) await store.addReal(file.dataUrl, file.path.split(/[/\\]/).pop() || "真图");
    app.status = `已加入 ${files.length} 张，提示词和隐写已清掉`;
  }
  if (target === "gif") {
    for (const file of files) await store.addGif(file.dataUrl, file.path.split(/[/\\]/).pop() || "帧");
  }
  if (target === "restore") extra.value = files.map((file) => withId(file));
  if (target === "meta" || target === "mosaic") {
    readyToFile.value = false;
    const incoming = await Promise.all(
      files.map(async (file) => withId({ ...file, sourcePath: file.path, dataUrl: target === "meta" ? await cleanPreview(file.dataUrl) : file.dataUrl })),
    );
    if (target === "mosaic") {
      extra.value = [...extra.value, ...incoming];
      metaActive.value = Math.max(extra.value.length - incoming.length, 0);
      metaPicked.value = [extra.value[metaActive.value].id];
      app.status = `已加入 ${incoming.length} 张，队列共 ${extra.value.length} 张`;
    } else {
      metaActive.value = 0;
      metaPicked.value = [];
      extra.value = incoming;
      app.status = `已加入 ${files.length} 张，拖动缩略图可以调整顺序`;
    }
  }
}

async function fromMosaicFolder() {
  const files = byFileName(await pickImageFolder());
  if (!files.length) return;
  const incoming = files.map((file) => withId({ ...file, sourcePath: file.path }));
  extra.value = [...extra.value, ...incoming];
  metaActive.value = Math.max(extra.value.length - incoming.length, 0);
  metaPicked.value = [extra.value[metaActive.value].id];
  app.status = incoming.length >= 200
    ? `已从文件夹加入 200 张，更多的请分批放入`
    : `已从文件夹加入 ${incoming.length} 张，队列共 ${extra.value.length} 张`;
}

async function chooseMosaicDir(kind: "save" | "out") {
  const picked = await pickOutputDir();
  if (!picked) return;
  if (kind === "save") {
    mosaicSaveDir.value = picked;
    localStorage.setItem("nai-mosaic-save-dir", picked);
    app.status = `保存目录：${picked}`;
  } else {
    mosaicOutDir.value = picked;
    localStorage.setItem("nai-mosaic-out-dir", picked);
    app.status = `输出目录：${picked}`;
  }
}

function hasDiskPath(path: string) {
  return /^[a-zA-Z]:[\\/]/.test(path) || path.startsWith("\\\\") || path.startsWith("/");
}

async function fileForExport(item: QueueImage, index: number, total: number) {
  if (hasDiskPath(item.path)) return item;
  app.status = `正在准备 ${index + 1}/${total}`;
  const saved = await censorSavePng(item.dataUrl);
  return { ...item, path: saved.path, dataUrl: saved.dataUrl || item.dataUrl };
}

async function exportMosaic(items: QueueImage[]) {
  if (!items.length) return "没有可保存的图片";
  const dirs = [...new Set([mosaicSaveDir.value.trim(), mosaicOutDir.value.trim()].filter(Boolean))];
  if (!dirs.length) return "";
  const ready: QueueImage[] = [];
  for (let i = 0; i < items.length; i += 1) ready.push(await fileForExport(items[i], i, items.length));
  const byId = new Map(ready.map((item) => [item.id, item]));
  extra.value = extra.value.map((item) => byId.get(item.id) || item);
  const payload = ready.map((item, index) => ({
    path: item.path,
    sourcePath: item.sourcePath || item.path,
    name: numberedName(index, ready.length),
  }));
  const notes: string[] = [];
  for (const dir of dirs) {
    const result = await copyNumberedImages(payload, dir);
    let moved = result.moved;
    const missing = result.paths.flatMap((path, index) => (path ? [] : [index]));
    if (missing.length) {
      const retry = [];
      for (const index of missing) {
        app.status = `正在准备 ${index + 1}/${ready.length}`;
        const saved = await censorSavePng(ready[index].dataUrl);
        ready[index] = { ...ready[index], path: saved.path, dataUrl: saved.dataUrl || ready[index].dataUrl };
        retry.push({
          path: saved.path,
          sourcePath: saved.path,
          name: numberedName(index, ready.length),
        });
      }
      const again = await copyNumberedImages(retry, dir);
      moved += again.moved;
      const next = new Map(ready.map((item) => [item.id, item]));
      extra.value = extra.value.map((item) => next.get(item.id) || item);
    }
    const label = dir === mosaicSaveDir.value.trim() ? "保存目录" : "输出目录";
    const missed = moved < ready.length ? `，${ready.length - moved} 张没写上` : "";
    notes.push(`${label} ${moved} 张${missed}`);
  }
  return notes.join("，");
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
  const list = Array.from(files).sort((a, b) => a.name.localeCompare(b.name, "zh", { numeric: true, sensitivity: "base" }));
  if (target === "restore") {
    extra.value = await Promise.all(list.map(async (file) => withId({ path: file.name, dataUrl: await readFile(file) })));
    return;
  }
  if (target === "meta" || target === "mosaic") {
    readyToFile.value = false;
    const incoming = await Promise.all(
      list.map(async (file) => {
        const dataUrl = await readFile(file);
        return withId({
          path: file.name,
          sourcePath: file.name,
          dataUrl: target === "meta" ? await cleanPreview(dataUrl) : dataUrl,
        });
      }),
    );
    extra.value = [...extra.value, ...incoming];
    if (target === "mosaic") {
      metaActive.value = Math.max(extra.value.length - incoming.length, 0);
      metaPicked.value = [extra.value[metaActive.value].id];
    }
    if (target === "meta") app.status = `已加入 ${incoming.length} 张，拖动缩略图可以调整顺序`;
    return;
  }
  for (const file of list) {
    const dataUrl = await readFile(file);
    if (target === "cover") await store.setCover(dataUrl);
    if (target === "reals") await store.addReal(dataUrl, file.name);
    if (target === "gif") await store.addGif(dataUrl, file.name);
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
    app.status = "已伪装并复制 APNG 文件，到聊天窗口 Ctrl+V 粘贴";
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
  if (!item) {
    app.status = "先把真图放进队列";
    return;
  }
  if (item.status !== "done" || !item.outPath) {
    await disguiseCurrent(true);
    return;
  }
  await run(() => copyImageFiles([item.outPath]));
  app.status = "已复制 APNG 文件，到聊天窗口 Ctrl+V 粘贴";
}

function onCopyApng(ev: Event) {
  ev.preventDefault();
  void copyCurrent();
}

function onCopyShortcut(ev: ClipboardEvent) {
  const target = ev.target as HTMLElement | null;
  if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable)) {
    return;
  }
  if (store.tab !== "disguise" || !selected.value) return;
  ev.preventDefault();
  void copyCurrent();
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
  extra.value = [withId(saved)];
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

function focusExtra(index: number) {
  if (index <= 0 || index >= extra.value.length) return;
  const next = [...extra.value];
  const [item] = next.splice(index, 1);
  extra.value = [item, ...next];
}

function moveMetaGroup(ids: string[], dropIndex: number) {
  const chosen = new Set(ids);
  const block = extra.value.filter((item) => chosen.has(item.id));
  if (!block.length) return;
  const rest = extra.value.filter((item) => !chosen.has(item.id));
  let insertAt = 0;
  const end = Math.min(Math.max(dropIndex, 0), extra.value.length);
  for (let i = 0; i < end; i += 1) {
    if (!chosen.has(extra.value[i].id)) insertAt += 1;
  }
  rest.splice(insertAt, 0, ...block);
  const activeId = extra.value[metaActive.value]?.id;
  extra.value = rest;
  const nextActive = rest.findIndex((item) => item.id === activeId);
  if (nextActive >= 0) metaActive.value = nextActive;
}

function metaIndexAt(clientX: number) {
  const queue = metaQueue.value;
  if (!queue) return dragFrom.value;
  const shots = [...queue.querySelectorAll<HTMLElement>(".shot")];
  if (!shots.length) return dragFrom.value;
  for (let i = 0; i < shots.length; i += 1) {
    const box = shots[i].getBoundingClientRect();
    const mid = box.left + box.width / 2;
    if (clientX < mid) return i;
  }
  return shots.length;
}

function queueCanSort() {
  return store.tab === "meta" || store.tab === "mosaic";
}

function onMetaPointerDown(index: number, ev: PointerEvent) {
  if (!queueCanSort() || ev.button !== 0) return;
  const item = extra.value[index];
  if (!item) return;
  metaDownCtrl = ev.ctrlKey || ev.metaKey;
  metaDownShift = ev.shiftKey;
  metaWasPicked = metaPicked.value.includes(item.id);
  if (metaDownShift) {
    const start = Math.min(metaAnchor, index);
    const end = Math.max(metaAnchor, index);
    metaPicked.value = extra.value.slice(start, end + 1).map((file) => file.id);
  } else if (metaDownCtrl) {
    if (!metaWasPicked) metaPicked.value = [...metaPicked.value, item.id];
  } else if (!metaWasPicked) {
    metaPicked.value = [item.id];
  }
  if (!metaDownShift) metaAnchor = index;
  metaActive.value = index;
  dragFrom.value = index;
  dragOver.value = index;
  metaPointer = ev.pointerId;
  metaStartX = ev.clientX;
  metaStartY = ev.clientY;
  metaMoved = false;
  (ev.currentTarget as HTMLElement).setPointerCapture(ev.pointerId);
}

function onMetaPointerMove(ev: PointerEvent) {
  if (metaPointer < 0 || ev.pointerId !== metaPointer || dragFrom.value < 0) return;
  if (!metaMoved && Math.hypot(ev.clientX - metaStartX, ev.clientY - metaStartY) < 6) return;
  metaMoved = true;
  metaSorting.value = true;
  const queue = metaQueue.value;
  if (queue) {
    const box = queue.getBoundingClientRect();
    if (ev.clientX < box.left + 48) queue.scrollLeft -= 18;
    if (ev.clientX > box.right - 48) queue.scrollLeft += 18;
  }
  dragOver.value = metaIndexAt(ev.clientX);
}

function onMetaPointerUp(ev: PointerEvent) {
  if (metaPointer < 0 || ev.pointerId !== metaPointer) return;
  const from = dragFrom.value;
  const index = dragOver.value;
  const item = extra.value[from];
  if (metaMoved && item) {
    const ids = metaPicked.value.includes(item.id) ? [...metaPicked.value] : [item.id];
    moveMetaGroup(ids, index);
  } else if (item) {
    if (metaDownCtrl && metaWasPicked) metaPicked.value = metaPicked.value.filter((id) => id !== item.id);
    else if (!metaDownCtrl && !metaDownShift) metaPicked.value = [item.id];
    metaActive.value = from;
  }
  dragFrom.value = -1;
  dragOver.value = -1;
  metaPointer = -1;
  metaSorting.value = false;
  metaMoved = false;
}

async function applyPending() {
  const batch = store.takePending();
  if (!batch.length) return;
  if (store.tab === "restore") {
    extra.value = batch.map((file) => withId(file));
    return;
  }
  if (store.tab === "meta" || store.tab === "mosaic") {
    readyToFile.value = false;
    metaActive.value = 0;
    metaPicked.value = [];
    extra.value = await Promise.all(
      batch.map(async (file) => withId({ ...file, sourcePath: file.path, dataUrl: await cleanPreview(file.dataUrl) })),
    );
  }
}

watch(() => store.pendingTick, () => {
  void applyPending();
}, { immediate: true });

async function restore() {
  if (!extra.value.length) {
    app.status = "先选一张伪装图，要原文件，不能是截图";
    return;
  }
  const outs: SavedImage[] = [];
  await run(async () => {
    for (const src of extra.value) outs.push(...(await apngRestore(src.dataUrl)));
  });
  extra.value = outs.map((file) => withId(file));
  app.status = outs[0] ? `已还原 ${outs.length} 张，第一张在 ${outs[0].path}` : "没有拆出隐藏帧";
}

async function strip() {
  if (!extra.value.length) return;
  const outs: QueueImage[] = [];
  await run(async () => {
    for (const [index, src] of extra.value.entries()) {
      const saved = await apngStrip(src.dataUrl, numberedName(index, extra.value.length));
      outs.push(withId({ ...saved, sourcePath: src.sourcePath || src.path }));
    }
  });
  extra.value = outs;
  readyToFile.value = outs.length > 0;
  const last = numberedName(Math.max(outs.length - 1, 0), outs.length);
  app.status = outs.length > 1
    ? `已清除 ${outs.length} 张，按当前顺序命名为 ${numberedName(0, outs.length)}.png 到 ${last}.png`
    : `已清除元数据：${outs[0]?.path || ""}`;
}

async function chooseSaveDir() {
  const picked = await pickOutputDir();
  if (!picked) return "";
  saveDir.value = picked;
  localStorage.setItem("nai-clean-save-dir", picked);
  app.status = `保存位置：${picked}`;
  return picked;
}

async function saveToFolders() {
  const queued = extra.value.filter((item) => item.path && item.sourcePath);
  if (!queued.length) {
    app.status = "没有可保存的图片";
    return;
  }
  const dest = saveDir.value || (await chooseSaveDir());
  if (!dest) {
    app.status = "先选择保存位置";
    return;
  }
  await run(async () => {
    const result = await fileCleanedImages(
      queued.map((item, index) => ({
        path: item.path,
        sourcePath: item.sourcePath || item.path,
        name: numberedName(index, queued.length),
      })),
      dest,
    );
    const next = [...extra.value];
    let cursor = 0;
    for (let i = 0; i < next.length; i += 1) {
      if (!next[i].path || !next[i].sourcePath) continue;
      next[i] = { ...next[i], path: result.paths[cursor] || next[i].path };
      cursor += 1;
    }
    extra.value = next;
    const last = numberedName(Math.max(queued.length - 1, 0), queued.length);
    app.status = result.moved
      ? `已把 ${result.moved} 张按顺序保存到 ${dest}，从 ${numberedName(0, queued.length)}.png 到 ${last}.png`
      : "这些图片已经在所选位置";
    if (result.skipped) app.status += `，${result.skipped} 张没保存`;
  });
}

async function mosaic() {
  if (!extra.value.length) return;
  reviewSaveToken += 1;
  reviewDirty = false;
  const chosen = censorEngines.value.filter((engine) => engine.kind !== "face" && enginePref.value[engine.id]?.on);
  if (!chosen.length) {
    app.status = "先勾选至少一个检测模型";
    return;
  }
  const missing = chosen.filter((engine) => !engine.present);
  if (missing.length) {
    app.status = `还没准备好：${missing.map((engine) => engine.title).join("、")}。到设置 → 打码模型里下载。`;
    return;
  }
  if (censorFace.value && !faceReady.value) {
    app.status = "面部保护需要先下载面部检测模型";
    return;
  }
  const outs: QueueImage[] = [];
  let hits = 0;
  let untouched = 0;
  let block = 0;
  let note = "";
  await run(async () => {
    for (const src of extra.value) {
      app.status = `正在打码 ${outs.length + 1}/${extra.value.length}`;
      const out = await censorApply({
        image: src.sourceDataUrl || src.dataUrl,
        engines: chosen.map((engine) => ({ id: engine.id, conf: enginePref.value[engine.id]?.conf || 0.4 })),
        parts: censorParts.value,
        precise: censorPrecise.value,
        faceGuard: censorFace.value,
        faceGuardMale: censorFaceMale.value,
        shape: censorShape.value,
        mode: censorMode.value,
        dilate: censorDilate.value,
        strength: censorStrength.value,
        minBlock: censorMinBlock.value,
      });
      hits += out.hits;
      block = out.block;
      const kept = { ...src, sourceDataUrl: src.sourceDataUrl || src.dataUrl, block: out.block || src.block };
      if (!out.hits) {
        untouched += 1;
        note = out.note || note;
        outs.push(kept);
      } else {
        outs.push({ ...kept, path: out.path, dataUrl: out.dataUrl, censored: true });
      }
    }
  });
  extra.value = outs;
  if (reviewOn.value) {
    reviewDirty = false;
    reviewIndex = -1;
    await nextTick();
    await loadReview();
  }
  const censoredCount = outs.length - untouched;
  const grain = block ? `，马赛克块 ${block}` : "";
  const missed = untouched && censoredCount ? `，${untouched} 张没有需要打码` : "";
  const base = censoredCount
    ? `已打码 ${censoredCount} 张，检出 ${hits} 处${grain}${missed}`
    : (note || "没有检出勾选的部位");
  app.status = `${base}。还没写入目录，可以人工审核补漏或擦除，再点保存到目录。`;
}

async function saveMosaicDirs() {
  if (reviewOn.value) await commitReview();
  if (!extra.value.length) return;
  if (!mosaicSaveDir.value.trim() && !mosaicOutDir.value.trim()) {
    app.status = "先选择保存目录或输出目录";
    return;
  }
  const plain = extra.value.filter((item) => !item.censored).length;
  await run(async () => {
    const saved = await exportMosaic(extra.value);
    if (!saved) app.status = "先选择保存目录或输出目录";
    else if (saved === "没有可保存的图片") app.status = "没有可保存的图片";
    else app.status = `已复制到${saved}${plain ? `，含 ${plain} 张没有打码的` : ""}，并清除了元数据`;
  });
}

const reviewOn = ref(false);
const brushMode = ref<"add" | "erase">("add");
const brushShape = ref<"round" | "square">("round");
const brushRadius = ref(36);
const reviewZoom = ref(1);
const reviewPan = ref({ x: 0, y: 0 });
const reviewBlock = ref(6);
const reviewCanvas = ref<HTMLCanvasElement | null>(null);
const brushCursor = ref({ show: false, x: 0, y: 0, d: 36, round: true });
let reviewIndex = -1;
let reviewToken = 0;
let reviewSaveToken = 0;
let reviewDirty = false;
let painting = false;
let panning = false;
let panOrigin = { x: 0, y: 0, px: 0, py: 0 };
let lastDab: { x: number; y: number } | null = null;
let originCanvas: HTMLCanvasElement | null = null;
let blurCanvas: HTMLCanvasElement | null = null;
let blurKey = "";
const blockColors = new Map<string, [number, number, number]>();
const reviewUndo: ImageData[] = [];

function mosaicBlock(width: number, height: number, saved?: number) {
  if (saved && saved > 0) return saved;
  const grain = Math.max(1, Math.round(Math.max(width, height) / Math.max(1, censorStrength.value)));
  return Math.max(grain, Math.max(1, censorMinBlock.value));
}

function loadHtmlImage(src: string) {
  return new Promise<HTMLImageElement>((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("图片读不出来"));
    img.src = src;
  });
}

function fitReviewCanvas() {
  const canvas = reviewCanvas.value;
  const view = canvas?.parentElement;
  if (!canvas || !view || !canvas.width) return;
  const maxW = Math.max(1, view.clientWidth - 16);
  const maxH = Math.max(1, view.clientHeight - 16);
  const scale = Math.min(maxW / canvas.width, maxH / canvas.height) * reviewZoom.value;
  canvas.style.width = `${Math.max(1, Math.floor(canvas.width * scale))}px`;
  canvas.style.height = `${Math.max(1, Math.floor(canvas.height * scale))}px`;
  canvas.style.transform = `translate(calc(-50% + ${reviewPan.value.x}px), calc(-50% + ${reviewPan.value.y}px))`;
}

async function loadReview() {
  const token = ++reviewToken;
  await commitReview();
  if (token !== reviewToken || !reviewOn.value) return;
  const index = Math.min(metaActive.value, extra.value.length - 1);
  const item = extra.value[index];
  const canvas = reviewCanvas.value;
  if (!item || !canvas) return;
  reviewIndex = index;
  reviewDirty = false;
  reviewZoom.value = 1;
  reviewPan.value = { x: 0, y: 0 };
  reviewUndo.length = 0;
  blockColors.clear();
  blurCanvas = null;
  blurKey = "";
  const source = await loadHtmlImage(item.sourceDataUrl || item.dataUrl);
  const current = item.dataUrl === (item.sourceDataUrl || item.dataUrl) ? source : await loadHtmlImage(item.dataUrl);
  if (token !== reviewToken) return;
  originCanvas = document.createElement("canvas");
  originCanvas.width = source.naturalWidth;
  originCanvas.height = source.naturalHeight;
  originCanvas.getContext("2d")?.drawImage(source, 0, 0);
  canvas.width = current.naturalWidth;
  canvas.height = current.naturalHeight;
  reviewBlock.value = mosaicBlock(canvas.width, canvas.height, item.block);
  canvas.getContext("2d", { willReadFrequently: true })?.drawImage(current, 0, 0);
  fitReviewCanvas();
}

async function commitReview() {
  const canvas = reviewCanvas.value;
  const index = reviewIndex;
  if (!reviewDirty || !canvas || index < 0) return;
  reviewDirty = false;
  const saveToken = reviewSaveToken;
  const dataUrl = canvas.toDataURL("image/png");
  const saved = await censorSavePng(dataUrl);
  if (saveToken !== reviewSaveToken) return;
  const current = extra.value[index];
  if (!current) return;
  extra.value = extra.value.map((item, i) =>
    i === index ? { ...item, path: saved.path, dataUrl: saved.dataUrl, censored: true, block: reviewBlock.value } : item,
  );
}

async function openReview() {
  if (!extra.value.length) return;
  reviewOn.value = true;
  const item = extra.value[Math.min(metaActive.value, extra.value.length - 1)];
  if (item) metaPicked.value = [item.id];
  await nextTick();
  await loadReview();
  app.status = "人工审核：滚轮放大，右键拖动画面。画笔补漏，橡皮去掉多盖的。审完点保存到目录。";
}

async function saveReviewed() {
  await commitReview();
  await saveMosaicDirs();
}

function rememberStroke() {
  const canvas = reviewCanvas.value;
  const ctx = canvas?.getContext("2d");
  if (!canvas || !ctx) return;
  reviewUndo.push(ctx.getImageData(0, 0, canvas.width, canvas.height));
  if (reviewUndo.length > 12) reviewUndo.shift();
}

function undoReview() {
  const canvas = reviewCanvas.value;
  const ctx = canvas?.getContext("2d");
  const prev = reviewUndo.pop();
  if (!canvas || !ctx || !prev) return;
  ctx.putImageData(prev, 0, 0);
  reviewDirty = true;
  void commitReview();
}

async function closeReview() {
  await commitReview();
  reviewOn.value = false;
  reviewIndex = -1;
  brushCursor.value = { ...brushCursor.value, show: false };
}

function stepReview(delta: number) {
  if (!extra.value.length) return;
  const next = Math.min(extra.value.length - 1, Math.max(0, metaActive.value + delta));
  if (next === metaActive.value) return;
  metaActive.value = next;
  const item = extra.value[next];
  if (item) metaPicked.value = [item.id];
}

function boxBlur(src: ImageData, radius: number) {
  const { width: w, height: h, data } = src;
  const tmp = new Uint8ClampedArray(data.length);
  const out = new Uint8ClampedArray(data.length);
  const rad = Math.max(1, radius);
  for (let y = 0; y < h; y += 1) {
    let r = 0;
    let g = 0;
    let b = 0;
    let n = 0;
    const init = Math.min(w - 1, rad);
    for (let x = 0; x <= init; x += 1) {
      const i = (y * w + x) * 4;
      r += data[i];
      g += data[i + 1];
      b += data[i + 2];
      n += 1;
    }
    for (let x = 0; x < w; x += 1) {
      const oi = (y * w + x) * 4;
      tmp[oi] = Math.floor(r / n);
      tmp[oi + 1] = Math.floor(g / n);
      tmp[oi + 2] = Math.floor(b / n);
      tmp[oi + 3] = 255;
      const remove = x - rad;
      const add = x + rad + 1;
      if (remove >= 0) {
        const i = (y * w + remove) * 4;
        r -= data[i];
        g -= data[i + 1];
        b -= data[i + 2];
        n -= 1;
      }
      if (add < w) {
        const i = (y * w + add) * 4;
        r += data[i];
        g += data[i + 1];
        b += data[i + 2];
        n += 1;
      }
    }
  }
  for (let x = 0; x < w; x += 1) {
    let r = 0;
    let g = 0;
    let b = 0;
    let n = 0;
    const init = Math.min(h - 1, rad);
    for (let y = 0; y <= init; y += 1) {
      const i = (y * w + x) * 4;
      r += tmp[i];
      g += tmp[i + 1];
      b += tmp[i + 2];
      n += 1;
    }
    for (let y = 0; y < h; y += 1) {
      const oi = (y * w + x) * 4;
      out[oi] = Math.floor(r / n);
      out[oi + 1] = Math.floor(g / n);
      out[oi + 2] = Math.floor(b / n);
      out[oi + 3] = 255;
      const remove = y - rad;
      const add = y + rad + 1;
      if (remove >= 0) {
        const i = (remove * w + x) * 4;
        r -= tmp[i];
        g -= tmp[i + 1];
        b -= tmp[i + 2];
        n -= 1;
      }
      if (add < h) {
        const i = (add * w + x) * 4;
        r += tmp[i];
        g += tmp[i + 1];
        b += tmp[i + 2];
        n += 1;
      }
    }
  }
  return new ImageData(out, w, h);
}

function ensureBlurCanvas() {
  const source = originCanvas;
  if (!source) return null;
  const radius = Math.max(1, reviewBlock.value);
  const key = `${source.width}x${source.height}:${radius}`;
  if (blurCanvas && blurKey === key) return blurCanvas;
  const src = source.getContext("2d", { willReadFrequently: true })?.getImageData(0, 0, source.width, source.height);
  if (!src) return null;
  const canvas = document.createElement("canvas");
  canvas.width = source.width;
  canvas.height = source.height;
  canvas.getContext("2d")?.putImageData(boxBlur(src, radius), 0, 0);
  blurCanvas = canvas;
  blurKey = key;
  return canvas;
}

function blockAverage(px: number, py: number) {
  const block = Math.max(1, reviewBlock.value);
  const bx = Math.floor(px / block);
  const by = Math.floor(py / block);
  const key = `${bx},${by}`;
  const cached = blockColors.get(key);
  if (cached) return cached;
  const source = originCanvas;
  if (!source) return [0, 0, 0] as [number, number, number];
  const x = bx * block;
  const y = by * block;
  const bw = Math.min(block, source.width - x);
  const bh = Math.min(block, source.height - y);
  const data = source.getContext("2d")?.getImageData(x, y, Math.max(1, bw), Math.max(1, bh)).data;
  if (!data) return [0, 0, 0] as [number, number, number];
  let r = 0;
  let g = 0;
  let b = 0;
  let n = 0;
  for (let i = 0; i < data.length; i += 4) {
    r += data[i];
    g += data[i + 1];
    b += data[i + 2];
    n += 1;
  }
  const color: [number, number, number] = [Math.round(r / n), Math.round(g / n), Math.round(b / n)];
  blockColors.set(key, color);
  return color;
}

function imagePoint(ev: PointerEvent) {
  const canvas = reviewCanvas.value;
  if (!canvas) return null;
  const rect = canvas.getBoundingClientRect();
  if (!rect.width || !rect.height) return null;
  return {
    x: ((ev.clientX - rect.left) / rect.width) * canvas.width,
    y: ((ev.clientY - rect.top) / rect.height) * canvas.height,
  };
}

function moveBrushCursor(ev: { clientX: number; clientY: number }) {
  const canvas = reviewCanvas.value;
  if (!canvas) return;
  const rect = canvas.getBoundingClientRect();
  const scale = rect.width / Math.max(1, canvas.width);
  brushCursor.value = {
    show: true,
    x: ev.clientX,
    y: ev.clientY,
    d: Math.max(8, brushRadius.value * 2 * scale),
    round: brushShape.value === "round",
  };
}

function onReviewWheel(ev: WheelEvent) {
  if (!reviewOn.value || store.tab !== "mosaic") return;
  ev.preventDefault();
  const factor = ev.deltaY < 0 ? 1.15 : 1 / 1.15;
  reviewZoom.value = Math.min(8, Math.max(1, reviewZoom.value * factor));
  if (reviewZoom.value <= 1.01) {
    reviewZoom.value = 1;
    reviewPan.value = { x: 0, y: 0 };
  }
  fitReviewCanvas();
  moveBrushCursor(ev);
}

function dab(cx: number, cy: number) {
  const canvas = reviewCanvas.value;
  const source = originCanvas;
  if (!canvas || !source) return;
  const radius = Math.max(1, brushRadius.value);
  const x0 = Math.max(0, Math.floor(cx - radius));
  const y0 = Math.max(0, Math.floor(cy - radius));
  const x1 = Math.min(canvas.width, Math.ceil(cx + radius));
  const y1 = Math.min(canvas.height, Math.ceil(cy + radius));
  const w = x1 - x0;
  const h = y1 - y0;
  if (w <= 0 || h <= 0) return;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  const orig = source.getContext("2d", { willReadFrequently: true });
  if (!ctx || !orig) return;
  const work = ctx.getImageData(x0, y0, w, h);
  const layer = brushMode.value === "add" && censorMode.value === "blur" ? ensureBlurCanvas() : null;
  const blurred = layer && layer.width === canvas.width && layer.height === canvas.height
    ? layer.getContext("2d")?.getImageData(x0, y0, w, h)
    : null;
  const base = brushMode.value === "erase" ? orig.getImageData(x0, y0, w, h) : blurred;
  const r2 = radius * radius;
  for (let y = 0; y < h; y += 1) {
    for (let x = 0; x < w; x += 1) {
      const dx = x0 + x + 0.5 - cx;
      const dy = y0 + y + 0.5 - cy;
      const inside = brushShape.value === "square" ? Math.abs(dx) <= radius && Math.abs(dy) <= radius : dx * dx + dy * dy <= r2;
      if (!inside) continue;
      const i = (y * w + x) * 4;
      if (base) {
        work.data[i] = base.data[i];
        work.data[i + 1] = base.data[i + 1];
        work.data[i + 2] = base.data[i + 2];
        work.data[i + 3] = 255;
      } else if (censorMode.value !== "blur") {
        const color = blockAverage(x0 + x, y0 + y);
        work.data[i] = color[0];
        work.data[i + 1] = color[1];
        work.data[i + 2] = color[2];
        work.data[i + 3] = 255;
      }
    }
  }
  ctx.putImageData(work, x0, y0);
  reviewDirty = true;
}

function strokeTo(x: number, y: number) {
  if (!lastDab) {
    dab(x, y);
    lastDab = { x, y };
    return;
  }
  const dx = x - lastDab.x;
  const dy = y - lastDab.y;
  const dist = Math.hypot(dx, dy);
  const step = Math.max(1, brushRadius.value / 4);
  const count = Math.max(1, Math.ceil(dist / step));
  for (let i = 1; i <= count; i += 1) dab(lastDab.x + (dx * i) / count, lastDab.y + (dy * i) / count);
  lastDab = { x, y };
}

function onBrushDown(ev: PointerEvent) {
  if (!reviewOn.value) return;
  (ev.currentTarget as HTMLElement).setPointerCapture(ev.pointerId);
  if (ev.button === 1 || ev.button === 2) {
    panning = true;
    panOrigin = { x: ev.clientX, y: ev.clientY, px: reviewPan.value.x, py: reviewPan.value.y };
    return;
  }
  if (ev.button !== 0) return;
  painting = true;
  lastDab = null;
  rememberStroke();
  moveBrushCursor(ev);
  const point = imagePoint(ev);
  if (point) strokeTo(point.x, point.y);
}

function onBrushMove(ev: PointerEvent) {
  if (!reviewOn.value) return;
  moveBrushCursor(ev);
  if (panning) {
    reviewPan.value = {
      x: panOrigin.px + ev.clientX - panOrigin.x,
      y: panOrigin.py + ev.clientY - panOrigin.y,
    };
    fitReviewCanvas();
    return;
  }
  if (!painting) return;
  const point = imagePoint(ev);
  if (point) strokeTo(point.x, point.y);
}

function onBrushUp(ev: PointerEvent) {
  if (panning) {
    panning = false;
    return;
  }
  if (!painting) return;
  painting = false;
  const point = imagePoint(ev);
  if (point) strokeTo(point.x, point.y);
  lastDab = null;
  void commitReview();
}

watch(metaActive, () => {
  if (reviewOn.value) void loadReview();
});
watch(() => store.tab, (tab) => {
  if (tab !== "mosaic" && reviewOn.value) void closeReview();
});
watch([extra, metaActive, metaPicked, () => store.tab], rememberQueue, { deep: true });

function onReviewKey(ev: KeyboardEvent) {
  if (!reviewOn.value || store.tab !== "mosaic") return;
  const target = ev.target as HTMLElement | null;
  if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.tagName === "SELECT")) return;
  if (ev.key === "ArrowLeft") {
    ev.preventDefault();
    stepReview(-1);
  } else if (ev.key === "ArrowRight") {
    ev.preventDefault();
    stepReview(1);
  } else if (ev.key === "b" || ev.key === "B") brushMode.value = "add";
  else if (ev.key === "e" || ev.key === "E") brushMode.value = "erase";
  else if (ev.key === "[") brushRadius.value = Math.max(4, brushRadius.value - 4);
  else if (ev.key === "]") brushRadius.value = Math.min(180, brushRadius.value + 4);
  else if ((ev.ctrlKey || ev.metaKey) && ev.key.toLowerCase() === "z" && !ev.shiftKey) {
    ev.preventDefault();
    undoReview();
  }
}

let timer = 0;
let lastTick = 0;
onMounted(() => {
  void restoreQueue();
  window.addEventListener("pagehide", flushQueue);
  window.addEventListener("paste", onPaste);
  window.addEventListener("copy", onCopyShortcut);
  window.addEventListener("keydown", onReviewKey);
  window.addEventListener("resize", fitReviewCanvas);
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
  flushQueue();
  window.removeEventListener("pagehide", flushQueue);
  window.removeEventListener("paste", onPaste);
  window.removeEventListener("copy", onCopyShortcut);
  window.removeEventListener("keydown", onReviewKey);
  window.removeEventListener("resize", fitReviewCanvas);
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
          <div class="view" @contextmenu="onCopyApng">
            <img v-if="coverPreview" :src="coverPreview" alt="" draggable="false" @dragstart.prevent />
            <p v-else>队列里选一张真图后，这里显示聊天缩略图。右键或「复制图片」会复制伪装 APNG 文件。</p>
          </div>
        </article>
        <article class="pane">
          <b>点开效果 · {{ selected && selected.frames.length > 1 ? `${selected.frames.length} 帧` : "真图" }}</b>
          <div class="view" @contextmenu="onCopyApng">
            <img v-if="realPreview" :src="realPreview" alt="" draggable="false" @dragstart.prevent />
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
        <label class="file">本地文件<input type="file" accept="image/*" multiple @change="fromFiles(($event.target as HTMLInputElement).files, store.tab)" /></label>
        <button v-if="store.tab === 'mosaic'" type="button" :disabled="busy" @click="fromMosaicFolder">选文件夹</button>
        <button v-if="store.tab === 'mosaic'" type="button" :disabled="!extra.length" @click="clearExtra">清空</button>
        <span class="spacer" />
        <button v-if="store.tab === 'restore'" type="button" class="primary" :disabled="busy" @click="restore">
          {{ extra.length > 1 ? `还原全部 ${extra.length}` : "还原当前" }}
        </button>
        <button v-else-if="store.tab === 'meta'" type="button" class="primary" :disabled="busy" @click="strip">
          {{ extra.length > 1 ? `清除全部 ${extra.length}` : "清除并保存" }}
        </button>
        <button v-if="store.tab === 'meta'" type="button" :disabled="busy" @click="chooseSaveDir">选择保存位置</button>
        <span v-if="store.tab === 'meta'" class="save-dir" :title="saveDir">{{ saveDir ? saveDirLabel : "未选择" }}</span>
        <button v-if="store.tab === 'meta'" type="button" :disabled="busy || !readyToFile" @click="saveToFolders">
          保存到此位置
        </button>
        <template v-if="store.tab === 'mosaic'">
          <button type="button" :disabled="busy" @click="chooseMosaicDir('save')">保存目录</button>
          <span class="save-dir" :title="mosaicSaveDir">{{ mosaicSaveDir ? mosaicSaveLabel : "未选择" }}</span>
          <button type="button" :disabled="busy" @click="chooseMosaicDir('out')">输出目录</button>
          <span class="save-dir" :title="mosaicOutDir">{{ mosaicOutDir ? mosaicOutLabel : "未选择" }}</span>
          <button type="button" :disabled="busy || !extra.length" @click="saveMosaicDirs">保存到目录</button>
          <button type="button" class="primary" :disabled="busy || !extra.length" @click="mosaic">
            {{ extra.length > 1 ? `打码全部 ${extra.length}` : "自动打码" }}
          </button>
          <button type="button" :class="{ on: reviewOn }" :disabled="!extra.length" @click="reviewOn ? closeReview() : openReview()">
            {{ reviewOn ? "退出审核" : "人工审核" }}
          </button>
        </template>
      </div>
      <div v-if="store.tab === 'mosaic'" class="censor">
        <span>部位</span>
        <label v-for="part in partOptions" :key="part.id" class="chk">
          <input v-model="censorParts" type="checkbox" :value="part.id" />{{ part.label }}
        </label>
        <span>模型</span>
        <template v-for="engine in censorEngines.filter((item) => item.kind !== 'face')" :key="engine.id">
          <label class="chk" :class="{ miss: !engine.present }">
            <input v-if="enginePref[engine.id]" v-model="enginePref[engine.id].on" type="checkbox" />{{ engine.title }}
          </label>
          <input v-if="enginePref[engine.id]" v-model.number="enginePref[engine.id].conf" type="number" min="0.05" max="0.95" step="0.05" title="置信度" />
          <button v-if="engine.builtin && !engine.present" type="button" :disabled="busy" @click="downloadCensor(engine.id)">下载</button>
        </template>
        <button v-if="!censorEngines.length" type="button" @click="refreshCensor">刷新模型</button>
        <label class="chk"><input v-model="censorPrecise" type="checkbox" />精确</label>
        <label class="chk"><input v-model="censorFace" type="checkbox" />面部保护</label>
        <label class="chk"><input v-model="censorFaceMale" type="checkbox" />男性也保护</label>
        <button v-if="!faceReady" type="button" :disabled="busy" @click="downloadCensor('anime_face')">下载面部模型</button>
        <label>形状
          <select v-model="censorShape">
            <option value="fit">贴合</option>
            <option value="ellipse">椭圆</option>
            <option value="rect">方框</option>
          </select>
        </label>
        <label>方式
          <select v-model="censorMode">
            <option value="mosaic">马赛克</option>
            <option value="blur">模糊</option>
          </select>
        </label>
        <label>膨胀 <input v-model.number="censorDilate" type="number" min="0" max="80" /></label>
        <label>强度 <input v-model.number="censorStrength" type="number" min="10" max="400" /></label>
        <label>最小块 <input v-model.number="censorMinBlock" type="number" min="1" max="64" /></label>
        <button type="button" @click="openCensorDir">模型目录</button>
        <button type="button" @click="refreshCensor">刷新</button>
        <span class="hint">贴合时分割模型按轮廓，框模型按颜色贴着物体，再按膨胀像素向外扩。模型在设置 → 打码模型里下载。</span>
      </div>
      <div v-if="store.tab === 'mosaic' && reviewOn" class="review-bar">
        <button type="button" :disabled="metaActive <= 0" @click="stepReview(-1)">上一张</button>
        <span>{{ metaActive + 1 }} / {{ extra.length }}</span>
        <button type="button" :disabled="metaActive >= extra.length - 1" @click="stepReview(1)">下一张</button>
        <button type="button" class="tool" :class="{ on: brushMode === 'add' }" title="画笔 B" @click="brushMode = 'add'">
          <NaiIcon name="brush" :size="16" />
        </button>
        <button type="button" class="tool" :class="{ on: brushMode === 'erase' }" title="橡皮 E" @click="brushMode = 'erase'">
          <NaiIcon name="eraser" :size="16" />
        </button>
        <button type="button" class="tool" :class="{ on: brushShape === 'round' }" title="圆形笔刷" @click="brushShape = 'round'">
          <i class="shape round" />
        </button>
        <button type="button" class="tool" :class="{ on: brushShape === 'square' }" title="方形笔刷" @click="brushShape = 'square'">
          <i class="shape square" />
        </button>
        <span>笔刷 {{ brushRadius }}</span>
        <input v-model.number="brushRadius" type="range" min="4" max="180" />
        <span>块 {{ reviewBlock }}</span>
        <span>{{ Math.round(reviewZoom * 100) }}%</span>
        <button type="button" @click="undoReview">撤销</button>
        <button type="button" class="primary" @click="saveReviewed">保存到目录</button>
      </div>
      <div class="stage" :class="{ single: store.tab !== 'restore' }">
        <article class="pane">
          <b>{{ reviewOn && store.tab === "mosaic" ? "人工审核 · 补漏或擦掉多打的" : store.tab === "restore" ? "伪装图 · 要原文件" : store.tab === "meta" ? "已去元数据的预览" : "当前图片" }}</b>
          <div class="view" @wheel.prevent="onReviewWheel" @contextmenu.prevent>
            <canvas
              v-show="reviewOn && store.tab === 'mosaic'"
              ref="reviewCanvas"
              class="review-canvas"
              @pointerdown="onBrushDown"
              @pointermove="onBrushMove"
              @pointerup="onBrushUp"
              @pointercancel="onBrushUp"
              @pointerleave="brushCursor.show = false"
            />
            <img v-if="previewFile && !(reviewOn && store.tab === 'mosaic')" :src="previewFile.dataUrl" alt="" />
            <p v-else-if="!(reviewOn && store.tab === 'mosaic')">{{ store.tab === "restore" ? "拖进来或 Ctrl+V 的必须是伪装 PNG 文件" : store.tab === "mosaic" ? "可以一次放进多张，或选整个文件夹。打码后先审核，再保存到目录。" : "Ctrl+V 可以贴图。生成图放进来会清掉提示词和透明通道隐写" }}</p>
          </div>
        </article>
        <article v-if="store.tab === 'restore' && extra.length > 1" class="pane">
          <b>还原后</b>
          <div class="view">
            <img :src="extra[extra.length - 1].dataUrl" alt="" />
          </div>
        </article>
      </div>
      <p v-if="(store.tab === 'meta' || store.tab === 'mosaic') && extra.length" class="path">单击选中当前图，按住 Ctrl 加选，按住 Shift 连选。拖已选的缩略图一起调整顺序。{{ store.tab === "mosaic" ? "没有需要打码的图片也会按这个顺序保存。" : `清除和保存按这个顺序命名，例如 ${numberedName(0, extra.length)}.png。` }}</p>
      <div v-if="store.tab === 'meta' || store.tab === 'mosaic' ? extra.length : extra.length > 1" ref="metaQueue" class="queue" :class="{ sorting: dragFrom >= 0 }">
        <button
          v-for="(file, i) in extra"
          :key="file.id"
          type="button"
          class="shot"
          :class="{ on: queueCanSort() ? (metaPicked.length ? metaPicked.includes(file.id) : i === metaActive) : i === 0, over: queueCanSort() && metaSorting && dragOver === i, dragging: queueCanSort() && metaSorting && metaPicked.includes(file.id) }"
          @click="queueCanSort() ? undefined : focusExtra(i)"
          @pointerdown="queueCanSort() && onMetaPointerDown(i, $event)"
          @pointermove="queueCanSort() && onMetaPointerMove($event)"
          @pointerup="queueCanSort() && onMetaPointerUp($event)"
          @pointercancel="queueCanSort() && onMetaPointerUp($event)"
        >
          <img :src="file.dataUrl" alt="" draggable="false" />
          <i>{{ queueCanSort() ? numberedName(i, extra.length) : i + 1 }}</i>
        </button>
      </div>
      <p v-if="previewFile" class="path">{{ previewFile.path }}</p>
    </template>
    <div
      v-if="brushCursor.show && reviewOn"
      class="brush-ring"
      :class="{ square: !brushCursor.round }"
      :style="{ left: `${brushCursor.x}px`, top: `${brushCursor.y}px`, width: `${brushCursor.d}px`, height: `${brushCursor.d}px` }"
    />
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
.save-dir {
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  color: #f5f3c2;
  font-size: 12px;
}
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
  min-width: 0;
  position: relative;
  overflow: hidden;
}
.review-bar {
  flex: none;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 12px 8px;
  overflow-x: auto;
  color: rgba(255,255,255,0.72);
  font-size: 12px;
}
.review-bar > * { flex: none; white-space: nowrap; }
.review-bar input[type="range"] { width: 120px; accent-color: var(--heading); }
.review-bar .tool { display: grid; place-items: center; width: 34px; height: 34px; padding: 0; }
.review-bar .shape { display: block; border: 2px solid currentColor; }
.review-bar .shape.round { width: 14px; height: 14px; border-radius: 50%; }
.review-bar .shape.square { width: 12px; height: 12px; }
.review-canvas {
  position: absolute;
  left: 50%;
  top: 50%;
  z-index: 2;
  transform: translate(-50%, -50%);
  cursor: none;
  touch-action: none;
}
.brush-ring {
  position: fixed;
  border: 1.5px solid #fff;
  border-radius: 50%;
  pointer-events: none;
  transform: translate(-50%, -50%);
  box-shadow: 0 0 0 1px rgba(0,0,0,0.85);
  z-index: 30;
}
.brush-ring.square { border-radius: 2px; }
.view img {
  position: absolute;
  left: 8px;
  top: 8px;
  width: calc(100% - 16px);
  height: calc(100% - 16px);
  object-fit: contain;
  object-position: center;
  -webkit-user-drag: none;
  user-select: none;
}
.view p {
  position: absolute;
  inset: 0;
  display: grid;
  place-items: center;
  padding: 16px;
  pointer-events: none;
  z-index: 1;
}
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
.shot.over { outline: 2px dashed #f5f3c2; }
.shot.dragging { opacity: 0.45; }
.queue.sorting { cursor: grabbing; }
.queue.sorting .shot { cursor: grabbing; touch-action: none; }
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
.censor {
  flex: none;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 6px 8px;
  padding: 0 12px 8px;
  color: rgba(255,255,255,0.72);
  font-size: 12px;
}
.censor .chk { background: transparent; padding: 2px 4px; }
.censor .miss { color: #e8b4b4; }
.censor .hint { flex-basis: 100%; color: rgba(255,255,255,0.45); }
.censor select {
  margin-left: 6px;
  background: #16182d;
  color: #fff;
  border: 0;
  border-radius: 6px;
  padding: 4px 6px;
}
</style>
