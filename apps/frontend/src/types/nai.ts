export const APP_NAME = "Langbai NovelAI Studio";

export const NAI_MODELS = [
  { label: "NAI Diffusion V5 Full（最新完整模型）", value: "nai-diffusion-5-full" },
  { label: "NAI Diffusion V5 Curated（最新精选模型）", value: "nai-diffusion-5-curated" },
  { label: "NAI Diffusion 4.5 Full（完整模型）", value: "nai-diffusion-4-5-full" },
  { label: "NAI Diffusion 4.5 Curated（精选模型）", value: "nai-diffusion-4-5-curated" },
  { label: "NAI Diffusion 4 Full（完整模型）", value: "nai-diffusion-4-full" },
  { label: "NAI Diffusion 4 Curated（精选模型）", value: "nai-diffusion-4-curated" },
  { label: "NAI Diffusion 3（旧版通用）", value: "nai-diffusion-3" },
  { label: "NAI Diffusion Furry 3（兽人模型）", value: "nai-diffusion-furry-3" },
] as const;

export const NAI_SAMPLERS = [
  { label: "Euler Ancestral（推荐）", short: "Euler Ancestral", value: "k_euler_ancestral" },
  { label: "Euler", short: "Euler", value: "k_euler" },
  { label: "DPM++ 2M", short: "DPM++ 2M", value: "k_dpmpp_2m" },
  { label: "DPM++ 2M SDE", short: "DPM++ 2M SDE", value: "k_dpmpp_2m_sde" },
  { label: "DPM++ SDE", short: "DPM++ SDE", value: "k_dpmpp_sde" },
  { label: "DPM++ 2S Ancestral", short: "DPM++ 2S Ancestral", value: "k_dpmpp_2s_ancestral" },
  { label: "DDIM", short: "DDIM", value: "ddim_v3" },
] as const;

export const NAI_UC_PRESETS = [
  { label: "Heavy（强负面）", value: 0 },
  { label: "Light（轻负面）", value: 1 },
  { label: "Human Focus（人物优先）", value: 2 },
  { label: "None（不使用预设）", value: 3 },
] as const;

export const DIRECTOR_TOOLS = [
  { label: "移除背景", value: "bg-removal", cost: 65 },
  { label: "去除杂乱", value: "declutter", cost: 0 },
  { label: "线稿提取", value: "lineart", cost: 0 },
  { label: "草图化", value: "sketch", cost: 0 },
] as const;

export const SIZE_PRESETS = [
  { label: "竖图 832×1216", width: 832, height: 1216 },
  { label: "横图 1216×832", width: 1216, height: 832 },
  { label: "方图 1024×1024", width: 1024, height: 1024 },
  { label: "竖图 1024×1536", width: 1024, height: 1536 },
  { label: "横图 1536×1024", width: 1536, height: 1024 },
];

export interface GenerateParams {
  model: string;
  stylePrompt: string;
  positivePrompt: string;
  negativePrompt: string;
  width: number;
  height: number;
  steps: number;
  cfgScale: number;
  cfgRescale: number;
  sampler: string;
  noiseSchedule: string;
  seed: number;
  seedMode: "random" | "fixed";
  ucPreset: number;
  qualityPreset: "standard" | "light" | "none";
  transparentBackground: boolean;
  smea: boolean;
  smeaDyn: boolean;
  variety: boolean;
  fileNamePrefix: string;
  modelMode: "anime" | "furry";
}

export const DEFAULT_PARAMS: GenerateParams = {
  model: "nai-diffusion-5-full",
  stylePrompt: "",
  positivePrompt: "",
  negativePrompt: "",
  width: 832,
  height: 1216,
  steps: 28,
  cfgScale: 5,
  cfgRescale: 0,
  sampler: "k_euler_ancestral",
  noiseSchedule: "karras",
  seed: 0,
  seedMode: "random",
  ucPreset: 2,
  qualityPreset: "standard",
  transparentBackground: false,
  smea: false,
  smeaDyn: false,
  variety: false,
  fileNamePrefix: "",
  modelMode: "anime",
};

export interface AppSettings {
  hasOnboarded: boolean;
  outputDir: string;
  apiBaseUrl: string;
  imageBaseUrl: string;
  allowCustomEndpoint: boolean;
  allowCustomEndpointFallback: boolean;
  proxyUrl: string;
  keepImageMetadata: boolean;
  theme: string;
  token: string;
  streamPreviewEnabled: boolean;
  huggingfaceToken: string;
  localClTaggerEnabled: boolean;
  localClTaggerThreshold: number;
}

export interface OpusGenerationUsage {
  percent: number;
  isNegative: boolean;
  timeUntilNextPercent: number;
  remainingImages?: number;
  maxImages?: number;
  dailyRefillImages?: number;
}

export interface AccountSummary {
  hasToken: boolean;
  tierName: string;
  tierLevel: number | null;
  anlasBalance: number | null;
  expiresAt: string | null;
  hasActiveSubscription: boolean;
  opusUsage?: OpusGenerationUsage | null;
  opusUsageUpdatedAt?: number | null;
}

export interface HistoryItem {
  id: string;
  path: string;
  createdAt: string;
  model: string;
  prompt: string;
  negativePrompt: string;
  seed: number;
  width: number;
  height: number;
  steps: number;
  sampler: string;
  kind: string;
  sessionId?: string;
  groupId?: string;
}

