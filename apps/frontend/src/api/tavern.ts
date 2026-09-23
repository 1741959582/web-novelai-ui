import { invoke } from "@tauri-apps/api/core";

// ---------------------------------------------------------------------------
// 数据类型
// ---------------------------------------------------------------------------

export type ProviderId = "deepseek" | "xai" | "custom";

export interface ProviderConfig {
  id: ProviderId;
  label: string;
  baseUrl: string;
  apiKey: string;
  model: string;
  /** 生图提示词导演用的模型，留空则与对话模型相同 */
  directorModel: string;
}

export type ImageMode = "manual" | "confirm" | "auto";
export type SizeMode = "auto" | "follow";

export interface TavernConfig {
  provider: ProviderId;
  providers: Record<ProviderId, ProviderConfig>;
  temperature: number;
  topP: number;
  maxTokens: number;
  stream: boolean;
  contextMessages: number;
  loreScanDepth: number;
  userName: string;
  userPersona: string;
  mainPrompt: string;
  directorPrompt: string;
  showReasoning: boolean;
  imageMode: ImageMode;
  sizeMode: SizeMode;
  extraNegative: string;
  directorContext: number;
}

/** NAI 角色提示词预设（V4+ 角色提示词框） */
export interface CharPreset {
  id: string;
  name: string;
  /** 给 AI 看的识别说明：别名、身份等 */
  note: string;
  prompt: string;
  negative: string;
}

/** 画风预设（写进 stylePrompt） */
export interface StylePreset {
  id: string;
  name: string;
  prompt: string;
  negative: string;
}

export interface LoreEntry {
  id: string;
  keys: string[];
  content: string;
  comment: string;
  enabled: boolean;
  constant: boolean;
}

export interface TavernCard {
  id: string;
  name: string;
  avatar: string;
  description: string;
  personality: string;
  scenario: string;
  firstMes: string;
  alternateGreetings: string[];
  mesExample: string;
  systemPrompt: string;
  postHistoryInstructions: string;
  creatorNotes: string;
  creator: string;
  tags: string[];
  lore: LoreEntry[];
  /** 本卡默认出场的角色 tag 预设 */
  charPresetIds: string[];
  /** 本卡默认画风 */
  stylePresetId: string;
  /** 导入时的 extensions，导出时原样带回 */
  extensions: Record<string, unknown>;
  createdAt: number;
  updatedAt: number;
}

export interface DraftChar {
  presetId: string;
  name: string;
  tags: string;
  x: number;
  y: number;
}

export interface ImageDraft {
  orientation: "portrait" | "landscape" | "square";
  scene: string;
  negative: string;
  chars: DraftChar[];
}

export interface ChatImage {
  id: string;
  path: string;
  seed: number;
  createdAt: number;
}

export interface ChatMessage {
  id: string;
  role: "user" | "assistant";
  swipes: string[];
  reasoning: string[];
  swipeIdx: number;
  images: ChatImage[];
  draft: ImageDraft | null;
  draftState: "" | "loading" | "ready" | "generating" | "error";
  draftError: string;
  createdAt: number;
}

export interface ChatMeta {
  id: string;
  cardId: string;
  title: string;
  count: number;
  updatedAt: number;
}

export interface ChatFile {
  id: string;
  cardId: string;
  messages: ChatMessage[];
  castIds: string[];
  styleId: string;
}

export interface TavernState {
  version: number;
  config: TavernConfig;
  cards: TavernCard[];
  charPresets: CharPreset[];
  stylePresets: StylePreset[];
  chats: ChatMeta[];
  activeCardId: string;
  activeChatId: string;
}

// ---------------------------------------------------------------------------
// Tauri 调用
// ---------------------------------------------------------------------------

export function tavernRead<T = unknown>(name: string) {
  return invoke<T | null>("tavern_read", { name });
}

export function tavernWrite(name: string, data: unknown) {
  return invoke<void>("tavern_write", { name, data });
}

export function tavernRemove(name: string) {
  return invoke<void>("tavern_remove", { name });
}

export function tavernOpenDir() {
  return invoke<void>("tavern_open_dir");
}

export function tavernExportFile(fileName: string, base64Data: string) {
  return invoke<string | null>("tavern_export_file", { fileName, base64Data });
}

export interface LlmMessage {
  role: "system" | "user" | "assistant";
  content: string;
}

export interface LlmRequest {
  requestId: string;
  baseUrl: string;
  apiKey: string;
  model: string;
  messages: LlmMessage[];
  temperature?: number;
  topP?: number;
  maxTokens?: number;
  stream?: boolean;
}

export interface LlmResult {
  content: string;
  reasoning: string;
  finishReason: string;
}

export interface LlmDelta {
  requestId: string;
  content: string;
  reasoning: string;
}

export function llmChat(request: LlmRequest) {
  return invoke<LlmResult>("tavern_llm_chat", { request });
}

export function llmCancel(requestId: string) {
  return invoke<void>("tavern_llm_cancel", { requestId });
}

export function llmModels(baseUrl: string, apiKey: string) {
  return invoke<string[]>("tavern_llm_models", { baseUrl, apiKey });
}
