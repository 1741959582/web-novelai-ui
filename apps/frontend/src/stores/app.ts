import { acceptHMRUpdate, defineStore } from "pinia";
import { computed, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  accountGet,
  accountRefresh,
  generateImg2img,
  generateTxt2img,
  historyDelete,
  historyList,
  readImageDataUrl,
  sessionDelete,
  sessionLoad,
  sessionNew,
  sessionsList,
  sessionUpsert,
  inspectImage,
  inspectImageBytes,
  settingsGet,
  settingsSave,
  tokenVerify,
} from "@/api/tauri";
import {
  DEFAULT_PARAMS,
  MAX_PRECISE_REFS,
  MAX_VIBE_IMAGES,
  emptyMetadataReport,
  maxCharacterPrompts,
  modelToNai,
  newCharacter,
  newPreciseReference,
  newVibeImage,
  supportsNAIPreciseReference,
  supportsNAIVibeTransfer,
  type AccountSummary,
  type AppSettings,
  type CharCaption,
  type GenerateParams,
  type HistoryItem,
  type MetadataReport,
  type PreciseReference,
  type SessionRecord,
  type VibeImage,
} from "@/types/nai";
import { dataUrlToBuffer, inspectImageBuffer } from "@/utils/imageMeta";

function stripDataUrl(value: string) {
  const idx = value.indexOf(",");
  return idx >= 0 ? value.slice(idx + 1) : value;
}

function formatErr(e: unknown) {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) return String((e as { message: unknown }).message);
  return String(e);
}

