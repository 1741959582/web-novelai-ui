import { acceptHMRUpdate, defineStore } from "pinia";
import { computed, ref, watch } from "vue";
import { readImageDataUrl } from "@/api/tauri";
import {
  DEFAULT_PARAMS,
  maxCharacterPrompts,
  newCharacter,
  type CharCaption,
  type GenerateParams,
} from "@/types/nai";
import { useAppStore } from "@/stores/app";

export type BatchJobStatus = "pending" | "running" | "done" | "error";

export interface BatchJob {
  id: string;
  prompt: string;
  negativePrompt: string;
  stylePrompt: string;
  characters: CharCaption[];
  params: GenerateParams;
  copies: number;
  status: BatchJobStatus;
  error: string;
  previewUrl: string;
  path: string;
  seed: number | null;
}

export interface BatchDraft {
  prompt: string;
  negativePrompt: string;
  characters: string[];
  width?: number;
  height?: number;
}

const STORAGE_KEY = "nai-batch-queue";

export function cloneParams(src: Partial<GenerateParams> = {}): GenerateParams {
  return { ...DEFAULT_PARAMS, ...src };
}

export function cloneCharacters(src: CharCaption[] = []): CharCaption[] {
  return src.map((item) => ({ ...item, id: crypto.randomUUID() }));
}

export function parseCharacterLines(text: string): string[] {
  return text
    .split(/\r?\n/)
    .map((line) => line.replace(/^(角色\s*\d*|char(?:acter)?\s*\d*)\s*[:：]\s*/i, "").trim())
    .filter(Boolean);
}

export function parseBulkJobs(text: string, sharedChars: string[], sharedNeg: string): BatchDraft[] {
  const raw = text.replace(/\r\n/g, "\n").trim();
  if (!raw) return [];
  const useBlocks = /\n-{3,}\n/.test(`\n${raw}\n`) || /^(角色|char|character|负面|uc|negative|尺寸|size)\s*[:：]/im.test(raw);
  const chunks = useBlocks ? raw.split(/\n-{3,}\n/) : raw.split("\n");
  const drafts: BatchDraft[] = [];
  for (const chunk of chunks) {
    const draft = useBlocks ? parseBlock(chunk) : parseLine(chunk, sharedChars, sharedNeg);
    if (draft) drafts.push(draft);
  }
  return drafts;
}

function parseLine(line: string, sharedChars: string[], sharedNeg: string): BatchDraft | null {
  const prompt = line.trim();
  if (!prompt || /^-+$/.test(prompt)) return null;
  return { prompt, negativePrompt: sharedNeg, characters: [...sharedChars] };
}

function parseBlock(chunk: string): BatchDraft | null {
  const prompt: string[] = [];
  const characters: string[] = [];
  let negativePrompt = "";
  let width: number | undefined;
  let height: number | undefined;
  for (const raw of chunk.split("\n")) {
    const line = raw.trim();
    if (!line) continue;
    const char = line.match(/^(角色\s*\d*|char(?:acter)?\s*\d*)\s*[:：]\s*(.*)$/i);
    if (char) {
      if (char[2].trim()) characters.push(char[2].trim());
      continue;
    }
    const neg = line.match(/^(负面|uc|negative)\s*[:：]\s*(.*)$/i);
    if (neg) {
      negativePrompt = neg[2].trim();
      continue;
    }
    const size = line.match(/^(尺寸|size)\s*[:：]\s*(\d+)\s*[x×*]\s*(\d+)$/i);
    if (size) {
      width = Number(size[2]);
      height = Number(size[3]);
      continue;
    }
    prompt.push(line);
  }
  if (!prompt.length && !characters.length) return null;
  return { prompt: prompt.join("\n"), negativePrompt, characters, width, height };
}

function newJob(partial: Partial<BatchJob> = {}): BatchJob {
  return {
    id: crypto.randomUUID(),
    prompt: "",
    negativePrompt: "",
    stylePrompt: "",
    characters: [newCharacter()],
    params: cloneParams(),
    copies: 1,
    status: "pending",
    error: "",
    previewUrl: "",
    path: "",
    seed: null,
    ...partial,
  };
}

