import { acceptHMRUpdate, defineStore } from "pinia";
import { computed, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  readImageDataUrl,
  reverseTaskCurrent,
  reverseTaskDelete,
  reverseTaskLoad,
  reverseTaskNew,
  reverseTaskSave,
  reverseTasksList,
  type ReverseJob,
  type ReverseTaskSummary,
} from "@/api/tauri";

export type ReverseJobStatus = "idle" | "running" | "done" | "error" | "cancelled";

const DEFAULT_MODEL = "SmilingWolf/wd-swinv2-tagger-v3";
const HF_TOKEN_KEY = "cl-hf-token";

export const useReverseStore = defineStore("reverse", () => {
  const jobs = ref<ReverseJob[]>([]);
  const activeId = ref("");
  const currentTaskId = ref("");
  const sealed = ref(false);
  const tasks = ref<ReverseTaskSummary[]>([]);
  const thumbs = ref<Record<string, string>>({});
  const model = ref(DEFAULT_MODEL);
  const generalThresh = ref(0.35);
  const generalMcut = ref(false);
  const characterThresh = ref(0.85);
  const characterMcut = ref(false);
  const hfToken = ref(typeof localStorage === "undefined" ? "" : localStorage.getItem(HF_TOKEN_KEY) || "");
  const batching = ref(false);
  const booted = ref(false);
  let persistTimer: number | undefined;
  let unlistenProgress: UnlistenFn | undefined;

  const active = computed(() => jobs.value.find((item) => item.id === activeId.value) ?? jobs.value[0]);
  const pendingJobs = computed(() => jobs.value.filter((item) => item.status === "idle" || item.status === "error" || item.status === "cancelled"));
  const runningJobs = computed(() => jobs.value.filter((item) => item.status === "running"));
  const doneJobs = computed(() => jobs.value.filter((item) => item.status === "done"));
  const busy = computed(() => batching.value || runningJobs.value.length > 0);

  function patchJob(id: string, patch: Partial<ReverseJob>) {
    jobs.value = jobs.value.map((item) => (item.id === id ? { ...item, ...patch } : item));
  }

  async function refreshThumbs(list = tasks.value) {
    for (const item of list) {
      if (!item.thumbnailPath || thumbs.value[item.id]) continue;
      try {
        thumbs.value[item.id] = await readImageDataUrl(item.thumbnailPath);
      } catch {
        thumbs.value[item.id] = "";
      }
    }
  }

  async function refreshList() {
    tasks.value = await reverseTasksList();
    await refreshThumbs();
  }

  function snapshotPayload(id?: string) {
    return {
      id,
      model: model.value,
      generalThresh: generalThresh.value,
      generalMcut: generalMcut.value,
      characterThresh: characterThresh.value,
      characterMcut: characterMcut.value,
      jobs: jobs.value.map((item) => ({
        ...item,
        status: item.status === "running" ? "idle" : item.status,
        progress: item.status === "done" ? 1 : 0,
      })),
    };
  }

  async function persist(createNew = false) {
    if (!jobs.value.length) return;
    const rec = await reverseTaskSave(snapshotPayload(createNew ? undefined : currentTaskId.value || undefined));
    currentTaskId.value = rec.id;
    await refreshList();
    return rec;
  }

  function schedulePersist() {
    if (persistTimer) window.clearTimeout(persistTimer);
    persistTimer = window.setTimeout(() => {
      void persist(false);
    }, 700);
  }

  async function ensureTask() {
    await persist(false);
  }

  async function beginRun() {
    const createNew = sealed.value && doneJobs.value.length > 0 && pendingJobs.value.length > 0;
    sealed.value = false;
    await persist(createNew);
  }

  async function finishRun() {
    sealed.value = true;
    await persist(false);
  }

  async function newTask() {
    if (jobs.value.length) {
      try {
        await persist(false);
      } catch {
        /* keep going */
      }
    }
    jobs.value = [];
    activeId.value = "";
    currentTaskId.value = "";
    sealed.value = false;
    try {
      await reverseTaskNew();
    } catch {
      /* ignore */
    }
  }

  async function loadTask(id: string) {
    if (jobs.value.length && currentTaskId.value && currentTaskId.value !== id) {
      try {
        await persist(false);
      } catch {
        /* still load */
      }
    }
    const rec = await reverseTaskLoad(id);
    jobs.value = rec.jobs.map((item) => ({
      ...item,
      status: (item.status as ReverseJobStatus) || "idle",
    }));
    currentTaskId.value = rec.id;
    model.value = rec.model || DEFAULT_MODEL;
    generalThresh.value = rec.generalThresh;
    generalMcut.value = rec.generalMcut;
    characterThresh.value = rec.characterThresh;
    characterMcut.value = rec.characterMcut;
    sealed.value = rec.jobs.some((item) => item.status === "done");
    activeId.value = rec.jobs[0]?.id || "";
    await refreshList();
  }

  async function deleteTask(id: string) {
    await reverseTaskDelete(id);
    delete thumbs.value[id];
    if (currentTaskId.value === id) {
      jobs.value = [];
      activeId.value = "";
      currentTaskId.value = "";
      sealed.value = false;
    }
    await refreshList();
  }

  function applyJobProgress(jobId: string, next: number, phase: string, message: string) {
    const job = jobs.value.find((item) => item.id === jobId);
    if (!job || job.status !== "running") return;
    const progress = next >= job.progress || phase === "done" || phase === "parsing" ? next : job.progress;
    patchJob(jobId, { progress, progressMsg: message || job.progressMsg });
  }

  async function boot() {
    if (booted.value) return;
    booted.value = true;
    try {
      await refreshList();
      const current = await reverseTaskCurrent();
      if (current && !jobs.value.length) await loadTask(current);
      unlistenProgress = await listen<{ jobId: string; progress: number; phase: string; message: string }>("wd-tag-progress", (event) => {
        applyJobProgress(event.payload.jobId, event.payload.progress, event.payload.phase, event.payload.message);
      });
    } catch {
      /* tauri not ready in browser preview */
    }
  }

  return {
    jobs,
    activeId,
    currentTaskId,
    sealed,
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
    patchJob,
    persist,
    schedulePersist,
    ensureTask,
    beginRun,
    finishRun,
    newTask,
    loadTask,
    deleteTask,
    boot,
  };
});

if (import.meta.hot) {
  import.meta.hot.accept(acceptHMRUpdate(useReverseStore, import.meta.hot));
}
