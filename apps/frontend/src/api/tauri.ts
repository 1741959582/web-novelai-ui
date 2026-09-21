import { invoke } from "@tauri-apps/api/core";
import type {
  AccountSummary,
  AppSettings,
  CharCaption,
  GenerateParams,
  GenerateResult,
  HistoryItem,
  MetadataReport,
  PreciseReference,
  SessionRecord,
  TokenStatus,
  VibeImage,
} from "@/types/nai";

export type GenerateApiRequest = GenerateParams & {
  imageBase64?: string;
  strength?: number;
  charCaptions?: CharCaption[];
  vibeImages?: Array<Pick<VibeImage, "base64" | "infoExtracted" | "strength">>;
  preciseReferences?: Array<Pick<PreciseReference, "base64" | "type" | "strength" | "fidelity">>;
  normalizeVibe?: boolean;
};

export function settingsGet() {
  return invoke<AppSettings>("settings_get");
}

export function settingsSave(settings: AppSettings) {
  return invoke<AppSettings>("settings_save", { settings });
}

export function historyList() {
  return invoke<HistoryItem[]>("history_list");
}

export function historyDelete(id: string) {
  return invoke<void>("history_delete", { id });
}

export function accountGet() {
  return invoke<AccountSummary>("account_get");
}

export function accountRefresh() {
  return invoke<AccountSummary>("account_refresh");
}

export function pickOutputDir() {
  return invoke<string | null>("pick_output_dir");
}

export function openOutputDir() {
  return invoke<void>("open_output_dir");
}

export function readImageDataUrl(path: string) {
  return invoke<string>("read_image_data_url", { path });
}

export function tokenVerify(token: string) {
  return invoke<TokenStatus>("token_verify", { token });
}

export function generateTxt2img(request: GenerateApiRequest) {
  return invoke<GenerateResult>("generate_txt2img", { request });
}

export function generateImg2img(request: GenerateApiRequest & { imageBase64: string; strength: number }) {
  return invoke<GenerateResult>("generate_img2img", { request });
}

export function sessionsList() {
  return invoke<SessionRecord[]>("sessions_list");
}

export function sessionUpsert(payload: {
  id?: string;
  params: GenerateParams;
  characters: CharCaption[];
  i2iStrength: number;
  batchCount: number;
  imageIds: string[];
  imagePaths: string[];
  thumbnailPath?: string;
}) {
  return invoke<SessionRecord>("session_upsert", { payload });
}

export function sessionLoad(id: string) {
  return invoke<SessionRecord>("session_load", { id });
}

export function sessionDelete(id: string) {
  return invoke<void>("session_delete", { id });
}

export function sessionNew() {
  return invoke<string>("session_new");
}

export function inspectImage(path: string) {
  return invoke<MetadataReport>("inspect_image", { path });
}

export function inspectImageBytes(base64Data: string) {
  return invoke<MetadataReport>("inspect_image_bytes", { base64Data, base64_data: base64Data });
}
