<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { storeToRefs } from "pinia";
import { useRouter } from "vue-router";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import {
  addCustomTag,
  fetchRemoteImage,
  lookupTags,
  readImageDataUrl,
  translateText,
  wdTagCancel,
  wdTagImage,
  type ReverseJob,
  type ReverseTag,
  type WdLabel,
} from "@/api/tauri";
import { useAppStore } from "@/stores/app";
import { useReverseStore } from "@/stores/reverse";
import { DANBOORU_CAT } from "@/utils/tagSuggest";
import { maxCharacterPrompts } from "@/types/nai";

const MAX_PARALLEL = 4;

const MODELS = [
  "SmilingWolf/wd-swinv2-tagger-v3",
  "SmilingWolf/wd-convnext-tagger-v3",
  "SmilingWolf/wd-vit-tagger-v3",
  "SmilingWolf/wd-vit-large-tagger-v3",
  "SmilingWolf/wd-eva02-large-tagger-v3",
  "SmilingWolf/wd-v1-4-moat-tagger-v2",
  "SmilingWolf/wd-v1-4-swinv2-tagger-v2",
  "SmilingWolf/wd-v1-4-convnext-tagger-v2",
  "SmilingWolf/wd-v1-4-convnextv2-tagger-v2",
  "SmilingWolf/wd-v1-4-vit-tagger-v2",
  "deepghs/idolsankaku-swinv2-tagger-v1",
  "deepghs/idolsankaku-eva02-large-tagger-v1",
  "cella110n/cl_tagger_v2",
];

const store = useAppStore();
const reverse = useReverseStore();
const router = useRouter();
const {
  jobs,
  activeId,
  currentTaskId,
  tasks,
  thumbs,
  model,
  generalThresh,
  generalMcut,
  characterThresh,
  characterMcut,
  hfToken,
  batching,
  active,
  pendingJobs,
  runningJobs,
  doneJobs,
  busy,
} = storeToRefs(reverse);
const dropping = ref(false);
const loadingMsg = ref("");
const applyOpen = ref(false);
const applyTarget = ref<"main" | "character">("main");
const applyMode = ref<"replace" | "append">("replace");
const applyCharId = ref("");
const addOpen = ref(false);
const addTagId = ref("");
const addName = ref("");
const addCn = ref("");
const addCategory = ref(0);
const addBusy = ref(false);
const addError = ref("");
let batchGen = 0;

const current = computed(() => store.previewUrl || store.i2iImage);
const batchPct = computed(() => {
  if (!jobs.value.length) return 0;
  const sum = jobs.value.reduce((acc, item) => acc + (item.status === "done" ? 1 : item.status === "running" ? item.progress : 0), 0);
  return Math.max(3, Math.round((sum / jobs.value.length) * 100));
});
const promptText = computed(() =>
  (active.value?.tags ?? [])
    .map((item) => item.label.replace(/_/g, " ").trim())
    .filter(Boolean)
    .join(", "),
);
const characterTags = computed(() => active.value?.tags.filter((item) => item.kind === "character") ?? []);
const generalTags = computed(() => active.value?.tags.filter((item) => item.kind === "general") ?? []);
const missingCharacters = computed(() => characterTags.value.filter((item) => item.found === false).length);
const missingGeneral = computed(() => generalTags.value.filter((item) => item.found === false).length);
const maxChars = computed(() => maxCharacterPrompts(store.params.model));
const canCharacters = computed(() => maxChars.value > 0);
const canAddCharacter = computed(() => store.characters.length < maxChars.value);