export interface HistoryGroup {
  id: string;
  name: string;
  createdAt: string;
}

export interface CharCaption {
  id: string;
  prompt: string;
  negativePrompt: string;
  useCoords: boolean;
  x: number;
  y: number;
  enabled: boolean;
}

export interface SessionRecord {
  id: string;
  savedAt: string;
  imageCount: number;
  thumbnailPath: string;
  params: Partial<GenerateParams>;
  characters: CharCaption[];
  i2iStrength: number;
  batchCount: number;
  imageIds: string[];
  imagePaths: string[];
}

export const MAX_VIBE_IMAGES = 16;
export const MAX_PRECISE_REFS = 6;

export type PreciseReferenceType = "character" | "style" | "character&style";

export interface VibeImage {
  id: string;
  previewUrl: string;
  base64: string;
  infoExtracted: number;
  strength: number;
  enabled: boolean;
  name?: string;
}

export interface PreciseReference {
  id: string;
  previewUrl: string;
  base64: string;
  type: PreciseReferenceType;
  strength: number;
  fidelity: number;
  enabled: boolean;
}

export function newVibeImage(previewUrl: string, base64: string): VibeImage {
  return {
    id: crypto.randomUUID(),
    previewUrl,
    base64,
    infoExtracted: 0.7,
    strength: 0.6,
    enabled: true,
  };
}

export function newPreciseReference(previewUrl: string, base64: string): PreciseReference {
  return {
    id: crypto.randomUUID(),
    previewUrl,
    base64,
    type: "character&style",
    strength: 1,
    fidelity: 1,
    enabled: true,
  };
}

export function isV4Plus(model: string) {
  return model.startsWith("nai-diffusion-4-") || model.startsWith("nai-diffusion-5-");
}

export function isV5(model: string) {
  return model.startsWith("nai-diffusion-5-");
}

export function modelToNai(value?: string | null) {
  if (!value) return undefined;
  if (NAI_MODELS.some((item) => item.value === value)) return value;
  const name = value.toLowerCase();
  if (name.includes("furry") && name.includes("v3")) return "nai-diffusion-furry-3";
  if (name.includes("v5")) return name.includes("curated") ? "nai-diffusion-5-curated" : "nai-diffusion-5-full";
  if (name.includes("v4.5") || name.includes("v4 5")) {
    return name.includes("curated") ? "nai-diffusion-4-5-curated" : "nai-diffusion-4-5-full";
  }
  if (name.includes("v4")) return name.includes("curated") ? "nai-diffusion-4-curated" : "nai-diffusion-4-full";
  if (name.includes("v3")) return "nai-diffusion-3";
  return undefined;
}

/** Desktop-app capability map: Vibe Transfer is V3/V4/V4.5 only. V5 launch does not accept it. */
export function supportsNAIVibeTransfer(model: string) {
  return !isV5(model);
}

/** Desktop-app capability map: Precise/Director Reference is V4.5 only. */
export function supportsNAIPreciseReference(model: string) {
  return model.startsWith("nai-diffusion-4-5-");
}

export function emptyMetadataReport(): MetadataReport {
  return {
    kind: "unknown",
    software: "",
    prompt: "",
    negative: "",
    model: "",
    seed: null,
    width: null,
    height: null,
    steps: null,
    sampler: "",
    hasMetadata: false,
    rawText: "",
    entries: [],
    characterCaptions: [],
  };
}

export function maxCharacterPrompts(model: string) {
  if (isV5(model)) return 32;
  if (isV4Plus(model)) return 6;
  return 0;
}

export function newCharacter(useCoords = false): CharCaption {
  return {
    id: crypto.randomUUID(),
    prompt: "",
    negativePrompt: "",
    useCoords,
    x: 0.5,
    y: 0.5,
    enabled: true,
  };
}

export interface GenerateResult {
  ok: boolean;
  message: string;
  items: HistoryItem[];
  actualSeed: number;
  account: AccountSummary;
}

export interface TokenStatus {
  valid: boolean;
  message: string;
  account: AccountSummary;
}

export interface MetadataReport {
  kind: string;
  software: string;
  prompt: string;
  negative: string;
  model: string;
  seed: number | null;
  width: number | null;
  height: number | null;
  steps: number | null;
  sampler: string;
  cfgScale?: number | null;
  cfgRescale?: number | null;
  characterCaptions?: CharCaption[];
  hasMetadata: boolean;
  rawText: string;
  entries: [string, string][];
}

export const TABS = [
  { id: "generate", path: "/", label: "生成" },
  { id: "batch", path: "/batch", label: "批量" },
  // { id: "inpaint", path: "/inpaint", label: "重绘" },
  { id: "postprocess", path: "/postprocess", label: "后期" },
  { id: "metadata", path: "/metadata", label: "原数据" },
  { id: "tools", path: "/tools", label: "工具" },
  { id: "apng", path: "/apng", label: "APNG" },
  { id: "reverse", path: "/reverse", label: "反推" },
  { id: "reference", path: "/reference", label: "参考预设" },
  { id: "gallery", path: "/gallery", label: "法典" },
  { id: "tavern", path: "/tavern", label: "酒馆" },
  { id: "records", path: "/records", label: "记录" },
  { id: "settings", path: "/settings", label: "设置" },
] as const;
