import { invoke } from "@tauri-apps/api/core";
import type {
  AccountSummary,
  AppSettings,
  CharCaption,
  GenerateParams,
  GenerateResult,
  HistoryGroup,
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
  noise?: number;
  maskBase64?: string;
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

export function historyGroupsList() {
  return invoke<HistoryGroup[]>("history_groups_list");
}

export function historyGroupCreate(name: string) {
  return invoke<HistoryGroup>("history_group_create", { name });
}

export function historyGroupRename(id: string, name: string) {
  return invoke<HistoryGroup>("history_group_rename", { id, name });
}

export function historyGroupDelete(id: string) {
  return invoke<void>("history_group_delete", { id });
}

export function historySetGroup(id: string, groupId: string) {
  return invoke<HistoryItem>("history_set_group", { id, groupId, group_id: groupId });
}

export function historyArrangeGroups() {
  return invoke<{ moved: number; missing: number; failed: number }>("history_arrange_groups");
}

export function revealInFolder(path: string) {
  return invoke<void>("reveal_in_folder", { path });
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

export function fetchRemoteImage(url: string) {
  return invoke<string>("fetch_remote_image", { url });
}

export function translateText(text: string, langpair = "zh-CN|en") {
  return invoke<string>("translate_text", { text, langpair });
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

export function upscaleImage(request: { imageBase64: string; scale?: number }) {
  return invoke<GenerateResult>("upscale_image", { request });
}

export function augmentImage(request: { imageBase64: string; tool: string; width?: number; height?: number; prompt?: string }) {
  return invoke<GenerateResult>("augment_image", { request });
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

export interface TagSuggestion {
  tag: string;
  count: number;
  category: number;
  description: string;
}

export function danbooruStatus() {
  return invoke<{ downloaded: boolean; count: number }>("danbooru_status");
}

export function downloadDanbooru() {
  return invoke<{ downloaded: boolean; count: number }>("download_danbooru");
}

export function suggestTags(query: string, limit = 8) {
  return invoke<TagSuggestion[]>("suggest_tags", { query, limit });
}

export interface TagLookup {
  tag: string;
  found: boolean;
  count: number;
  category: number;
  description: string;
}

export function lookupTags(names: string[]) {
  return invoke<TagLookup[]>("lookup_tags", { names });
}

export function addCustomTag(name: string, cn: string, category = 0) {
  return invoke<TagLookup>("add_custom_tag", { name, cn, category });
}

export interface WdLabel {
  label: string;
  confidence: number;
}

export interface WdGroup {
  label?: string;
  confidences: WdLabel[];
}

export interface WdTagResult {
  prompt: string;
  rating: WdGroup;
  characters: WdGroup;
  tags: WdGroup;
}

export function wdTagImage(request: {
  imageBase64: string;
  model?: string;
  generalThresh?: number;
  generalMcut?: boolean;
  characterThresh?: number;
  characterMcut?: boolean;
  jobId?: string;
  hfToken?: string;
}) {
  return invoke<WdTagResult>("wd_tag_image", { request });
}

export function wdTagCancel(jobId?: string) {
  return invoke<void>("wd_tag_cancel", { jobId: jobId ?? null, job_id: jobId ?? null });
}

export interface GpuInfo {
  name: string;
  vramMb: number;
  driver: string;
}

export interface GpuDetectResult {
  usable: boolean;
  reason: string;
  gpus: GpuInfo[];
}

export interface ClTaggerStatus {
  enabled: boolean;
  downloaded: boolean;
  version: string;
  dir: string;
  missing: string[];
}

export function gpuDetect() {
  return invoke<GpuDetectResult>("gpu_detect");
}

export function clTaggerStatus() {
  return invoke<ClTaggerStatus>("cl_tagger_status");
}

export function clTaggerDownload(token?: string) {
  return invoke<ClTaggerStatus>("cl_tagger_download", { token: token?.trim() || null });
}

export function clTaggerOpenDir() {
  return invoke<void>("cl_tagger_open_dir");
}

export interface ReverseTag {
  id: string;
  label: string;
  confidence: number;
  kind: "character" | "general" | string;
  found: boolean | null;
  description: string;
  category: number;
}

export interface ReverseRating {
  label: string;
  confidence: number;
}

export interface ReverseJob {
  id: string;
  name: string;
  image: string;
  path?: string;
  status: "idle" | "running" | "done" | "error" | "cancelled" | string;
  progress: number;
  progressMsg: string;
  error: string;
  tags: ReverseTag[];
  rating: ReverseRating[];
}

export interface ReverseTaskSummary {
  id: string;
  savedAt: string;
  name: string;
  imageCount: number;
  doneCount: number;
  thumbnailPath: string;
  model: string;
}

export interface ReverseTask {
  id: string;
  savedAt: string;
  name: string;
  model: string;
  generalThresh: number;
  generalMcut: boolean;
  characterThresh: number;
  characterMcut: boolean;
  jobs: ReverseJob[];
}

export function reverseTasksList() {
  return invoke<ReverseTaskSummary[]>("reverse_tasks_list");
}

export function reverseTaskCurrent() {
  return invoke<string | null>("reverse_task_current");
}

export function reverseTaskSave(payload: {
  id?: string;
  name?: string;
  model: string;
  generalThresh: number;
  generalMcut: boolean;
  characterThresh: number;
  characterMcut: boolean;
  jobs: ReverseJob[];
}) {
  return invoke<ReverseTaskSummary>("reverse_task_save", { payload });
}

export function reverseTaskLoad(id: string) {
  return invoke<ReverseTask>("reverse_task_load", { id });
}

export function reverseTaskDelete(id: string) {
  return invoke<void>("reverse_task_delete", { id });
}

export function reverseTaskNew() {
  return invoke<void>("reverse_task_new");
}

export interface QuickCollection {
  id: string;
  title: string;
  author: string;
  version: string;
  type: string;
  entryCount: number;
  imagedCount: number;
  nsfw: boolean;
  cover: string;
  coverUrl: string;
}

export interface QuickCategory {
  path: string[];
  count: number;
}

export interface QuickChar {
  label: string;
  prompt: string;
  negative?: string;
}

export interface QuickImage {
  previewUrl: string;
  originalUrl: string;
  width: number;
  height: number;
}

export interface QuickEntry {
  id: string;
  collectionId: string;
  title: string;
  path: string[];
  prompt: string;
  negative: string;
  note: string;
  nsfw: boolean;
  characterPrompts: QuickChar[];
  images: QuickImage[];
  coverUrl: string;
  sourceUrl: string;
  model: string;
  sampler: string;
  steps: number | null;
  seed: number | null;
  width: number | null;
  height: number | null;
  cfgScale: number | null;
}

export interface QuickPage {
  release: string;
  collectionId: string;
  collectionTitle: string;
  page: number;
  pageSize: number;
  total: number;
  categories: QuickCategory[];
  items: QuickEntry[];
}

export function quicktagCatalog(safeOnly = true) {
  return invoke<{ release: string; collections: QuickCollection[] }>("quicktag_catalog", { safeOnly, safe_only: safeOnly });
}

export function quicktagSearch(payload: {
  collectionId: string;
  query?: string;
  path?: string[];
  page?: number;
  pageSize?: number;
  safeOnly?: boolean;
}) {
  return invoke<QuickPage>("quicktag_search", {
    collectionId: payload.collectionId,
    collection_id: payload.collectionId,
    query: payload.query ?? "",
    path: payload.path ?? [],
    page: payload.page ?? 1,
    pageSize: payload.pageSize ?? 48,
    page_size: payload.pageSize ?? 48,
    safeOnly: payload.safeOnly ?? true,
    safe_only: payload.safeOnly ?? true,
  });
}

export function quicktagEntry(collectionId: string, entryId: string) {
  return invoke<QuickEntry>("quicktag_entry", { collectionId, collection_id: collectionId, entryId, entry_id: entryId });
}

export interface ReferencePreset {
  id: string;
  name: string;
  group: string;
  createdAt: string;
  sourceId?: string;
  vibeImages: Array<{
    previewUrl: string;
    base64: string;
    infoExtracted: number;
    strength: number;
    enabled: boolean;
    name?: string;
  }>;
  preciseReferences: Array<{
    previewUrl: string;
    base64: string;
    type: string;
    strength: number;
    fidelity: number;
    enabled: boolean;
  }>;
  normalizeVibe: boolean;
}

export function referencePresetList() {
  return invoke<ReferencePreset[]>("reference_preset_list");
}

export function referencePresetSave(preset: ReferencePreset) {
  return invoke<ReferencePreset>("reference_preset_save", { preset });
}

export function referencePresetDelete(id: string) {
  return invoke<void>("reference_preset_delete", { id });
}

export interface CatalogGame {
  id: string;
  name: string;
  categories: string[];
}

export interface CatalogAsset {
  id: string;
  game: string;
  category: string;
  name: string;
  search: string;
  width: number;
  height: number;
  bytes: number;
  thumbnailUrl: string;
  downloadUrls: string[];
}

export interface CatalogManifest {
  generatedAt: string;
  provider: string;
  games: CatalogGame[];
  assets: CatalogAsset[];
}

export function referenceCatalogLoad(refresh = false) {
  return invoke<CatalogManifest>("reference_catalog_load", { refresh });
}

export function referenceCatalogDownload(urls: string[]) {
  return invoke<string>("reference_catalog_download", { urls });
}

export interface SavedImage {
  path: string;
  dataUrl: string;
}

export function apngDisguise(payload: { cover: string; reals: string[]; delayMs?: number; padColor?: string; name?: string }) {
  return invoke<SavedImage>("apng_disguise", {
    cover: payload.cover,
    reals: payload.reals,
    delayMs: payload.delayMs,
    delay_ms: payload.delayMs,
    padColor: payload.padColor,
    pad_color: payload.padColor,
    name: payload.name,
  });
}

export function apngClean(image: string) {
  return invoke<string>("apng_clean", { image });
}

export function copyImageFiles(paths: string[]) {
  return invoke<void>("copy_image_files", { paths });
}

export function apngGif(payload: { frames: string[]; delayMs?: number; padColor?: string; fitFirst?: boolean }) {
  return invoke<SavedImage>("apng_gif", {
    frames: payload.frames,
    delayMs: payload.delayMs,
    delay_ms: payload.delayMs,
    padColor: payload.padColor,
    pad_color: payload.padColor,
    fitFirst: payload.fitFirst,
    fit_first: payload.fitFirst,
  });
}

export function apngStrip(image: string) {
  return invoke<SavedImage>("apng_strip", { image });
}

export function fileCleanedImages(items: { path: string; sourcePath: string }[], destDir: string) {
  return invoke<{ moved: number; skipped: number; paths: string[] }>("file_cleaned_images", {
    items,
    destDir,
    dest_dir: destDir,
  });
}

export function apngMosaic(image: string, block = 16) {
  return invoke<SavedImage>("apng_mosaic", { image, block });
}

export function apngRestore(image: string) {
  return invoke<SavedImage[]>("apng_restore", { image });
}

export function pickImages() {
  return invoke<SavedImage[]>("pick_images");
}

export interface AppUpdateInfo {
  current: string;
  latest: string;
  notes: string;
  htmlUrl: string;
  setupUrl?: string | null;
  portableUrl?: string | null;
  available: boolean;
  portable: boolean;
}

export function appVersion() {
  return invoke<string>("app_version");
}

export function checkAppUpdate() {
  return invoke<AppUpdateInfo>("check_app_update");
}

export function openLatestRelease(url?: string) {
  return invoke<void>("open_latest_release", { url: url || "" });
}

export function installAppUpdate(info: AppUpdateInfo) {
  return invoke<void>("install_app_update", { info });
}
