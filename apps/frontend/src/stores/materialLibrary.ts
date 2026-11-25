import { defineStore } from "pinia";
import { ref } from "vue";
import { libraryDeleteImage, librarySaveImage, readImageDataUrl } from "@/api/tauri";
import { normalizePreciseType, type PreciseReferenceType } from "@/types/nai";

export interface PromptBlock {
  id: string;
  name: string;
  positive: string;
  negative: string;
  favorite: boolean;
  folderId: string;
  usedAt: number;
  createdAt: number;
}

export interface BlockFolder {
  id: string;
  name: string;
}

export interface SavedVibe {
  id: string;
  name: string;
  previewUrl: string;
  infoExtracted: number;
  strength: number;
  favorite: boolean;
  categoryId: string;
  file?: string;
  usedAt: number;
  createdAt: number;
}

export interface VibeCategory {
  id: string;
  name: string;
}

export interface SavedPrecise {
  id: string;
  name: string;
  previewUrl: string;
  type: PreciseReferenceType;
  strength: number;
  fidelity: number;
  favorite: boolean;
  file?: string;
  usedAt: number;
  createdAt: number;
}

function read<T>(key: string): T[] {
  try {
    const raw = localStorage.getItem(key);
    const parsed = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function write(key: string, value: unknown) {
  localStorage.setItem(key, JSON.stringify(value));
}

function stamp() {
  return Date.now();
}

export const useMaterialLibrary = defineStore("materialLibrary", () => {
  const blocks = ref<PromptBlock[]>(
    read<Partial<PromptBlock>>("nai_prompt_blocks").map((item) => ({
      id: item.id || crypto.randomUUID(),
      name: item.name || "未命名提示词块",
      positive: item.positive || "",
      negative: item.negative || "",
      favorite: Boolean(item.favorite),
      folderId: item.folderId || "",
      usedAt: item.usedAt || 0,
      createdAt: item.createdAt || 0,
    })),
  );
  const folders = ref<BlockFolder[]>(read("nai_block_folders"));
  const vibes = ref<SavedVibe[]>(
    read<Partial<SavedVibe>>("nai_vibe_library").map((item) => ({
      id: item.id || crypto.randomUUID(),
      name: item.name || "Vibe",
      previewUrl: item.previewUrl || "",
      infoExtracted: item.infoExtracted ?? 1,
      strength: item.strength ?? 0.6,
      favorite: Boolean(item.favorite),
      categoryId: item.categoryId || "",
      file: item.file || "",
      usedAt: item.usedAt || 0,
      createdAt: item.createdAt || 0,
    })),
  );
  const categories = ref<VibeCategory[]>(read("nai_vibe_categories"));
  const precise = ref<SavedPrecise[]>(
    read<Partial<SavedPrecise>>("nai_precise_library").map((item) => ({
      id: item.id || crypto.randomUUID(),
      name: item.name || "精准参考",
      previewUrl: item.previewUrl || "",
      type: normalizePreciseType(item.type),
      strength: item.strength ?? 1,
      fidelity: item.fidelity ?? 1,
      favorite: Boolean(item.favorite),
      file: item.file || "",
      usedAt: item.usedAt || 0,
      createdAt: item.createdAt || 0,
    })),
  );
  const error = ref("");

  function pack<T extends { previewUrl: string; file?: string }>(item: T): T {
    if (!item.file || !item.previewUrl.startsWith("data:")) return item;
    return { ...item, previewUrl: "" };
  }

  function persist(key: string, value: unknown) {
    try {
      write(key, value);
      error.value = "";
      return true;
    } catch {
      error.value = "库记录没有写进去";
      return false;
    }
  }

  async function keepImage(id: string, previewUrl: string, file?: string) {
    if (file || !previewUrl.startsWith("data:")) return file || "";
    return librarySaveImage(id, previewUrl);
  }

  async function spill<T extends { id: string; previewUrl: string; file?: string }>(
    list: { value: T[] },
    key: string,
  ) {
    let changed = false;
    const next: T[] = [];
    for (const item of list.value) {
      if (item.previewUrl.startsWith("data:") && !item.file) {
        try {
          next.push({ ...item, file: await librarySaveImage(item.id, item.previewUrl) });
          changed = true;
          continue;
        } catch {
          next.push(item);
          continue;
        }
      }
      next.push(item);
    }
    if (!changed) return;
    list.value = next;
    persist(key, next.map(pack));
  }

  async function hydrate<T extends { id: string; previewUrl: string; file?: string }>(list: { value: T[] }) {
    for (const item of [...list.value]) {
      if (item.previewUrl || !item.file) continue;
      try {
        const previewUrl = await readImageDataUrl(item.file);
        list.value = list.value.map((entry) => (entry.id === item.id ? { ...entry, previewUrl } : entry));
      } catch {
        /* the file was removed */
      }
    }
  }

  void (async () => {
    await spill(vibes, "nai_vibe_library");
    await spill(precise, "nai_precise_library");
    await hydrate(vibes);
    await hydrate(precise);
  })();

  function addBlock(block: { name: string; positive: string; negative: string; folderId?: string }) {
    const next: PromptBlock = {
      id: crypto.randomUUID(),
      name: block.name.trim() || "未命名提示词块",
      positive: block.positive.trim(),
      negative: block.negative.trim(),
      favorite: false,
      folderId: block.folderId || "",
      usedAt: 0,
      createdAt: stamp(),
    };
    blocks.value = [next, ...blocks.value];
    persist("nai_prompt_blocks", blocks.value);
    return next;
  }

  function updateBlock(id: string, patch: Partial<PromptBlock>) {
    blocks.value = blocks.value.map((item) => (item.id === id ? { ...item, ...patch } : item));
    persist("nai_prompt_blocks", blocks.value);
  }

  function touchBlock(id: string) {
    updateBlock(id, { usedAt: stamp() });
  }

  function removeBlock(id: string) {
    blocks.value = blocks.value.filter((item) => item.id !== id);
    persist("nai_prompt_blocks", blocks.value);
  }

  function addFolder(name: string) {
    const folder = { id: crypto.randomUUID(), name: name.trim() || "未命名文件夹" };
    folders.value = [...folders.value, folder];
    persist("nai_block_folders", folders.value);
    return folder;
  }

  function removeFolder(id: string) {
    folders.value = folders.value.filter((item) => item.id !== id);
    blocks.value = blocks.value.map((item) => (item.folderId === id ? { ...item, folderId: "" } : item));
    persist("nai_block_folders", folders.value);
    persist("nai_prompt_blocks", blocks.value);
  }

  async function addVibe(item: { name: string; previewUrl: string; infoExtracted?: number; strength?: number; categoryId?: string }) {
    const id = crypto.randomUUID();
    let file = "";
    try {
      file = await keepImage(id, item.previewUrl);
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : "图片没有存到本地";
      return null;
    }
    const next: SavedVibe = {
      id,
      name: item.name.trim() || "Vibe",
      previewUrl: item.previewUrl,
      infoExtracted: item.infoExtracted ?? 1,
      strength: item.strength ?? 0.6,
      favorite: false,
      categoryId: item.categoryId || "",
      file,
      usedAt: 0,
      createdAt: stamp(),
    };
    const list = [next, ...vibes.value];
    if (!persist("nai_vibe_library", list.map(pack))) return null;
    vibes.value = list;
    return next;
  }

  function updateVibe(id: string, patch: Partial<SavedVibe>) {
    vibes.value = vibes.value.map((item) => (item.id === id ? { ...item, ...patch } : item));
    persist("nai_vibe_library", vibes.value.map(pack));
  }

  function touchVibe(id: string) {
    updateVibe(id, { usedAt: stamp() });
  }

  function removeVibe(id: string) {
    const removed = vibes.value.find((item) => item.id === id);
    vibes.value = vibes.value.filter((item) => item.id !== id);
    persist("nai_vibe_library", vibes.value.map(pack));
    if (removed?.file) void libraryDeleteImage(removed.file);
  }

  function addCategory(name: string) {
    const category = { id: crypto.randomUUID(), name: name.trim() || "未命名分类" };
    categories.value = [...categories.value, category];
    persist("nai_vibe_categories", categories.value);
    return category;
  }

  async function addPrecise(item: {
    name: string;
    previewUrl: string;
    type?: SavedPrecise["type"];
    strength?: number;
    fidelity?: number;
  }) {
    const id = crypto.randomUUID();
    let file = "";
    try {
      file = await keepImage(id, item.previewUrl);
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : "图片没有存到本地";
      return null;
    }
    const next: SavedPrecise = {
      id,
      name: item.name.trim() || "精准参考",
      previewUrl: item.previewUrl,
      type: normalizePreciseType(item.type),
      strength: item.strength ?? 1,
      fidelity: item.fidelity ?? 1,
      favorite: false,
      file,
      usedAt: 0,
      createdAt: stamp(),
    };
    const list = [next, ...precise.value];
    if (!persist("nai_precise_library", list.map(pack))) return null;
    precise.value = list;
    return next;
  }

  function updatePrecise(id: string, patch: Partial<SavedPrecise>) {
    precise.value = precise.value.map((item) => (item.id === id ? { ...item, ...patch } : item));
    persist("nai_precise_library", precise.value.map(pack));
  }

  function touchPrecise(id: string) {
    updatePrecise(id, { usedAt: stamp() });
  }

  function removePrecise(id: string) {
    const removed = precise.value.find((item) => item.id === id);
    precise.value = precise.value.filter((item) => item.id !== id);
    persist("nai_precise_library", precise.value.map(pack));
    if (removed?.file) void libraryDeleteImage(removed.file);
  }

  return {
    blocks,
    folders,
    vibes,
    categories,
    precise,
    error,
    addBlock,
    updateBlock,
    touchBlock,
    removeBlock,
    addFolder,
    removeFolder,
    addVibe,
    updateVibe,
    touchVibe,
    removeVibe,
    addCategory,
    addPrecise,
    updatePrecise,
    touchPrecise,
    removePrecise,
  };
});