function persistable(jobs: BatchJob[]) {
  return jobs.map((job) => ({
    ...job,
    previewUrl: "",
    status: job.status === "running" ? "pending" : job.status,
    error: job.status === "running" ? "" : job.error,
  }));
}

export const useBatchStore = defineStore("batch", () => {
  const jobs = ref<BatchJob[]>([]);
  const bulkText = ref("");
  const sharedCharacters = ref<CharCaption[]>([newCharacter()]);
  const sharedNeg = ref("");
  const sharedCopies = ref(1);
  const running = ref(false);
  const stopRequested = ref(false);
  const currentId = ref("");
  const booted = ref(false);

  const pending = computed(() => jobs.value.filter((job) => job.status === "pending" || job.status === "error"));
  const done = computed(() => jobs.value.filter((job) => job.status === "done"));
  const current = computed(() => jobs.value.find((job) => job.id === currentId.value));
  const progressText = computed(() => {
    const total = jobs.value.length;
    if (!total) return "还没有任务";
    return `${done.value.length}/${total} 已完成`;
  });

  function patch(id: string, next: Partial<BatchJob>) {
    jobs.value = jobs.value.map((job) => (job.id === id ? { ...job, ...next } : job));
  }

  function updateJob(id: string, next: Partial<BatchJob>) {
    patch(id, next);
  }

  function addEmpty() {
    const app = useAppStore();
    jobs.value = [
      ...jobs.value,
      newJob({
        params: cloneParams(app.params),
        stylePrompt: app.params.stylePrompt,
        negativePrompt: app.params.negativePrompt || sharedNeg.value,
        characters: cloneCharacters(app.characters.length ? app.characters : [newCharacter()]),
        copies: sharedCopies.value,
      }),
    ];
  }

  function importCurrent() {
    const app = useAppStore();
    jobs.value = [
      ...jobs.value,
      newJob({
        prompt: app.params.positivePrompt,
        negativePrompt: app.params.negativePrompt,
        stylePrompt: app.params.stylePrompt,
        characters: cloneCharacters(app.characters.length ? app.characters : [newCharacter()]),
        params: cloneParams(app.params),
        copies: Math.min(4, Math.max(1, app.batchCount)),
      }),
    ];
  }

  function sharedForJobs() {
    const filled = sharedCharacters.value.filter((item) => item.enabled && (item.prompt.trim() || item.negativePrompt.trim()));
    return filled.length ? cloneCharacters(filled) : [newCharacter()];
  }

  function addSharedCharacter() {
    const app = useAppStore();
    const max = maxCharacterPrompts(app.params.model);
    if (!max || sharedCharacters.value.length >= max) return false;
    sharedCharacters.value = [...sharedCharacters.value, newCharacter()];
    return true;
  }

  function updateSharedCharacter(id: string, next: Partial<CharCaption>) {
    sharedCharacters.value = sharedCharacters.value.map((item) => (item.id === id ? { ...item, ...next } : item));
  }

  function removeSharedCharacter(id: string) {
    sharedCharacters.value = sharedCharacters.value.filter((item) => item.id !== id);
  }

  function moveSharedCharacter(id: string, delta: number) {
    const index = sharedCharacters.value.findIndex((item) => item.id === id);
    const next = index + delta;
    if (index < 0 || next < 0 || next >= sharedCharacters.value.length) return;
    const list = [...sharedCharacters.value];
    const [item] = list.splice(index, 1);
    list.splice(next, 0, item);
    sharedCharacters.value = list;
  }

  function addFromDrafts(drafts: BatchDraft[]) {
    const app = useAppStore();
    const extras = drafts.map((draft) =>
      newJob({
        prompt: draft.prompt,
        negativePrompt: draft.negativePrompt || sharedNeg.value || app.params.negativePrompt,
        stylePrompt: app.params.stylePrompt,
        characters: draft.characters.length
          ? draft.characters.map((prompt) => ({ ...newCharacter(), prompt }))
          : sharedForJobs(),
        params: cloneParams({
          ...app.params,
          width: draft.width || app.params.width,
          height: draft.height || app.params.height,
        }),
        copies: sharedCopies.value,
      }),
    );
    jobs.value = [...jobs.value, ...extras];
    return extras.length;
  }

  function addFromBulk() {
    const drafts = parseBulkJobs(bulkText.value, [], sharedNeg.value);
    return addFromDrafts(drafts);
  }

  function applySharedToAll() {
    const app = useAppStore();
    const chars = sharedForJobs();
    jobs.value = jobs.value.map((job) =>
      job.status === "done"
        ? job
        : {
            ...job,
            negativePrompt: sharedNeg.value || job.negativePrompt,
            characters: chars.some((item) => item.prompt.trim() || item.negativePrompt.trim()) ? cloneCharacters(chars) : job.characters,
            params: cloneParams({
              ...app.params,
              positivePrompt: job.prompt,
              negativePrompt: sharedNeg.value || job.negativePrompt,
              stylePrompt: job.stylePrompt,
            }),
            copies: sharedCopies.value,
            status: job.status === "error" ? "pending" : job.status,
            error: job.status === "error" ? "" : job.error,
          },
    );
  }

  function duplicate(id: string) {
    const job = jobs.value.find((item) => item.id === id);
    if (!job) return;
    const index = jobs.value.findIndex((item) => item.id === id);
    const copy = newJob({
      prompt: job.prompt,
      negativePrompt: job.negativePrompt,
      stylePrompt: job.stylePrompt,
      characters: cloneCharacters(job.characters),
      params: cloneParams(job.params),
      copies: job.copies,
    });
    jobs.value = [...jobs.value.slice(0, index + 1), copy, ...jobs.value.slice(index + 1)];
  }

  function remove(id: string) {
    jobs.value = jobs.value.filter((job) => job.id !== id);
    if (currentId.value === id) currentId.value = "";
  }

  function move(id: string, delta: number) {
    const index = jobs.value.findIndex((job) => job.id === id);
    const next = index + delta;
    if (index < 0 || next < 0 || next >= jobs.value.length) return;
    const list = [...jobs.value];
    const [item] = list.splice(index, 1);
    list.splice(next, 0, item);
    jobs.value = list;
  }

  function addCharacter(id: string) {
    const job = jobs.value.find((item) => item.id === id);
    if (!job) return;
    const max = maxCharacterPrompts(job.params.model);
    if (job.characters.length >= max) return;
    patch(id, { characters: [...job.characters, newCharacter()] });
  }

  function updateCharacter(id: string, charId: string, next: Partial<CharCaption>) {
    const job = jobs.value.find((item) => item.id === id);
    if (!job) return;
    patch(id, {
      characters: job.characters.map((item) => (item.id === charId ? { ...item, ...next } : item)),
    });
  }

  function moveCharacter(id: string, charId: string, delta: number) {
    const job = jobs.value.find((item) => item.id === id);
    if (!job) return;
    const index = job.characters.findIndex((item) => item.id === charId);
    const next = index + delta;
    if (index < 0 || next < 0 || next >= job.characters.length) return;
    const list = [...job.characters];
    const [item] = list.splice(index, 1);
    list.splice(next, 0, item);
    patch(id, { characters: list });
  }

  function removeCharacter(id: string, charId: string) {
    const job = jobs.value.find((item) => item.id === id);
    if (!job) return;
    const next = job.characters.filter((item) => item.id !== charId);
    patch(id, { characters: next.length ? next : [newCharacter()] });
  }

  function resetPending() {
    jobs.value = jobs.value.map((job) =>
      job.status === "error" || job.status === "done"
        ? { ...job, status: "pending", error: "", previewUrl: job.status === "done" ? job.previewUrl : "", path: job.status === "done" ? job.path : "", seed: job.status === "done" ? job.seed : null }
        : job,
    );
  }

  function resetErrors() {
    jobs.value = jobs.value.map((job) => (job.status === "error" ? { ...job, status: "pending", error: "" } : job));
  }

  function clearDone() {
    jobs.value = jobs.value.filter((job) => job.status !== "done");
  }

  function clearAll() {
    if (running.value) return;
    jobs.value = [];
    currentId.value = "";
  }

  function applyJob(job: BatchJob) {
    const app = useAppStore();
    Object.assign(
      app.params,
      cloneParams({
        ...job.params,
        positivePrompt: job.prompt,
        negativePrompt: job.negativePrompt,
        stylePrompt: job.stylePrompt,
      }),
    );
    app.characters = cloneCharacters(job.characters.length ? job.characters : [newCharacter()]);
    app.batchCount = Math.min(4, Math.max(1, job.copies || 1));
  }

  function sendToGenerate(id: string) {
    const job = jobs.value.find((item) => item.id === id);
    if (!job) return;
    applyJob(job);
    const app = useAppStore();
    app.status = "已把该任务参数写回生成页";
  }

  function requestStop() {
    stopRequested.value = true;
    const app = useAppStore();
    app.status = "当前这张完成后停止排队";
  }

  async function runQueue() {
    if (running.value) return;
    const app = useAppStore();
    if (app.busy) {
      app.status = "当前已有生成任务，等它结束后再排队";
      return;
    }
    const queue = jobs.value.filter((job) => job.status === "pending" || job.status === "error");
    if (!queue.length) {
      app.status = "没有可生成的任务";
      return;
    }
    running.value = true;
    stopRequested.value = false;
    try {
      for (const job of queue) {
        if (stopRequested.value) {
          app.status = "已停止排队";
          break;
        }
        currentId.value = job.id;
        patch(job.id, { status: "running", error: "" });
        applyJob(job);
        app.status = `批量生成 ${jobs.value.findIndex((item) => item.id === job.id) + 1}/${jobs.value.length}…`;
        const result = await app.generate("txt2img", { ignoreI2i: true });
        if (result.ok && result.items[0]) {
          const last = result.items[0];
          let preview = app.previewUrl;
          if (!preview && last.path) {
            try {
              preview = await readImageDataUrl(last.path);
            } catch {
              preview = "";
            }
          }
          patch(job.id, {
            status: "done",
            previewUrl: preview,
            path: last.path,
            seed: last.seed,
            error: "",
          });
        } else {
          patch(job.id, {
            status: "error",
            error: app.status || "生成失败",
          });
        }
      }
    } finally {
      running.value = false;
      currentId.value = stopRequested.value ? currentId.value : "";
      stopRequested.value = false;
    }
  }

  async function boot() {
    if (booted.value) return;
    booted.value = true;
    try {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (!raw) return;
      const parsed = JSON.parse(raw) as BatchJob[];
      if (!Array.isArray(parsed)) return;
      jobs.value = parsed.map((job) =>
        newJob({
          ...job,
          params: cloneParams(job.params),
          characters: cloneCharacters(job.characters?.length ? job.characters : [newCharacter()]),
          status: job.status === "running" ? "pending" : job.status,
          previewUrl: "",
        }),
      );
      for (const job of jobs.value) {
        if (!job.path) continue;
        try {
          job.previewUrl = await readImageDataUrl(job.path);
        } catch {
          job.previewUrl = "";
        }
      }
    } catch {
      jobs.value = [];
    }
  }

  watch(
    jobs,
    (list) => {
      if (!booted.value) return;
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(persistable(list)));
      } catch {
        /* ignore quota */
      }
    },
    { deep: true },
  );

  return {
    jobs,
    bulkText,
    sharedCharacters,
    addSharedCharacter,
    updateSharedCharacter,
    removeSharedCharacter,
    moveSharedCharacter,
    sharedNeg,
    sharedCopies,
    running,
    stopRequested,
    currentId,
    pending,
    done,
    current,
    progressText,
    boot,
    addEmpty,
    importCurrent,
    addFromBulk,
    applySharedToAll,
    duplicate,
    remove,
    move,
    addCharacter,
    updateCharacter,
    moveCharacter,
    removeCharacter,
    updateJob,
    resetPending,
    resetErrors,
    clearDone,
    clearAll,
    sendToGenerate,
    requestStop,
    runQueue,
  };
});

if (import.meta.hot) {
  import.meta.hot.accept(acceptHMRUpdate(useBatchStore, import.meta.hot));
}