export const useAppStore = defineStore("app", () => {
  const settings = ref<AppSettings>({
    hasOnboarded: false,
    outputDir: "",
    apiBaseUrl: "https://image.novelai.net",
    imageBaseUrl: "https://image.novelai.net",
    allowCustomEndpoint: false,
    allowCustomEndpointFallback: false,
    proxyUrl: "",
    keepImageMetadata: true,
    theme: "dark",
    token: "",
    streamPreviewEnabled: true,
  });
  const account = ref<AccountSummary>({
    hasToken: false,
    tierName: "未知",
    tierLevel: null,
    anlasBalance: null,
    expiresAt: null,
    hasActiveSubscription: false,
  });
  const params = ref<GenerateParams>({ ...DEFAULT_PARAMS });
  const characters = ref<CharCaption[]>([newCharacter()]);
  const history = ref<HistoryItem[]>([]);
  const sessions = ref<SessionRecord[]>([]);
  const currentSessionId = ref("");
  const showSessionDialog = ref(false);
  const previewUrl = ref("");
  const status = ref("就绪");
  const busy = ref(false);
  const i2iImage = ref("");
  const i2iStrength = ref(0.7);
  const vibeImages = ref<VibeImage[]>([]);
  const preciseReferences = ref<PreciseReference[]>([]);
  const normalizeVibe = ref(true);
  const batchCount = ref(1);
  const positionCustom = ref(false);
  const historyOpen = ref(false);
  const importPreview = ref("");
  const importReport = ref<MetadataReport | null>(null);
  const genProgress = ref(0);
  const genStep = ref(0);
  const genSteps = ref(28);
  const genPreview = ref("");
  const genPhase = ref("");
  let unlistenProgress: UnlistenFn | undefined;

  const hasToken = computed(() => account.value.hasToken || settings.value.token === "configured");
  const customPositions = computed(() => positionCustom.value || characters.value.some((c) => c.useCoords));

  async function boot() {
    try {
      settings.value = await settingsGet();
      account.value = await accountGet();
      history.value = await historyList();
      sessions.value = await sessionsList();
      showSessionDialog.value = sessions.value.length > 0;
      status.value = hasToken.value ? "API 已配置" : "请先在设置中填写 Token";
      unlistenProgress = await listen<{
        progress: number;
        currentStep: number;
        totalSteps: number;
        previewDataUrl: string;
        phase: string;
      }>("generate-progress", (event) => {
        genProgress.value = event.payload.progress;
        genStep.value = event.payload.currentStep;
        genSteps.value = event.payload.totalSteps;
        genPhase.value = event.payload.phase;
        if (event.payload.previewDataUrl) genPreview.value = event.payload.previewDataUrl;
      });
    } catch {
      status.value = "未连接到 Tauri 宿主，请用 npm run dev 启动桌面窗口";
    }
  }

  async function saveSettings() {
    settings.value = await settingsSave(settings.value);
  }

  async function verifyToken(token: string) {
    const res = await tokenVerify(token);
    status.value = res.message;
    if (res.valid) {
      account.value = res.account;
      settings.value = await settingsGet();
    }
    return res;
  }

  async function refreshAccount() {
    try {
      account.value = await accountRefresh();
      status.value = `已刷新积分：${account.value.anlasBalance ?? 0} Anlas`;
    } catch (e) {
      status.value = formatErr(e);
    }
  }

  async function showHistory(item: HistoryItem) {
    try {
      previewUrl.value = await readImageDataUrl(item.path);
    } catch {
      previewUrl.value = "";
    }
  }

  async function applyHistory(item: HistoryItem) {
    const mapped = modelToNai(item.model);
    params.value.model = mapped || item.model || params.value.model;
    params.value.positivePrompt = item.prompt;
    params.value.negativePrompt = item.negativePrompt;
    params.value.seed = item.seed;
    params.value.seedMode = "fixed";
    params.value.width = item.width;
    params.value.height = item.height;
    params.value.steps = item.steps;
    params.value.sampler = item.sampler || params.value.sampler;
    await showHistory(item);
    status.value = `已载入参数并锁定种子 ${item.seed}`;
  }

  function closeImportDialog() {
    importReport.value = null;
    importPreview.value = "";
  }

  function openImportDialog(preview: string, report: MetadataReport) {
    importPreview.value = preview;
    importReport.value = report;
  }

  async function inspectAndOpenImport(preview: string, buffer?: ArrayBuffer) {
    let report = emptyMetadataReport();
    try {
      report = await inspectImageBuffer(buffer ?? dataUrlToBuffer(preview));
    } catch {
      /* try rust fallback */
    }
    if (!report.hasMetadata) {
      try {
        const fallback = await inspectImageBytes(preview);
        if (fallback.hasMetadata) report = fallback;
      } catch {
        /* keep local parse */
      }
    }
    openImportDialog(preview, report);
  }

  async function openImportFromFile(file: File | Blob) {
    const bytes = new Uint8Array(await file.arrayBuffer());
    const blob = new Blob([bytes], { type: (file as File).type || "image/png" });
    const preview = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result));
      reader.onerror = reject;
      reader.readAsDataURL(blob);
    });
    await inspectAndOpenImport(preview, bytes.buffer);
  }

  async function openImportFromPath(path: string) {
    const preview = await readImageDataUrl(path);
    let report = emptyMetadataReport();
    try {
      report = await inspectImageBuffer(dataUrlToBuffer(preview));
    } catch {
      /* rust path inspect */
    }
    if (!report.hasMetadata) {
      try {
        const rust = await inspectImage(path);
        if (rust.hasMetadata) report = rust;
      } catch {
        /* keep local */
      }
    }
    openImportDialog(preview, report);
  }

  async function importFromHistory(item: HistoryItem) {
    let preview = "";
    try {
      preview = await readImageDataUrl(item.path);
    } catch {
      preview = "";
    }
    let report = emptyMetadataReport();
    if (preview) {
      try {
        report = await inspectImageBuffer(dataUrlToBuffer(preview));
      } catch {
        /* try rust / history fields */
      }
    }
    if (!report.hasMetadata) {
      try {
        const rust = await inspectImage(item.path);
        if (rust.hasMetadata) report = rust;
      } catch {
        /* history fields */
      }
    }
    if (!report.hasMetadata) {
      report = {
        ...emptyMetadataReport(),
        hasMetadata: true,
        kind: "novelai",
        prompt: item.prompt,
        negative: item.negativePrompt,
        model: modelToNai(item.model) || item.model,
        seed: item.seed,
        width: item.width,
        height: item.height,
        steps: item.steps,
        sampler: item.sampler,
      };
    } else if (!modelToNai(report.model) && item.model) {
      report.model = modelToNai(item.model) || report.model;
    }
    openImportDialog(preview, report);
    await showHistory(item);
  }

  async function removeHistory(id: string) {
    await historyDelete(id);
    history.value = history.value.filter((h) => h.id !== id);
    if (!history.value.some((h) => previewUrl.value.includes(h.id))) {
      previewUrl.value = "";
    }
  }

  function setPositionMode(custom: boolean) {
    positionCustom.value = custom;
    characters.value = characters.value.map((c) => ({ ...c, useCoords: custom }));
  }

  function addCharacter() {
    const max = maxCharacterPrompts(params.value.model);
    if (characters.value.length >= max) {
      status.value = `当前模型最多 ${max} 个角色提示`;
      return;
    }
    characters.value = [...characters.value, newCharacter(positionCustom.value)];
  }

  function removeCharacter(id: string) {
    characters.value = characters.value.filter((c) => c.id !== id);
  }

  function moveCharacter(id: string, dir: -1 | 1) {
    const idx = characters.value.findIndex((c) => c.id === id);
    const next = idx + dir;
    if (idx < 0 || next < 0 || next >= characters.value.length) return;
    const copy = [...characters.value];
    const [item] = copy.splice(idx, 1);
    copy.splice(next, 0, item);
    characters.value = copy;
  }

  function updateCharacter(id: string, patch: Partial<CharCaption>) {
    characters.value = characters.value.map((c) => (c.id === id ? { ...c, ...patch } : c));
  }

  function joinPrompt(current: string, incoming: string, append: boolean) {
    const next = incoming.trim();
    if (!next) return current;
    if (!append || !current.trim()) return next;
    return `${current.replace(/,\s*$/, "")}, ${next}`;
  }

  function cleanPrompt(text: string) {
    const drop = new Set([
      "very aesthetic",
      "masterpiece",
      "no text",
      "best quality",
      "amazing quality",
      "absurdres",
      "rating:general",
    ]);
    return text
      .split(",")
      .map((p) => p.trim())
      .filter((p) => p && !drop.has(p.toLowerCase()))
      .join(", ");
  }

  function applyImportedMetadata(
    report: MetadataReport,
    opts: {
      prompt: boolean;
      uc: boolean;
      characters: boolean;
      append: boolean;
      settings: boolean;
      seed: boolean;
      clean: boolean;
    },
  ) {
    const text = (value: string) => (opts.clean ? cleanPrompt(value) : value);
    if (opts.settings) {
      const mapped = modelToNai(report.model);
      if (mapped) params.value.model = mapped;
      if (report.steps) params.value.steps = report.steps;
      if (report.sampler) params.value.sampler = report.sampler;
      if (report.width) params.value.width = report.width;
      if (report.height) params.value.height = report.height;
      if (typeof report.cfgScale === "number") params.value.cfgScale = report.cfgScale;
      if (typeof report.cfgRescale === "number") params.value.cfgRescale = report.cfgRescale;
    }
    if (opts.prompt && report.prompt) {
      params.value.positivePrompt = joinPrompt(params.value.positivePrompt, text(report.prompt), opts.append);
    }
    if (opts.uc && report.negative) {
      params.value.negativePrompt = joinPrompt(params.value.negativePrompt, text(report.negative), opts.append);
    }
    if (opts.characters) {
      const incoming = (report.characterCaptions ?? []).map((c) => ({
        ...newCharacter(Boolean(c.useCoords)),
        prompt: text(c.prompt || ""),
        negativePrompt: text(c.negativePrompt || ""),
        useCoords: Boolean(c.useCoords),
        x: c.x ?? 0.5,
        y: c.y ?? 0.5,
        enabled: true,
      }));
      if (incoming.length) {
        characters.value = opts.append ? [...characters.value.filter((c) => c.prompt.trim()), ...incoming] : incoming;
        positionCustom.value = incoming.some((c) => c.useCoords);
      }
    }
    if (opts.seed && report.seed) {
      params.value.seed = report.seed;
      params.value.seedMode = "fixed";
    }
    status.value = "已导入图片元数据";
  }

  function rollSeed() {
    params.value.seed = Math.floor(Math.random() * 2_147_483_647) + 1;
    params.value.seedMode = "fixed";
  }

  function clearSeed() {
    params.value.seed = 0;
    params.value.seedMode = "random";
  }

  function addVibeFromDataUrl(dataUrl: string) {
    if (vibeImages.value.length >= MAX_VIBE_IMAGES) {
      status.value = `Vibe Transfer 最多 ${MAX_VIBE_IMAGES} 张`;
      return false;
    }
    vibeImages.value = [...vibeImages.value, newVibeImage(dataUrl, stripDataUrl(dataUrl))];
    status.value = `已加入 Vibe Transfer（${vibeImages.value.length}/${MAX_VIBE_IMAGES}）`;
    return true;
  }

  function updateVibeImage(id: string, patch: Partial<VibeImage>) {
    vibeImages.value = vibeImages.value.map((item) => (item.id === id ? { ...item, ...patch } : item));
  }

  function removeVibeImage(id: string) {
    vibeImages.value = vibeImages.value.filter((item) => item.id !== id);
  }

  function addPreciseFromDataUrl(dataUrl: string) {
    if (preciseReferences.value.length >= MAX_PRECISE_REFS) {
      status.value = `Precise Reference 最多 ${MAX_PRECISE_REFS} 张`;
      return false;
    }
    preciseReferences.value = [...preciseReferences.value, newPreciseReference(dataUrl, stripDataUrl(dataUrl))];
    status.value = `已加入 Precise Reference（${preciseReferences.value.length}）`;
    return true;
  }

  function updatePreciseReference(id: string, patch: Partial<PreciseReference>) {
    preciseReferences.value = preciseReferences.value.map((item) => (item.id === id ? { ...item, ...patch } : item));
  }

  function removePreciseReference(id: string) {
    preciseReferences.value = preciseReferences.value.filter((item) => item.id !== id);
  }

  async function persistSession(extraItems: HistoryItem[] = []) {
    const rec = await sessionUpsert({
      id: currentSessionId.value || undefined,
      params: { ...params.value },
      characters: characters.value.map((c) => ({ ...c })),
      i2iStrength: i2iStrength.value,
      batchCount: batchCount.value,
      imageIds: extraItems.map((i) => i.id),
      imagePaths: extraItems.map((i) => i.path),
      thumbnailPath: extraItems[0]?.path,
    });
    currentSessionId.value = rec.id;
    sessions.value = await sessionsList();
  }

  async function loadSession(id: string) {
    const rec = await sessionLoad(id);
    currentSessionId.value = rec.id;
    const nextParams = rec.params && typeof rec.params === "object" && !Array.isArray(rec.params) ? rec.params : {};
    params.value = { ...DEFAULT_PARAMS, ...(nextParams as Partial<GenerateParams>) };
    characters.value = Array.isArray(rec.characters)
      ? rec.characters.map((c) => ({ ...newCharacter(), ...c }))
      : [];
    positionCustom.value = characters.value.some((c) => c.useCoords);
    i2iStrength.value = rec.i2iStrength || 0.7;
    batchCount.value = Math.min(4, Math.max(1, rec.batchCount || 1));
    const first =
      history.value.find((h) => rec.imageIds.includes(h.id)) ||
      (rec.imagePaths[0] ? ({ path: rec.imagePaths[0] } as HistoryItem) : null);
    if (first?.path) {
      try {
        previewUrl.value = await readImageDataUrl(first.path);
      } catch {
        previewUrl.value = "";
      }
    }
    showSessionDialog.value = false;
    status.value = `已载入本地会话 · ${rec.imageCount} 张`;
  }

  async function removeSession(id: string) {
    await sessionDelete(id);
    sessions.value = sessions.value.filter((s) => s.id !== id);
    if (currentSessionId.value === id) currentSessionId.value = "";
  }

  async function startNewSession() {
    await sessionNew();
    currentSessionId.value = "";
    showSessionDialog.value = false;
    status.value = "已开始新会话";
  }

  async function generate(kind: "txt2img" | "img2img" = "txt2img") {
    const hasPrompt =
      params.value.positivePrompt.trim() ||
      params.value.stylePrompt.trim() ||
      characters.value.some((c) => c.prompt.trim()) ||
      vibeImages.value.some((v) => v.enabled) ||
      preciseReferences.value.some((p) => p.enabled);
    if (!hasPrompt && kind !== "img2img") {
      status.value = "请输入正面提示词、角色提示词，或加入参考图";
      return;
    }
    if (kind === "img2img" && !i2iImage.value) {
      status.value = "请先加载 Image2Image 参考图";
      return;
    }
    const vibeOn = vibeImages.value.some((v) => v.enabled && v.base64);
    const preciseOn = preciseReferences.value.some((p) => p.enabled && p.base64);
    if (vibeOn && !supportsNAIVibeTransfer(params.value.model)) {
      status.value = "Vibe Transfer 需要 V3 / V4 / V4.5，当前 V5 不支持。请切换模型或移除氛围图。";
      return;
    }
    if (preciseOn && !supportsNAIPreciseReference(params.value.model)) {
      status.value = "Precise Reference 仅支持 V4.5。请切换到 V4.5 Full/Curated，或移除精准参考图。";
      return;
    }
    busy.value = true;
    genProgress.value = 0.02;
    genStep.value = 0;
    genSteps.value = params.value.steps;
    genPreview.value = "";
    genPhase.value = settings.value.streamPreviewEnabled ? "streaming" : "waiting";
    const n = Math.min(4, Math.max(1, batchCount.value));
    const extras = {
      vibeImages: vibeImages.value
        .filter((v) => v.enabled && v.base64)
        .map((v) => ({ base64: stripDataUrl(v.base64), infoExtracted: v.infoExtracted, strength: v.strength })),
      preciseReferences: preciseReferences.value
        .filter((p) => p.enabled && p.base64)
        .map((p) => ({
          base64: stripDataUrl(p.base64),
          type: p.type,
          strength: p.strength,
          fidelity: p.fidelity,
        })),
      normalizeVibe: normalizeVibe.value,
    };
    try {
      for (let i = 0; i < n; i++) {
        status.value = n > 1 ? `正在生成 ${i + 1}/${n}…` : "正在生成…";
        const request = {
          ...params.value,
          charCaptions: characters.value,
          ...extras,
        };
        const res =
          kind === "img2img"
            ? await generateImg2img({
                ...request,
                imageBase64: stripDataUrl(i2iImage.value),
                strength: i2iStrength.value,
              })
            : await generateTxt2img(request);
        status.value = res.message;
        account.value = res.account;
        history.value = [...res.items, ...history.value];
        if (res.items[0]) await showHistory(res.items[0]);
        await persistSession(res.items);
        if (params.value.seedMode === "fixed") {
          params.value.seed = res.actualSeed + 1;
        }
      }
    } catch (e) {
      status.value = formatErr(e);
    } finally {
      busy.value = false;
    }
  }

  return {
    settings,
    account,
    params,
    characters,
    history,
    sessions,
    currentSessionId,
    showSessionDialog,
    previewUrl,
    status,
    busy,
    i2iImage,
    i2iStrength,
    vibeImages,
    preciseReferences,
    normalizeVibe,
    batchCount,
    historyOpen,
    importPreview,
    importReport,
    genProgress,
    genStep,
    genSteps,
    genPreview,
    genPhase,
    hasToken,
    customPositions,
    boot,
    saveSettings,
    verifyToken,
    refreshAccount,
    showHistory,
    applyHistory,
    removeHistory,
    setPositionMode,
    addCharacter,
    removeCharacter,
    moveCharacter,
    updateCharacter,
    rollSeed,
    clearSeed,
    applyImportedMetadata,
    closeImportDialog,
    openImportFromFile,
    openImportFromPath,
    importFromHistory,
    addVibeFromDataUrl,
    updateVibeImage,
    removeVibeImage,
    addPreciseFromDataUrl,
    updatePreciseReference,
    removePreciseReference,
    loadSession,
    removeSession,
    startNewSession,
    generate,
  };
});

if (import.meta.hot) {
  import.meta.hot.accept(acceptHMRUpdate(useAppStore, import.meta.hot));
}
