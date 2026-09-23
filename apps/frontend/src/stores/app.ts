import { acceptHMRUpdate, defineStore } from "pinia";
import { computed, ref } from "vue";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import {
  accountGet,
  accountRefresh,
  generateImg2img,
  generateTxt2img,
  upscaleImage,
  augmentImage,
  historyDelete,
  historyGroupCreate,
  historyGroupDelete,
  historyGroupRename,
  historyGroupsList,
  historyImportFolder,
  historyList,
  historySetGroup,
  historyArrangeGroups,
  revealInFolder,
  readImageDataUrl,
  sessionDelete,
  sessionLoad,
  sessionNew,
  sessionsList,
  sessionUpsert,
  fetchRemoteImage,
  inspectImage,
  settingsGet,
  settingsSave,
  tokenVerify,
  checkAppUpdate,
  installAppUpdate,
  openLatestRelease,
  type AppUpdateInfo,
  type QuickEntry,
  type ReferencePreset,
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
  type HistoryGroup,
  type HistoryItem,
  type MetadataReport,
  type PreciseReference,
  type SessionRecord,
  type VibeImage,
} from "@/types/nai";
import { dataUrlToBuffer, inspectImageBuffer } from "@/utils/imageMeta";
import { prepareOfficialInpaintAssets } from "@/utils/inpaintMask";

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
    huggingfaceToken: "",
    localClTaggerEnabled: false,
    localClTaggerThreshold: 0.55,
  });
  const account = ref<AccountSummary>({
    hasToken: false,
    tierName: "未知",
    tierLevel: null,
    anlasBalance: null,
    expiresAt: null,
    hasActiveSubscription: false,
    opusUsage: null,
    opusUsageUpdatedAt: null,
  });
  const params = ref<GenerateParams>({ ...DEFAULT_PARAMS });
  const characters = ref<CharCaption[]>([newCharacter()]);
  const history = ref<HistoryItem[]>([]);
  const historyGroups = ref<HistoryGroup[]>([]);
  const selectedHistoryGroupId = ref("");
  const sessions = ref<SessionRecord[]>([]);
  const currentSessionId = ref("");
  const showSessionDialog = ref(false);
  const ready = ref(false);
  const previewUrl = ref("");
  const status = ref("就绪");
  const busy = ref(false);
  const i2iImage = ref("");
  const i2iStrength = ref(0.7);
  const i2iNoise = ref(0);
  const vibeImages = ref<VibeImage[]>([]);
  const preciseReferences = ref<PreciseReference[]>([]);
  const normalizeVibe = ref(true);
  const batchCount = ref(1);
  const positionCustom = ref(false);
  const positionEditorOpen = ref(false);
  const overlayGuide = ref<"none" | "thirds" | "phi" | "grid">("none");
  const overlayCols = ref(2);
  const overlayRows = ref(2);
  const historyOpen = ref(false);
  const importPreview = ref("");
  const importReport = ref<MetadataReport | null>(null);
  const inpaintMask = ref("");
  const paintMode = ref(false);
  const paintEditorOpen = ref(false);
  const brushErase = ref(false);
  const brushSize = ref(20);
  const brushShape = ref<"round" | "square">("round");
  const currentItem = ref<HistoryItem | null>(null);
  const pinnedUrl = ref("");
  const genProgress = ref(0);
  const genStep = ref(0);
  const genSteps = ref(28);
  const genPreview = ref("");
  const genPhase = ref("");
  const streamFrames = ref<string[]>([]);
  const streamReplaying = ref(false);
  const appVersion = ref("");
  const updateInfo = ref<AppUpdateInfo | null>(null);
  const updateBusy = ref(false);
  const updatePct = ref(0);
  const updateMsg = ref("");
  const updateDismissed = ref("");
  let unlistenProgress: UnlistenFn | undefined;
  let unlistenUpdate: UnlistenFn | undefined;
  let genTick: number | undefined;
  let genStartedAt = 0;
  let lastProgressAt = 0;

  const hasToken = computed(() => account.value.hasToken || settings.value.token === "configured");
  const customPositions = computed(() => positionCustom.value || characters.value.some((c) => c.useCoords));

  async function boot() {
    try {
      settings.value = await settingsGet();
      account.value = await accountGet();
      history.value = await historyList();
      historyGroups.value = await historyGroupsList();
      sessions.value = await sessionsList();
      ready.value = true;
      showSessionDialog.value = settings.value.hasOnboarded && sessions.value.length > 0;
      status.value = hasToken.value ? "API 已配置" : "请先在设置中填写 Token";
      void checkForAppUpdate();
      unlistenProgress = await listen<{
        progress: number;
        currentStep: number;
        totalSteps: number;
        previewDataUrl: string;
        phase: string;
      }>("generate-progress", (event) => {
        lastProgressAt = Date.now();
        if (event.payload.progress >= genProgress.value || event.payload.phase === "saving") {
          genProgress.value = event.payload.progress;
        }
        if (event.payload.currentStep >= genStep.value || event.payload.phase === "saving") {
          genStep.value = event.payload.currentStep;
        }
        genSteps.value = event.payload.totalSteps;
        genPhase.value = event.payload.phase;
        if (event.payload.previewDataUrl) {
          genPreview.value = event.payload.previewDataUrl;
          if (event.payload.phase === "streaming") {
            const last = streamFrames.value[streamFrames.value.length - 1];
            if (last !== event.payload.previewDataUrl && streamFrames.value.length < 48) {
              streamFrames.value = [...streamFrames.value, event.payload.previewDataUrl];
            }
          }
        }
      });
      unlistenUpdate = await listen<{ received: number; total: number; percent: number; message: string }>("app-update", (event) => {
        updatePct.value = Math.round(event.payload.percent);
        updateMsg.value = event.payload.message;
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
      applyAccount(res.account);
      settings.value = await settingsGet();
    }
    return res;
  }

  function applyAccount(next: AccountSummary) {
    const prev = account.value.opusUsage;
    const incoming = next.opusUsage ?? prev ?? null;
    let opusUsage = incoming;
    if (prev && incoming) {
      const prevRemain = prev.remainingImages ?? 0;
      const nextRemain = incoming.remainingImages ?? 0;
      if (prevRemain > 0 && nextRemain > prevRemain && Math.abs(Math.round(prev.percent) - Math.round(incoming.percent)) <= 1) {
        opusUsage = {
          ...incoming,
          remainingImages: prevRemain,
          percent: Math.min(100, Math.max(0, prevRemain / 17)),
        };
      }
    }
    account.value = {
      ...next,
      opusUsage,
      opusUsageUpdatedAt: next.opusUsage ? next.opusUsageUpdatedAt : account.value.opusUsageUpdatedAt,
    };
  }

  async function refreshAccount(silent = false) {
    try {
      applyAccount(await accountRefresh());
      if (!silent) status.value = `已刷新积分：${account.value.anlasBalance ?? 0} Anlas`;
    } catch (e) {
      if (!silent) status.value = formatErr(e);
    }
  }

  async function showHistory(item: HistoryItem) {
    currentItem.value = item;
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
    openImportDialog(preview, emptyMetadataReport());
    let report = emptyMetadataReport();
    try {
      report = await inspectImageBuffer(buffer ?? dataUrlToBuffer(preview));
    } catch {
      /* try rust fallback */
    }
    if (importPreview.value === preview) importReport.value = report;
  }

  async function openImportFromUrl(url: string) {
    status.value = "正在下载外站图片…";
    const preview = await fetchRemoteImage(url);
    await inspectAndOpenImport(preview);
    status.value = "已导入外站图片";
  }

  async function openImportFromFile(file: File | Blob) {
    if (file.size > 25 * 1024 * 1024) {
      status.value = "图片超过 25MB，无法直接导入。";
      return;
    }
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
    await inspectAndOpenImport(preview);
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

  function itemGroupId(item: HistoryItem) {
    return item.groupId?.trim() || "";
  }

  function visibleHistory() {
    const gid = selectedHistoryGroupId.value;
    if (!gid) return history.value;
    if (gid === "__ungrouped") return history.value.filter((h) => !itemGroupId(h));
    return history.value.filter((h) => itemGroupId(h) === gid);
  }

  async function refreshHistoryGroups() {
    historyGroups.value = await historyGroupsList();
  }

  async function createHistoryGroup(name: string) {
    const group = await historyGroupCreate(name);
    historyGroups.value = [...historyGroups.value, group];
    selectedHistoryGroupId.value = group.id;
    status.value = `已创建分组「${group.name}」`;
    return group;
  }

  async function renameHistoryGroup(id: string, name: string) {
    const group = await historyGroupRename(id, name);
    historyGroups.value = historyGroups.value.map((g) => (g.id === id ? group : g));
    history.value = await historyList();
    sessions.value = await sessionsList();
    status.value = `已重命名为「${group.name}」，文件夹已一起改名`;
    return group;
  }

  async function deleteHistoryGroup(id: string) {
    await historyGroupDelete(id);
    historyGroups.value = historyGroups.value.filter((g) => g.id !== id);
    history.value = await historyList();
    sessions.value = await sessionsList();
    if (selectedHistoryGroupId.value === id) selectedHistoryGroupId.value = "";
    status.value = "已删除分组，图片已移回输出目录";
  }

  async function importMetadataFolder() {
    const gid = selectedHistoryGroupId.value;
    const groupId = gid && gid !== "__ungrouped" ? gid : "";
    const result = await historyImportFolder(groupId);
    if (!result.cancelled) history.value = await historyList();
    return result;
  }

  async function setHistoryItemGroup(id: string, groupId: string) {
    const next = await historySetGroup(id, groupId);
    history.value = history.value.map((h) => (h.id === id ? next : h));
    if (currentItem.value?.id === id) currentItem.value = next;
    const group = historyGroups.value.find((g) => g.id === groupId);
    status.value = group ? `已放入文件夹「${group.name}」` : "已移回输出目录";
    return next;
  }

  async function arrangeHistoryGroups() {
    const result = await historyArrangeGroups();
    history.value = await historyList();
    sessions.value = await sessionsList();
    status.value = result.moved
      ? `已把 ${result.moved} 张图片放进对应分组文件夹`
      : "分组里的图片已经在对应文件夹里";
    if (result.missing) status.value += `，${result.missing} 张文件找不到`;
    if (result.failed) status.value += `，${result.failed} 张移动失败`;
    return result;
  }

  function syncedHistory(items: HistoryItem[]) {
    return items.map((item) => history.value.find((entry) => entry.id === item.id) || item);
  }

  async function assignSelectedGroup(items: HistoryItem[]) {
    const gid = selectedHistoryGroupId.value;
    if (!gid || gid === "__ungrouped" || !items.length) return syncedHistory(items);
    for (const item of items) {
      try {
        await setHistoryItemGroup(item.id, gid);
      } catch {
        /* keep the image visible even if filing it fails */
      }
    }
    return syncedHistory(items);
  }

  async function removeHistory(id: string) {
    await historyDelete(id);
    history.value = history.value.filter((h) => h.id !== id);
    if (currentItem.value?.id === id) {
      currentItem.value = null;
      previewUrl.value = "";
    }
  }

  async function copyHistoryImage(item: HistoryItem) {
    try {
      const src = await readImageDataUrl(item.path);
      const blob = await (await fetch(src)).blob();
      await navigator.clipboard.write([new ClipboardItem({ [blob.type || "image/png"]: blob })]);
      status.value = "已复制图片";
    } catch {
      status.value = "复制失败，请改用资源管理器打开后复制";
    }
  }

  async function revealHistoryItem(item: HistoryItem) {
    try {
      await revealInFolder(item.path);
      status.value = "已打开文件所在目录";
    } catch (e) {
      status.value = formatErr(e);
    }
  }

  function setPositionMode(custom: boolean) {
    positionCustom.value = custom;
    characters.value = characters.value.map((c) => ({ ...c, useCoords: custom }));
    if (!custom) positionEditorOpen.value = false;
  }

  function startPositionEditor() {
    setPositionMode(true);
    if (!characters.value.length) addCharacter();
    positionEditorOpen.value = true;
  }

  function finishPositionEditor() {
    positionEditorOpen.value = false;
    status.value = "已保存角色位置";
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

  function importCodexEntry(entry: QuickEntry) {
    applyImportedMetadata(
      {
        ...emptyMetadataReport(),
        kind: "quicktag",
        software: "QuickTagCloud",
        prompt: entry.prompt,
        negative: entry.negative,
        model: entry.model,
        seed: entry.seed,
        width: entry.width,
        height: entry.height,
        steps: entry.steps,
        sampler: entry.sampler,
        cfgScale: entry.cfgScale,
        characterCaptions: entry.characterPrompts.map((c) => ({
          ...newCharacter(),
          prompt: c.prompt,
          enabled: true,
        })),
        hasMetadata: Boolean(entry.prompt || entry.negative || entry.characterPrompts.length),
        rawText: [entry.title, entry.prompt, entry.negative].filter(Boolean).join("\n"),
      },
      { prompt: true, uc: true, characters: true, append: false, settings: true, seed: Boolean(entry.seed), clean: false },
    );
    status.value = entry.title ? `已导入法典「${entry.title}」` : "已导入法典词条";
  }

  function applyReferencePreset(preset: ReferencePreset) {
    vibeImages.value = preset.vibeImages.map((v) => ({
      ...newVibeImage(v.previewUrl, v.base64),
      infoExtracted: v.infoExtracted,
      strength: v.strength,
      enabled: v.enabled,
      name: v.name,
    }));
    preciseReferences.value = preset.preciseReferences.map((p) => ({
      ...newPreciseReference(p.previewUrl, p.base64),
      type: (p.type as PreciseReference["type"]) || "character&style",
      strength: p.strength,
      fidelity: p.fidelity,
      enabled: p.enabled,
    }));
    normalizeVibe.value = preset.normalizeVibe;
    status.value = `已套用参考预设「${preset.name}」`;
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

  function useImageAs(kind: "i2i" | "vibe" | "precise", dataUrl: string) {
    if (!dataUrl) {
      status.value = "当前没有可参考的图片";
      return false;
    }
    if (kind === "i2i") {
      i2iImage.value = dataUrl;
      status.value = "已设为 Image2Image 参考图";
      return true;
    }
    if (kind === "vibe") return addVibeFromDataUrl(dataUrl);
    return addPreciseFromDataUrl(dataUrl);
  }

  function currentSourceImage() {
    return i2iImage.value || previewUrl.value;
  }

  function clearI2i() {
    i2iImage.value = "";
    inpaintMask.value = "";
    paintMode.value = false;
    paintEditorOpen.value = false;
  }

  function clearInpaintMask() {
    inpaintMask.value = "";
    status.value = "已清除局部重绘蒙版";
  }

  function startInpaint() {
    const src = currentSourceImage();
    if (!src) {
      status.value = "请先上传图生图底图，或用当前图";
      return false;
    }
    if (!i2iImage.value) i2iImage.value = src;
    paintMode.value = true;
    paintEditorOpen.value = true;
    brushErase.value = false;
    status.value = "涂抹要重绘的区域，然后 Save & Close";
    return true;
  }

  async function usePathAs(kind: "i2i" | "vibe" | "precise", path: string) {
    const url = await readImageDataUrl(path);
    return useImageAs(kind, url);
  }

  async function generate(
    kind: "txt2img" | "img2img" = "txt2img",
    options?: { ignoreI2i?: boolean },
  ): Promise<{ ok: boolean; items: HistoryItem[] }> {
    const empty = { ok: false, items: [] as HistoryItem[] };
    if (busy.value) return empty;
    const source = i2iImage.value || (inpaintMask.value && paintMode.value ? previewUrl.value : "");
    const hasMask = Boolean(inpaintMask.value) && paintMode.value;
    if (!options?.ignoreI2i && ((hasMask && source) || i2iImage.value)) kind = "img2img";
    const hasPrompt =
      params.value.positivePrompt.trim() ||
      params.value.stylePrompt.trim() ||
      characters.value.some((c) => c.prompt.trim()) ||
      vibeImages.value.some((v) => v.enabled) ||
      preciseReferences.value.some((p) => p.enabled);
    if (!hasPrompt && kind !== "img2img") {
      status.value = "请输入正面提示词、角色提示词，或加入参考图";
      return empty;
    }
    if (kind === "img2img" && !source) {
      status.value = "请先加载 Image2Image 参考图，或把当前图设为参考";
      return empty;
    }
    if (hasMask && !source) {
      status.value = "请先加载要涂抹重绘的原图";
      return empty;
    }
    const vibeOn = supportsNAIVibeTransfer(params.value.model) && vibeImages.value.some((v) => v.enabled && v.base64);
    const preciseOn = supportsNAIPreciseReference(params.value.model) && preciseReferences.value.some((p) => p.enabled && p.base64);
    busy.value = true;
    genProgress.value = 0.04;
    genStep.value = 0;
    genSteps.value = params.value.steps;
    genPreview.value = "";
    genPhase.value = settings.value.streamPreviewEnabled ? "streaming" : "waiting";
    streamFrames.value = [];
    startGenTicker();
    const n = Math.min(4, Math.max(1, batchCount.value));
    const extras = {
      vibeImages: vibeOn
        ? vibeImages.value
            .filter((v) => v.enabled && v.base64)
            .map((v) => ({ base64: stripDataUrl(v.base64), infoExtracted: v.infoExtracted, strength: v.strength }))
        : [],
      preciseReferences: preciseOn
        ? preciseReferences.value
            .filter((p) => p.enabled && p.base64)
            .map((p) => ({
              base64: stripDataUrl(p.base64),
              type: p.type,
              strength: p.strength,
              fidelity: p.fidelity,
            }))
        : [],
      normalizeVibe: normalizeVibe.value,
    };
    const collected: HistoryItem[] = [];
    try {
      for (let i = 0; i < n; i++) {
        status.value = n > 1 ? `正在生成 ${i + 1}/${n}…` : "正在生成…";
        const request = {
          ...params.value,
          charCaptions: characters.value,
          ...extras,
        };
        let imageBase64 = kind === "img2img" ? stripDataUrl(source) : "";
        let maskBase64 = hasMask ? stripDataUrl(inpaintMask.value) : undefined;
        let width = params.value.width;
        let height = params.value.height;
        if (hasMask && source) {
          const assets = await prepareOfficialInpaintAssets(
            source,
            inpaintMask.value,
            params.value.width,
            params.value.height,
          );
          imageBase64 = assets.imageBase64;
          maskBase64 = assets.maskBase64;
          width = assets.width;
          height = assets.height;
        }
        const res =
          kind === "img2img"
            ? await generateImg2img({
                ...request,
                width,
                height,
                imageBase64,
                strength: i2iStrength.value,
                noise: hasMask ? 0 : i2iNoise.value,
                maskBase64,
              })
            : await generateTxt2img(request);
        status.value = res.message;
        applyAccount(res.account);
        history.value = [...res.items, ...history.value];
        collected.push(...res.items);
        const saved = await assignSelectedGroup(res.items);
        if (saved[0]) await showHistory(saved[0]);
        await persistSession(saved);
        if (params.value.seedMode === "fixed") {
          params.value.seed = res.actualSeed + 1;
        }
      }
      return { ok: collected.length > 0, items: collected };
    } catch (e) {
      status.value = formatErr(e);
      return { ok: false, items: collected };
    } finally {
      stopGenTicker();
      busy.value = false;
    }
  }

  function startGenTicker() {
    stopGenTicker();
    genStartedAt = Date.now();
    lastProgressAt = Date.now();
    genTick = window.setInterval(() => {
      if (!busy.value || genPhase.value === "saving") return;
      const expected = Math.max(5000, genSteps.value * 480);
      const estimated = Math.min(0.92, (Date.now() - genStartedAt) / expected);
      const estStep = Math.max(1, Math.round(estimated * genSteps.value));
      if (genProgress.value < estimated) {
        genProgress.value = estimated;
        genStep.value = Math.max(genStep.value, estStep);
        if (genPhase.value === "waiting" || !genPhase.value) genPhase.value = "generating";
      }
    }, 160);
  }

  function stopGenTicker() {
    if (genTick) {
      window.clearInterval(genTick);
      genTick = undefined;
    }
  }

  async function toggleStreamPreview() {
    settings.value.streamPreviewEnabled = !settings.value.streamPreviewEnabled;
    await saveSettings();
    status.value = settings.value.streamPreviewEnabled ? "流式预览已开启" : "流式预览已关闭";
  }

  async function replayStream() {
    if (busy.value || streamReplaying.value) return;
    if (!streamFrames.value.length) {
      await generate();
      return;
    }
    streamReplaying.value = true;
    try {
      for (const url of streamFrames.value) {
        genPreview.value = url;
        await new Promise((resolve) => window.setTimeout(resolve, 90));
      }
      if (previewUrl.value) genPreview.value = "";
    } finally {
      streamReplaying.value = false;
    }
  }

  function currentPreview() {
    return previewUrl.value || i2iImage.value;
  }

  async function applyToolResult(res: Awaited<ReturnType<typeof generateImg2img>>) {
    status.value = res.message;
    applyAccount(res.account);
    history.value = [...res.items, ...history.value];
    const saved = await assignSelectedGroup(res.items);
    if (saved[0]) await showHistory(saved[0]);
    await persistSession(saved);
  }

  async function enhanceCurrent() {
    const src = currentPreview();
    if (!src) {
      status.value = "没有可增强的图片";
      return;
    }
    busy.value = true;
    genPhase.value = "waiting";
    try {
      const res = await generateImg2img({
        ...params.value,
        charCaptions: characters.value,
        imageBase64: stripDataUrl(src),
        strength: 0.25,
        noise: 0,
      });
      await applyToolResult(res);
    } catch (e) {
      status.value = formatErr(e);
    } finally {
      busy.value = false;
    }
  }

  async function upscaleCurrent() {
    const src = currentPreview();
    if (!src) {
      status.value = "没有可超分的图片";
      return;
    }
    busy.value = true;
    genPhase.value = "waiting";
    try {
      const res = await upscaleImage({ imageBase64: stripDataUrl(src), scale: 4 });
      await applyToolResult(res);
    } catch (e) {
      status.value = formatErr(e);
    } finally {
      busy.value = false;
    }
  }

  async function runDirector(tool: string) {
    const src = currentPreview();
    if (!src) {
      status.value = "没有可处理的图片";
      return;
    }
    busy.value = true;
    genPhase.value = "waiting";
    try {
      const res = await augmentImage({
        imageBase64: stripDataUrl(src),
        tool,
        width: params.value.width,
        height: params.value.height,
      });
      await applyToolResult(res);
    } catch (e) {
      status.value = formatErr(e);
    } finally {
      busy.value = false;
    }
  }

  async function copyCurrentImage() {
    const src = currentPreview();
    if (!src) return;
    try {
      const blob = await (await fetch(src)).blob();
      await navigator.clipboard.write([new ClipboardItem({ [blob.type || "image/png"]: blob })]);
      status.value = "已复制图片";
    } catch {
      status.value = "复制失败，请改用下载";
    }
  }

  function togglePin() {
    const src = currentPreview();
    if (!src) return;
    pinnedUrl.value = pinnedUrl.value === src ? "" : src;
    status.value = pinnedUrl.value ? "已固定当前图，可与下一张对比" : "已取消固定";
  }

  function useCurrentAsI2i() {
    const src = currentPreview();
    if (!src) {
      status.value = "没有可设为底图的图片";
      return;
    }
    useImageAs("i2i", src);
  }

  function downloadCurrentImage() {
    const src = currentPreview();
    if (!src) return;
    const link = document.createElement("a");
    link.href = src;
    link.download = currentItem.value
      ? `${currentItem.value.id}.png`
      : `novelai-${Date.now()}.png`;
    link.click();
    status.value = "已开始下载";
  }

  async function checkForAppUpdate() {
    try {
      const info = await checkAppUpdate();
      appVersion.value = info.current;
      updateInfo.value = info;
      if (info.available) {
        status.value = `发现新版本 ${info.latest}`;
      }
    } catch (e) {
      appVersion.value = appVersion.value || "";
      if (!updateInfo.value) {
        /* keep quiet on boot; settings page will show the error */
        updateMsg.value = e instanceof Error ? e.message : String(e);
      }
    }
  }

  function dismissUpdate() {
    if (updateInfo.value) updateDismissed.value = updateInfo.value.latest;
  }

  async function applyAppUpdate() {
    if (!updateInfo.value || updateBusy.value) return;
    updateBusy.value = true;
    updatePct.value = 1;
    updateMsg.value = "正在下载新版本…";
    try {
      await installAppUpdate(updateInfo.value);
      status.value = "已启动更新";
    } catch (e) {
      updateMsg.value = e instanceof Error ? e.message : String(e);
      status.value = updateMsg.value;
    } finally {
      updateBusy.value = false;
    }
  }

  async function openUpdatePage() {
    await openLatestRelease(updateInfo.value?.htmlUrl || "");
  }

  return {
    settings,
    account,
    params,
    characters,
    history,
    historyGroups,
    selectedHistoryGroupId,
    sessions,
    currentSessionId,
    ready,
    showSessionDialog,
    previewUrl,
    status,
    busy,
    appVersion,
    updateInfo,
    updateBusy,
    updatePct,
    updateMsg,
    updateDismissed,
    i2iImage,
    i2iStrength,
    i2iNoise,
    vibeImages,
    preciseReferences,
    normalizeVibe,
    batchCount,
    historyOpen,
    importPreview,
    importReport,
    inpaintMask,
    paintMode,
    paintEditorOpen,
    brushErase,
    brushSize,
    brushShape,
    currentItem,
    pinnedUrl,
    useImageAs,
    usePathAs,
    currentSourceImage,
    clearI2i,
    clearInpaintMask,
    startInpaint,
    genProgress,
    genStep,
    genSteps,
    streamFrames,
    streamReplaying,
    toggleStreamPreview,
    replayStream,
    genPreview,
    genPhase,
    hasToken,
    customPositions,
    positionEditorOpen,
    overlayGuide,
    overlayCols,
    overlayRows,
    boot,
    saveSettings,
    verifyToken,
    refreshAccount,
    showHistory,
    applyHistory,
    removeHistory,
    visibleHistory,
    refreshHistoryGroups,
    createHistoryGroup,
    renameHistoryGroup,
    deleteHistoryGroup,
    importMetadataFolder,
    setHistoryItemGroup,
    arrangeHistoryGroups,
    copyHistoryImage,
    revealHistoryItem,
    setPositionMode,
    startPositionEditor,
    finishPositionEditor,
    addCharacter,
    removeCharacter,
    moveCharacter,
    updateCharacter,
    rollSeed,
    clearSeed,
    applyImportedMetadata,
    importCodexEntry,
    applyReferencePreset,
    closeImportDialog,
    openImportFromFile,
    openImportFromPath,
    openImportFromUrl,
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
    enhanceCurrent,
    upscaleCurrent,
    runDirector,
    copyCurrentImage,
    togglePin,
    useCurrentAsI2i,
    downloadCurrentImage,
    currentPreview,
    checkForAppUpdate,
    dismissUpdate,
    applyAppUpdate,
    openUpdatePage,
  };
});

if (import.meta.hot) {
  import.meta.hot.accept(acceptHMRUpdate(useAppStore, import.meta.hot));
}