function uid() {
  return globalThis.crypto?.randomUUID?.() ?? `t-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

const isCl = computed(() => model.value.includes("cl_tagger"));
const localCl = computed(() => isCl.value && store.settings.localClTaggerEnabled);

function modelLabel(id: string) {
  return id.includes("cl_tagger") ? "CL Tagger v2" : shortName(id);
}

function shortName(id: string) {
  return id.split("/").pop() ?? id;
}

watch(hfToken, (next) => {
  localStorage.setItem("cl-hf-token", next.trim());
});

watch(model, (next, prev) => {
  const wasCl = (prev || "").includes("cl_tagger");
  const nowCl = next.includes("cl_tagger");
  if (!wasCl && nowCl) {
    if (generalThresh.value === 0.35) generalThresh.value = 0.55;
    if (characterThresh.value === 0.85) characterThresh.value = 0.6;
  } else if (wasCl && !nowCl) {
    if (generalThresh.value === 0.55) generalThresh.value = 0.35;
    if (characterThresh.value === 0.6) characterThresh.value = 0.85;
  }
});

function baseName(path: string) {
  return path.replace(/\\/g, "/").split("/").pop() || "图片";
}

function jobLabel(job: ReverseJob) {
  if (job.status === "running") return `${Math.round(job.progress * 100)}%`;
  if (job.status === "done") return "完成";
  if (job.status === "error") return "失败";
  if (job.status === "cancelled") return "取消";
  return "等待";
}

function toEdits(items: WdLabel[], kind: "character" | "general"): ReverseTag[] {
  return items.map((item) => ({
    id: uid(),
    label: item.label,
    confidence: item.confidence,
    kind,
    found: null,
    description: "",
    category: kind === "character" ? 4 : 0,
  }));
}

function addJob(image: string, name: string, path?: string) {
  if (path && jobs.value.some((item) => item.path === path)) return false;
  const job: ReverseJob = {
    id: uid(),
    name,
    image,
    path,
    status: "idle",
    progress: 0,
    progressMsg: "",
    error: "",
    tags: [],
    rating: [],
  };
  jobs.value = [...jobs.value, job];
  if (!activeId.value) activeId.value = job.id;
  reverse.schedulePersist();
  return true;
}

async function readBlob(file: File) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result || ""));
    reader.onerror = () => reject(new Error("读取图片失败"));
    reader.readAsDataURL(file);
  });
}

async function onPick(e: Event) {
  const files = [...((e.target as HTMLInputElement).files ?? [])].filter((file) => file.type.startsWith("image/"));
  (e.target as HTMLInputElement).value = "";
  if (!files.length) return;
  loadingMsg.value = `正在载入 0/${files.length}`;
  let added = 0;
  for (let i = 0; i < files.length; i += 1) {
    loadingMsg.value = `正在载入 ${i + 1}/${files.length}`;
    try {
      if (addJob(await readBlob(files[i]), files[i].name || `图片 ${jobs.value.length + 1}`)) added += 1;
    } catch {
      /* skip unreadable */
    }
  }
  loadingMsg.value = "";
  store.status = added ? `已加入 ${added} 张，队列共 ${jobs.value.length} 张` : "没有新的图片加入";
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

function safeTransferText(dt: DataTransfer | null | undefined, type: string) {
  try {
    return dt?.getData(type) || "";
  } catch {
    return "";
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

function dropSourcesFromTransfer(dt: DataTransfer | null | undefined) {
  const urls: string[] = [];
  const paths: string[] = [];
  if (!dt) return { urls, paths };
  const download = safeTransferText(dt, "DownloadURL");
  if (download) {
    const href = download.split(":").slice(2).join(":");
    if (isHttpUrl(href)) urls.push(href.trim());
  }
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
    if (src?.startsWith("data:image/") && src.length < 8_000_000) urls.push(src);
    else if (src && isHttpUrl(src)) urls.push(src);
    else if (src) {
      const path = pathFromTransferText(src);
      if (path) paths.push(path);
    }
  }
  return { urls: [...new Set(urls)], paths: [...new Set(paths)] };
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

async function addFromSources(urls: string[], paths: string[]) {
  const total = urls.length + paths.length;
  if (!total) {
    store.status = "拖入的不是图片。可从网页拖图片，或拖本地文件。";
    return;
  }
  loadingMsg.value = `正在载入 0/${total}`;
  let added = 0;
  let index = 0;
  for (const path of paths) {
    index += 1;
    loadingMsg.value = `正在载入 ${index}/${total}`;
    try {
      if (addJob(await readImageDataUrl(path), baseName(path), path)) added += 1;
    } catch (e) {
      store.status = e instanceof Error ? e.message : String(e);
    }
  }
  for (const url of urls) {
    index += 1;
    loadingMsg.value = `正在载入 ${index}/${total}`;
    try {
      const image = url.startsWith("data:image/") ? url : await fetchRemoteImage(url);
      if (addJob(image, baseName(url.split("?")[0]) || `图片 ${jobs.value.length + 1}`)) added += 1;
    } catch (e) {
      store.status = e instanceof Error ? e.message : String(e);
    }
  }
  loadingMsg.value = "";
  store.status = added ? `已加入 ${added} 张，队列共 ${jobs.value.length} 张` : "没有新的图片加入";
}

function onWinDragEnter(ev: DragEvent) {
  if (!hasImageDrag(ev)) return;
  ev.preventDefault();
  dragDepth += 1;
  dropping.value = true;
}

function onWinDragOver(ev: DragEvent) {
  if (!hasImageDrag(ev)) return;
  ev.preventDefault();
  if (ev.dataTransfer) ev.dataTransfer.dropEffect = "copy";
  dropping.value = true;
}

function onWinDragLeave(ev: DragEvent) {
  if (!hasImageDrag(ev)) return;
  dragDepth = Math.max(0, dragDepth - 1);
  if (dragDepth === 0) dropping.value = false;
}

function onWinDrop(ev: DragEvent) {
  ev.preventDefault();
  ev.stopPropagation();
  dragDepth = 0;
  dropping.value = false;
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

function isCancelError(e: unknown) {
  const msg = e instanceof Error ? e.message : String(e);
  return msg.includes("已取消");
}

async function cancelJob(id: string) {
  const job = jobs.value.find((item) => item.id === id);
  if (!job || job.status !== "running") return;
  try {
    await wdTagCancel(id);
  } catch {
    /* ignore */
  }
  reverse.patchJob(id, { status: "cancelled", progress: 0, progressMsg: "已取消反推", error: "" });
  reverse.schedulePersist();
}

async function cancelAll() {
  batchGen += 1;
  batching.value = false;
  const running = jobs.value.filter((item) => item.status === "running").map((item) => item.id);
  try {
    await wdTagCancel();
  } catch {
    /* ignore */
  }
  jobs.value = jobs.value.map((item) =>
    item.status === "running" ? { ...item, status: "cancelled", progress: 0, progressMsg: "已取消反推", error: "" } : item,
  );
  store.status = running.length ? `已取消 ${running.length} 张正在反推的图片` : "已停止批量反推";
  reverse.schedulePersist();
}

async function removeJob(id: string) {
  const job = jobs.value.find((item) => item.id === id);
  if (job?.status === "running") await cancelJob(id);
  jobs.value = jobs.value.filter((item) => item.id !== id);
  if (activeId.value === id) activeId.value = jobs.value[0]?.id || "";
  if (jobs.value.length) reverse.schedulePersist();
}

async function useCurrent() {
  if (!current.value) return;
  addJob(current.value, "当前生成图");
  store.status = `已加入当前生成图，队列共 ${jobs.value.length} 张`;
}

async function clearAll() {
  if (busy.value) await cancelAll();
  await reverse.newTask();
  loadingMsg.value = "";
  store.status = "已新建反推任务";
}

async function refreshLookups(jobId: string, tags: ReverseTag[], ids?: string[]) {
  const targets = ids ? tags.filter((item) => ids.includes(item.id)) : tags;
  if (!targets.length) return tags;
  try {
    const hits = await lookupTags(targets.map((item) => item.label));
    const byId = new Map(targets.map((item, index) => [item.id, hits[index]]));
    return tags.map((item) => {
      const hit = byId.get(item.id);
      if (!hit) return item;
      return {
        ...item,
        found: hit.found,
        description: hit.description,
        category: hit.found ? hit.category : item.category,
      };
    });
  } catch {
    return tags.map((item) => (ids && !ids.includes(item.id) ? item : { ...item, found: false }));
  }
}

async function runJob(jobId: string, gen: number) {
  const job = jobs.value.find((item) => item.id === jobId);
  if (!job || gen !== batchGen) return;
  reverse.patchJob(jobId, { status: "running", progress: 0.04, progressMsg: "正在准备图片…", error: "", tags: [], rating: [] });
  try {
    const next = await wdTagImage({
      imageBase64: job.image,
      model: model.value,
      generalThresh: generalThresh.value,
      generalMcut: generalMcut.value,
      characterThresh: characterThresh.value,
      characterMcut: characterMcut.value,
      jobId,
      hfToken: hfToken.value.trim(),
    });
    if (gen !== batchGen) return;
    const currentJob = jobs.value.find((item) => item.id === jobId);
    if (!currentJob || currentJob.status === "cancelled") return;
    let tags = [...toEdits(next.characters.confidences, "character"), ...toEdits(next.tags.confidences, "general")];
    reverse.patchJob(jobId, { progress: 0.96, progressMsg: "正在对照词库…", tags, rating: next.rating.confidences });
    tags = await refreshLookups(jobId, tags);
    if (gen !== batchGen) return;
    reverse.patchJob(jobId, { status: "done", progress: 1, progressMsg: "反推完成", tags, rating: next.rating.confidences, error: "" });
  } catch (e) {
    if (gen !== batchGen || isCancelError(e)) {
      const currentJob = jobs.value.find((item) => item.id === jobId);
      if (currentJob && currentJob.status === "running") {
        reverse.patchJob(jobId, { status: "cancelled", progress: 0, progressMsg: "已取消反推", error: "" });
      }
      return;
    }
    const message = e instanceof Error ? e.message : String(e);
    reverse.patchJob(jobId, { status: "error", progress: 0, progressMsg: message, error: message });
  }
}

async function submit() {
  const queue = pendingJobs.value.map((item) => item.id);
  if (!queue.length || batching.value) return;
  const gen = ++batchGen;
  batching.value = true;
  try {
    await reverse.beginRun();
  } catch {
    /* still run even if first save fails */
  }
  store.status = `开始批量反推 ${queue.length} 张，最多同时 ${Math.min(MAX_PARALLEL, queue.length)} 张`;
  const workers = Array.from({ length: Math.min(MAX_PARALLEL, queue.length) }, async () => {
    while (queue.length && gen === batchGen) {
      const id = queue.shift();
      if (!id) break;
      await runJob(id, gen);
    }
  });
  await Promise.all(workers);
  if (gen !== batchGen) return;
  batching.value = false;
  try {
    await reverse.finishRun();
  } catch {
    /* keep results in memory */
  }
  const failed = jobs.value.filter((item) => item.status === "error").length;
  store.status = failed ? `批量反推结束：完成 ${doneJobs.value.length}，失败 ${failed}` : `批量反推完成：${doneJobs.value.length}/${jobs.value.length}`;
}

async function retryFailed() {
  jobs.value = jobs.value.map((item) =>
    item.status === "error" || item.status === "cancelled" ? { ...item, status: "idle", error: "", progress: 0, progressMsg: "" } : item,
  );
  await submit();
}

let unlistenTauri: (() => void) | undefined;
onMounted(() => {
  void reverse.boot();
  window.addEventListener("dragenter", onWinDragEnter, true);
  window.addEventListener("dragover", onWinDragOver, true);
  window.addEventListener("dragleave", onWinDragLeave, true);
  window.addEventListener("drop", onWinDrop, true);
  void getCurrentWebview()
    .onDragDropEvent((event) => {
      const kind = event.payload.type;
      if (kind === "enter" || kind === "over") {
        dropping.value = true;
        return;
      }
      if (kind === "leave") {
        dropping.value = false;
        return;
      }
      if (kind === "drop") {
        dropping.value = false;
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
      }
    })
    .then((fn) => {
      unlistenTauri = fn;
    })
    .catch(() => {
      /* browser preview */
    });
});

onUnmounted(() => {
  window.removeEventListener("dragenter", onWinDragEnter, true);
  window.removeEventListener("dragover", onWinDragOver, true);
  window.removeEventListener("dragleave", onWinDragLeave, true);
  window.removeEventListener("drop", onWinDrop, true);
  unlistenTauri?.();
  if (jobs.value.length) reverse.schedulePersist();
});

function updateTag(id: string, label: string) {
  if (!active.value) return;
  reverse.patchJob(active.value.id, {
    tags: active.value.tags.map((item) => (item.id === id ? { ...item, label, found: null } : item)),
  });
  reverse.schedulePersist();
}

async function lookupOne(id: string) {
  if (!active.value) return;
  const tags = await refreshLookups(active.value.id, active.value.tags, [id]);
  reverse.patchJob(active.value.id, { tags });
  reverse.schedulePersist();
}

function removeTag(id: string) {
  if (!active.value) return;
  reverse.patchJob(active.value.id, { tags: active.value.tags.filter((item) => item.id !== id) });
  reverse.schedulePersist();
}

function joinPrompt(current: string, incoming: string, append: boolean) {
  const next = incoming.trim();
  if (!next) return current;
  if (!append || !current.trim()) return next;
  return `${current.replace(/,\s*$/, "")}, ${next}`;
}

function openApply() {
  if (!promptText.value) return;
  applyTarget.value = "main";
  applyMode.value = "replace";
  applyCharId.value = store.characters[0]?.id || "__new__";
  applyOpen.value = true;
}

function confirmApply() {
  const prompt = promptText.value;
  if (!prompt) return;
  if (applyTarget.value === "main") {
    store.params.positivePrompt = joinPrompt(store.params.positivePrompt, prompt, applyMode.value === "append");
    store.status = applyMode.value === "append" ? "已追加到主关键词" : "已写入主关键词";
  } else {
    if (!canCharacters.value) {
      store.status = "当前模型不支持角色提示词";
      return;
    }
    let id = applyCharId.value;
    let index = store.characters.findIndex((item) => item.id === id);
    if (id === "__new__" || index < 0) {
      if (!canAddCharacter.value) {
        store.status = `当前模型最多 ${maxChars.value} 个角色提示`;
        return;
      }
      store.addCharacter();
      const created = store.characters[store.characters.length - 1];
      if (!created) return;
      id = created.id;
      index = store.characters.length - 1;
    }
    const currentPrompt = store.characters.find((item) => item.id === id)?.prompt || "";
    store.updateCharacter(id, { prompt: joinPrompt(currentPrompt, prompt, applyMode.value === "append") });
    store.status = applyMode.value === "append" ? `已追加到角色 ${index + 1}` : `已写入角色 ${index + 1}`;
  }
  applyOpen.value = false;
  void router.push("/");
}

async function copyPrompt() {
  if (!promptText.value) return;
  await navigator.clipboard.writeText(promptText.value);
  store.status = "已复制反推提示词";
}

function openAdd(item: ReverseTag) {
  addTagId.value = item.id;
  addName.value = item.label.replace(/_/g, " ");
  addCn.value = item.description;
  addCategory.value = item.kind === "character" ? 4 : item.category || 0;
  addError.value = "";
  addOpen.value = true;
  if (!addCn.value) void autoTranslate();
}

async function autoTranslate() {
  const source = addName.value.replace(/_/g, " ").trim();
  if (!source || addBusy.value) return;
  addBusy.value = true;
  addError.value = "";
  try {
    addCn.value = await translateText(source, "en|zh-CN");
  } catch (e) {
    addError.value = e instanceof Error ? e.message : String(e);
  } finally {
    addBusy.value = false;
  }
}

async function saveCustom() {
  if (addBusy.value || !active.value) return;
  addBusy.value = true;
  addError.value = "";
  try {
    const hit = await addCustomTag(addName.value, addCn.value, addCategory.value);
    reverse.patchJob(active.value.id, {
      tags: active.value.tags.map((item) =>
        item.id === addTagId.value
          ? {
              ...item,
              label: addName.value,
              found: true,
              description: hit.description || addCn.value,
              category: hit.category,
            }
          : item,
      ),
    });
    reverse.schedulePersist();
    addOpen.value = false;
    store.status = `已加入词条库：${addName.value}`;
  } catch (e) {
    addError.value = e instanceof Error ? e.message : String(e);
  } finally {
    addBusy.value = false;
  }
}

function catLabel(id: number) {
  return DANBOORU_CAT[id]?.label ?? "通用";
}

function pct(n: number) {
  return `${Math.round(n * 100)}%`;
}

function formatTaskTime(iso: string) {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return `${d.getMonth() + 1}/${d.getDate()} ${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

async function openTask(id: string) {
  if (id === currentTaskId.value) return;
  try {
    await reverse.loadTask(id);
    store.status = "已还原反推任务";
  } catch (e) {
    store.status = e instanceof Error ? e.message : String(e);
  }
}

async function removeTask(id: string) {
  try {
    await reverse.deleteTask(id);
    store.status = "已删除反推任务";
  } catch (e) {
    store.status = e instanceof Error ? e.message : String(e);
  }
}
</script>

<template>
  <div class="page">
    <header class="head">
      <div>
        <h2>图片反推</h2>
        <p>一次可拖入多张。每次反推会存成任务，切走页面也能点回来。</p>
      </div>
    </header>

    <section class="tasks">
      <button class="btn" type="button" @click="clearAll">新建任务</button>
      <button
        v-for="item in tasks"
        :key="item.id"
        type="button"
        class="task"
        :class="{ on: item.id === currentTaskId }"
        @click="openTask(item.id)"
      >
        <img v-if="thumbs[item.id]" :src="thumbs[item.id]" alt="" />
        <span v-else class="ph" />
        <div>
          <strong>{{ item.name }}</strong>
          <small>{{ item.doneCount }}/{{ item.imageCount }} · {{ formatTaskTime(item.savedAt) }}</small>
        </div>
        <em @click.stop="removeTask(item.id)">×</em>
      </button>
    </section>

    <div class="grid">
      <section class="panel">
        <label class="drop" :class="{ on: dropping, has: jobs.length }">
          <span>{{ jobs.length ? `已加入 ${jobs.length} 张，再拖可继续添加` : "拖入或点击选择多张图片" }}</span>
          <input class="pick" type="file" accept="image/*" multiple @change="onPick" />
        </label>
        <div v-if="jobs.length" class="queue">
          <button
            v-for="job in jobs"
            :key="job.id"
            type="button"
            class="thumb"
            :class="[job.status, { on: job.id === active?.id }]"
            :title="job.name"
            @click="activeId = job.id"
          >
            <img :src="job.image" alt="" />
            <i>{{ jobLabel(job) }}</i>
            <b v-if="job.status === 'running'" :style="{ width: `${Math.max(6, Math.round(job.progress * 100))}%` }" />
            <em @click.stop="removeJob(job.id)">×</em>
          </button>
        </div>
        <div class="row">
          <button class="btn" type="button" :disabled="!current" @click="useCurrent">使用当前生成图</button>
          <button class="btn" type="button" :disabled="!jobs.length && !currentTaskId" @click="clearAll">清空并新建</button>
        </div>
        <label class="field">
          <span>模型</span>
          <select v-model="model">
            <option v-for="id in MODELS" :key="id" :value="id">{{ modelLabel(id) }}</option>
          </select>
        </label>
        <p v-if="isCl && localCl" class="hint">
          本地 CL Tagger v2 已启用，将使用本机模型（v2_00），不再走官方空间的免费 GPU。建议通用 0.55、角色 0.60。模型与 token 在设置 → 本地打标。
        </p>
        <p v-else-if="isCl" class="hint">
          CL Tagger v2 走官方空间的免费 GPU。建议通用 0.55、角色 0.60。匿名额度用完后，填下面的 token 再试。若本机有 NVIDIA 显卡，可在设置 → 本地打标改为离线推理。
        </p>
        <label v-if="isCl && !localCl" class="field">Hugging Face token
          <input v-model="hfToken" type="password" autocomplete="off" placeholder="hf_... 只保存在本机" />
        </label>
        <label class="slide">
          <span>{{ isCl ? "通用 / Meta 阈值" : "通用标签阈值" }} <b>{{ generalThresh.toFixed(2) }}</b></span>
          <input v-model.number="generalThresh" type="range" min="0" max="1" step="0.05" />
        </label>
        <label v-if="!isCl" class="check">
          <input v-model="generalMcut" type="checkbox" />
          通用标签使用 MCut
        </label>
        <label class="slide">
          <span>{{ isCl ? "角色 / 版权阈值" : "角色标签阈值" }} <b>{{ characterThresh.toFixed(2) }}</b></span>
          <input v-model.number="characterThresh" type="range" min="0" max="1" step="0.05" />
        </label>
        <label v-if="!isCl" class="check">
          <input v-model="characterMcut" type="checkbox" />
          角色标签使用 MCut
        </label>
        <div class="row">
          <button class="btn primary" type="button" :disabled="!pendingJobs.length || batching" @click="submit">
            {{ batching ? `反推中 ${runningJobs.length}/${MAX_PARALLEL}` : pendingJobs.length ? `开始批量反推（${pendingJobs.length}）` : "开始反推" }}
          </button>
          <button v-if="busy" class="btn danger" type="button" @click="cancelAll">全部取消</button>
          <button v-else-if="jobs.some((item) => item.status === 'error' || item.status === 'cancelled')" class="btn" type="button" @click="retryFailed">
            重试未完成
          </button>
        </div>
        <div v-if="busy || loadingMsg" class="progress" role="status">
          <div class="progress-head">
            <strong>{{ loadingMsg || active?.progressMsg || "批量反推中…" }}</strong>
            <span>{{ doneJobs.length }}/{{ jobs.length }} · {{ batchPct }}%</span>
          </div>
          <div class="track" role="progressbar" :aria-valuenow="batchPct" aria-valuemin="0" aria-valuemax="100">
            <i :style="{ width: `${batchPct}%` }" />
          </div>
          <p class="hint">最多同时 {{ MAX_PARALLEL }} 张。点缩略图可看单张进度，角标 × 可去掉或取消该张。</p>
        </div>
        <p v-if="active?.error" class="err">{{ active.error }}</p>
      </section>

      <section class="panel">
        <div v-if="active" class="picked">
          <img :src="active.image" alt="" />
          <div>
            <strong>{{ active.name }}</strong>
            <p>{{ active.progressMsg || jobLabel(active) }}</p>
            <button v-if="active.status === 'running'" class="tiny danger" type="button" @click="cancelJob(active.id)">取消这张</button>
          </div>
        </div>
        <label class="field">
          <span>输出提示词</span>
          <textarea :value="promptText" readonly rows="6" placeholder="选中一张完成后，反推结果会出现在这里" />
        </label>
        <div class="row">
          <button class="btn primary" type="button" :disabled="!promptText" @click="openApply">应用到生成</button>
          <button class="btn" type="button" :disabled="!promptText" @click="copyPrompt">复制</button>
        </div>
        <p v-if="active?.tags.length" class="hint">下面的标签可直接改、删。不在词库的可以补中文后加入。</p>

        <article v-if="active?.rating.length" class="group">
          <h3>分级 Rating</h3>
          <div v-for="item in active.rating" :key="item.label" class="bar">
            <span>{{ item.label }}</span>
            <i><b :style="{ width: pct(item.confidence) }" /></i>
            <em>{{ pct(item.confidence) }}</em>
          </div>
        </article>
        <article v-if="characterTags.length" class="group">
          <h3>角色 <small v-if="missingCharacters">{{ missingCharacters }} 条不在词库</small></h3>
          <div v-for="item in characterTags" :key="item.id" class="tag">
            <input
              :value="item.label"
              @input="updateTag(item.id, ($event.target as HTMLInputElement).value)"
              @change="lookupOne(item.id)"
            />
            <i><b :style="{ width: pct(item.confidence) }" /></i>
            <em>{{ pct(item.confidence) }}</em>
            <span class="meta" :class="{ miss: item.found === false }">
              {{ item.found ? item.description || catLabel(item.category) : item.found === false ? "不在词库" : "查询中…" }}
            </span>
            <button v-if="item.found === false" class="tiny" type="button" @click="openAdd(item)">加入</button>
            <button class="tiny danger" type="button" @click="removeTag(item.id)">删除</button>
          </div>
        </article>
        <article v-if="generalTags.length" class="group">
          <h3>通用标签 <small v-if="missingGeneral">{{ missingGeneral }} 条不在词库</small></h3>
          <div v-for="item in generalTags" :key="item.id" class="tag">
            <input
              :value="item.label"
              @input="updateTag(item.id, ($event.target as HTMLInputElement).value)"
              @change="lookupOne(item.id)"
            />
            <i><b :style="{ width: pct(item.confidence) }" /></i>
            <em>{{ pct(item.confidence) }}</em>
            <span class="meta" :class="{ miss: item.found === false }">
              {{ item.found ? item.description || catLabel(item.category) : item.found === false ? "不在词库" : "查询中…" }}
            </span>
            <button v-if="item.found === false" class="tiny" type="button" @click="openAdd(item)">加入</button>
            <button class="tiny danger" type="button" @click="removeTag(item.id)">删除</button>
          </div>
        </article>
      </section>
    </div>

    <div v-if="applyOpen" class="back" @click.self="applyOpen = false">
      <section class="dlg" role="dialog" aria-labelledby="apply-title">
        <h3 id="apply-title">应用到生成</h3>
        <p class="hint">将当前选中这张的 {{ active?.tags.length || 0 }} 个标签写入生成页。</p>
        <div class="choices">
          <label :class="{ on: applyTarget === 'main' }">
            <input v-model="applyTarget" type="radio" value="main" />
            <span>主关键词</span>
            <small>写入正面 Prompt</small>
          </label>
          <label :class="{ on: applyTarget === 'character', off: !canCharacters }">
            <input v-model="applyTarget" type="radio" value="character" :disabled="!canCharacters" />
            <span>角色提示词</span>
            <small>{{ canCharacters ? "写入 Character Prompts" : "当前模型不支持角色提示" }}</small>
          </label>
        </div>
        <label v-if="applyTarget === 'character' && canCharacters" class="field">
          <span>目标角色</span>
          <select v-model="applyCharId">
            <option v-for="(c, i) in store.characters" :key="c.id" :value="c.id">
              角色 {{ i + 1 }}{{ c.prompt.trim() ? `：${c.prompt.slice(0, 24)}` : "（空）" }}
            </option>
            <option value="__new__" :disabled="!canAddCharacter">新建角色</option>
          </select>
        </label>
        <div class="choices slim">
          <label :class="{ on: applyMode === 'replace' }">
            <input v-model="applyMode" type="radio" value="replace" />
            <span>覆盖</span>
          </label>
          <label :class="{ on: applyMode === 'append' }">
            <input v-model="applyMode" type="radio" value="append" />
            <span>追加</span>
          </label>
        </div>
        <div class="row">
          <button class="btn primary" type="button" @click="confirmApply">应用并前往生成</button>
          <button class="btn" type="button" @click="applyOpen = false">取消</button>
        </div>
      </section>
    </div>

    <div v-if="addOpen" class="back" @click.self="addOpen = false">
      <section class="dlg" role="dialog" aria-labelledby="add-title">
        <h3 id="add-title">加入词条库</h3>
        <p class="hint">补上中文后，这个标签会出现在提示词补全里。</p>
        <label class="field">
          <span>英文标签</span>
          <input v-model="addName" />
        </label>
        <label class="field">
          <span>中文翻译</span>
          <div class="row">
            <input v-model="addCn" placeholder="例如：长发" />
            <button class="btn" type="button" :disabled="addBusy" @click="autoTranslate">
              {{ addBusy ? "翻译中…" : "自动翻译" }}
            </button>
          </div>
        </label>
        <label class="field">
          <span>分类</span>
          <select v-model.number="addCategory">
            <option :value="0">通用</option>
            <option :value="4">角色</option>
            <option :value="3">作品</option>
            <option :value="1">画师</option>
            <option :value="5">元信息</option>
          </select>
        </label>
        <p v-if="addError" class="err">{{ addError }}</p>
        <div class="row">
          <button class="btn primary" type="button" :disabled="addBusy || !addName.trim() || !addCn.trim()" @click="saveCustom">
            保存到词库
          </button>
          <button class="btn" type="button" @click="addOpen = false">取消</button>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.page {
  height: 100%;
  overflow: auto;
  padding: 18px 22px 28px;
}
.head { margin-bottom: 14px; }
.head p { color: var(--muted); font-size: 13px; }
.tasks {
  display: flex;
  gap: 8px;
  overflow-x: auto;
  padding-bottom: 12px;
  margin-bottom: 4px;
  align-items: stretch;
}
.task {
  position: relative;
  display: grid;
  grid-template-columns: 44px minmax(96px, 140px);
  gap: 8px;
  align-items: center;
  min-width: 168px;
  padding: 6px 22px 6px 6px;
  border: 1px solid var(--bg3);
  border-radius: 10px;
  background: var(--bg1);
  text-align: left;
}
.task.on { border-color: var(--heading); }
.task img, .task .ph {
  width: 44px;
  height: 44px;
  border-radius: 6px;
  object-fit: cover;
  background: var(--bg0);
  display: block;
}
.task strong, .task small { display: block; }
.task strong { color: #fff; font-size: 12px; }
.task small { color: var(--muted); font-size: 11px; margin-top: 2px; }
.task em {
  position: absolute;
  top: 2px;
  right: 4px;
  font-style: normal;
  color: var(--muted);
  font-size: 14px;
}
.grid {
  display: grid;
  grid-template-columns: minmax(300px, 420px) minmax(0, 1fr);
  gap: 16px;
  align-items: start;
}
.panel {
  background: var(--bg1);
  border: 1px solid var(--bg3);
  border-radius: 12px;
  padding: 14px;
  display: grid;
  gap: 10px;
}
.drop {
  position: relative;
  min-height: 92px;
  display: grid;
  place-items: center;
  border: 1px dashed #2b2e4a;
  border-radius: 10px;
  background: var(--bg0);
  color: var(--muted);
  overflow: hidden;
  cursor: pointer;
  text-align: center;
  padding: 16px;
}
.drop.on, .drop.has { border-style: solid; }
.drop .pick {
  position: absolute;
  width: 0;
  height: 0;
  opacity: 0;
  pointer-events: none;
}
.queue {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(72px, 1fr));
  gap: 8px;
}
.thumb {
  position: relative;
  aspect-ratio: 1;
  padding: 0;
  border: 1px solid var(--bg3);
  border-radius: 8px;
  overflow: hidden;
  background: var(--bg0);
}
.thumb.on { border-color: var(--heading); }
.thumb.running { border-color: #60a5fa; }
.thumb.error { border-color: var(--danger); }
.thumb img { width: 100%; height: 100%; object-fit: cover; display: block; }
.thumb i, .thumb em {
  position: absolute;
  font-style: normal;
  font-size: 11px;
  line-height: 1;
}
.thumb i {
  left: 4px;
  bottom: 4px;
  padding: 3px 5px;
  border-radius: 4px;
  background: rgba(14, 15, 33, 0.78);
  color: #fff;
}
.thumb em {
  top: 2px;
  right: 2px;
  width: 18px;
  height: 18px;
  display: grid;
  place-items: center;
  border-radius: 99px;
  background: rgba(14, 15, 33, 0.78);
  color: #fff;
}
.thumb b {
  position: absolute;
  left: 0;
  bottom: 0;
  height: 3px;
  background: var(--heading);
}
.picked {
  display: grid;
  grid-template-columns: 72px 1fr;
  gap: 10px;
  align-items: center;
}
.picked img {
  width: 72px;
  height: 72px;
  object-fit: cover;
  border-radius: 8px;
  background: var(--bg0);
}
.picked strong { color: #fff; display: block; }
.picked p { margin: 4px 0 0; color: var(--muted); font-size: 12px; }
.row { display: flex; gap: 8px; flex-wrap: wrap; }
.slide, .field { display: grid; gap: 6px; color: #fff; font-size: 13px; }
.hint { margin: 0; color: rgba(255,255,255,0.5); font-size: 12px; line-height: 1.45; }
.slide b { color: var(--heading); }
.check { display: flex; align-items: center; gap: 8px; color: rgba(255,255,255,0.78); }
textarea { min-height: 120px; resize: vertical; }
.err { color: var(--danger); font-size: 13px; }
.progress {
  display: grid;
  gap: 8px;
  padding: 10px 12px;
  border: 1px solid #2b2e4a;
  border-radius: 10px;
  background: #1a1c34;
}
.progress-head {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  align-items: baseline;
  color: #fff;
  font-size: 13px;
}
.progress-head span { color: var(--heading); }
.progress .track {
  height: 6px;
  border-radius: 99px;
  background: #22253f;
  overflow: hidden;
}
.progress .track i {
  display: block;
  height: 100%;
  background: var(--heading);
  transition: width 0.16s ease-out;
}
.group { display: grid; gap: 6px; }
.group h3 { font-size: 14px; display: flex; align-items: baseline; gap: 8px; }
.group h3 small { color: var(--muted); font-size: 12px; font-weight: 500; }
.bar, .tag {
  display: grid;
  gap: 8px;
  align-items: center;
  color: #fff;
  font-size: 12px;
}
.bar { grid-template-columns: minmax(0, 1fr) minmax(80px, 1.4fr) 42px; }
.tag { grid-template-columns: minmax(120px, 1.2fr) minmax(70px, 1fr) 40px minmax(72px, 1fr) auto auto; }
.bar span, .meta { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.bar i, .tag i { height: 6px; border-radius: 99px; background: #22253f; overflow: hidden; }
.bar b, .tag b { display: block; height: 100%; background: var(--heading); }
.bar em, .tag em { font-style: normal; color: var(--muted); text-align: right; }
.tag input {
  width: 100%;
  background: var(--bg0);
  border: 1px solid var(--bg3);
  border-radius: 4px;
  padding: 4px 8px;
}
.meta { color: var(--muted); }
.meta.miss { color: #fbbf24; }
.tiny {
  background: transparent;
  border: 1px solid var(--bg3);
  border-radius: 4px;
  padding: 3px 8px;
  color: var(--heading);
  font-size: 12px;
}
.tiny.danger { color: var(--danger); border-color: #7f1d1d; }
.back {
  position: fixed;
  inset: 0;
  z-index: 50;
  background: rgba(14, 15, 33, 0.72);
  display: grid;
  place-items: center;
  padding: 24px;
}
.dlg {
  width: min(460px, 100%);
  background: #191b31;
  border: 1px solid #22253f;
  border-radius: 12px;
  padding: 20px;
  display: grid;
  gap: 12px;
}
.choices { display: grid; gap: 8px; }
.choices.slim { grid-template-columns: 1fr 1fr; }
.choices label {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 4px 10px;
  padding: 10px 12px;
  border: 1px solid var(--bg3);
  border-radius: 8px;
  background: var(--bg0);
}
.choices label.on { border-color: var(--heading); }
.choices label.off { opacity: 0.5; }
.choices small { grid-column: 2; color: var(--muted); font-size: 12px; }
@media (max-width: 880px) {
  .grid { grid-template-columns: 1fr; }
  .tag { grid-template-columns: minmax(0, 1fr) 56px auto auto; }
  .tag i, .tag em { display: none; }
}
</style>
