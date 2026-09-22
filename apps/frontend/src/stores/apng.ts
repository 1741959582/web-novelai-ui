import { acceptHMRUpdate, defineStore } from "pinia";
import { ref } from "vue";
import { apngClean } from "@/api/tauri";

export interface ApngShot {
  id: string;
  name: string;
  dataUrl: string;
}

export interface ApngItem {
  id: string;
  name: string;
  dataUrl: string;
  frames: string[];
  delayMs: number;
  status: "pending" | "done" | "error";
  outPath: string;
  outUrl: string;
  error: string;
}

export type ApngTab = "disguise" | "gif" | "restore" | "meta" | "mosaic";
export type CoverMode = "default" | "custom";

const MAX_QUEUE = 150;

async function cleanUrl(dataUrl: string) {
  try {
    return await apngClean(dataUrl);
  } catch {
    return dataUrl;
  }
}

function itemFrom(name: string, frames: string[], delayMs: number): ApngItem {
  const stem = name.replace(/\.[^.]+$/, "").trim() || "image";
  return {
    id: crypto.randomUUID(),
    name: stem,
    dataUrl: frames[0],
    frames,
    delayMs,
    status: "pending",
    outPath: "",
    outUrl: "",
    error: "",
  };
}

export const useApngStore = defineStore("apng", () => {
  const tab = ref<ApngTab>("disguise");
  const coverMode = ref<CoverMode>("default");
  const coverTitle = ref("点击查看原图 ~");
  const coverSubtitle = ref("");
  const customCover = ref("");
  const items = ref<ApngItem[]>([]);
  const selectedId = ref("");
  const gifFrames = ref<ApngShot[]>([]);
  const delayMs = ref(400);
  const padColor = ref("#ffffff");
  const fitFirst = ref(false);
  const mosaicBlock = ref(16);

  async function addReal(dataUrl: string, name = "生成图") {
    if (items.value.length >= MAX_QUEUE) return false;
    const cleaned = await cleanUrl(dataUrl);
    const item = itemFrom(name, [cleaned], delayMs.value);
    items.value = [...items.value, item];
    selectedId.value = item.id;
    tab.value = "disguise";
    return true;
  }

  async function addAnimation(frames: string[], name = "循环动画", delay = delayMs.value) {
    if (frames.length < 2 || items.value.length >= MAX_QUEUE) return false;
    const cleaned: string[] = [];
    for (const frame of frames) cleaned.push(await cleanUrl(frame));
    const item = itemFrom(name, cleaned, delay);
    items.value = [...items.value, item];
    selectedId.value = item.id;
    tab.value = "disguise";
    return true;
  }

  async function setCover(dataUrl: string) {
    customCover.value = await cleanUrl(dataUrl);
    coverMode.value = "custom";
    tab.value = "disguise";
    invalidate();
  }

  function invalidate() {
    items.value = items.value.map((item) =>
      item.status === "pending" && !item.outPath
        ? item
        : { ...item, status: "pending", outPath: "", outUrl: "", error: "" },
    );
  }

  async function addGif(dataUrl: string, name = "生成图") {
    if (gifFrames.value.length >= MAX_QUEUE) return false;
    const cleaned = await cleanUrl(dataUrl);
    gifFrames.value = [...gifFrames.value, { id: crypto.randomUUID(), name, dataUrl: cleaned }];
    return true;
  }

  function removeItem(id: string) {
    items.value = items.value.filter((item) => item.id !== id);
    if (selectedId.value === id) selectedId.value = items.value[0]?.id || "";
  }

  function moveItem(id: string, delta: number) {
    const i = items.value.findIndex((item) => item.id === id);
    const j = i + delta;
    if (i < 0 || j < 0 || j >= items.value.length) return;
    const next = [...items.value];
    [next[i], next[j]] = [next[j], next[i]];
    items.value = next;
  }

  function patchItem(id: string, patch: Partial<ApngItem>) {
    items.value = items.value.map((item) => (item.id === id ? { ...item, ...patch } : item));
  }

  function removeGif(id: string) {
    gifFrames.value = gifFrames.value.filter((item) => item.id !== id);
  }

  function moveGif(id: string, delta: number) {
    const i = gifFrames.value.findIndex((item) => item.id === id);
    const j = i + delta;
    if (i < 0 || j < 0 || j >= gifFrames.value.length) return;
    const next = [...gifFrames.value];
    [next[i], next[j]] = [next[j], next[i]];
    gifFrames.value = next;
  }

  return {
    tab,
    coverMode,
    coverTitle,
    coverSubtitle,
    customCover,
    items,
    selectedId,
    gifFrames,
    delayMs,
    padColor,
    fitFirst,
    mosaicBlock,
    addReal,
    addAnimation,
    setCover,
    invalidate,
    addGif,
    removeItem,
    moveItem,
    patchItem,
    removeGif,
    moveGif,
    clearItems: () => {
      items.value = [];
      selectedId.value = "";
    },
    clearGif: () => {
      gifFrames.value = [];
    },
  };
});

if (import.meta.hot) {
  import.meta.hot.accept(acceptHMRUpdate(useApngStore, import.meta.hot));
}
