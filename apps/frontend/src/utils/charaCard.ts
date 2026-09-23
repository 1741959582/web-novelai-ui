/**
 * SillyTavern 角色卡读写：
 * - PNG 卡：tEXt 块 "ccv3"（V3）或 "chara"（V1/V2），内容为 base64 的 UTF-8 JSON
 * - JSON 卡：直接是 V1 / V2 / V3 结构
 * 导出时同时写入 chara(V2) 与 ccv3(V3)，SillyTavern 等前端都能识别。
 */
import type { CharPreset, LoreEntry, StylePreset, TavernCard } from "@/api/tavern";

const PNG_SIG = [137, 80, 78, 71, 13, 10, 26, 10];

// ---------------------------------------------------------------------------
// base64 / UTF-8
// ---------------------------------------------------------------------------

export function utf8ToBase64(text: string) {
  const bytes = new TextEncoder().encode(text);
  let bin = "";
  for (let i = 0; i < bytes.length; i += 0x8000) {
    bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(bin);
}

export function base64ToUtf8(b64: string) {
  const bin = atob(b64.replace(/\s+/g, ""));
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return new TextDecoder().decode(bytes);
}

export function bytesToBase64(bytes: Uint8Array) {
  let bin = "";
  for (let i = 0; i < bytes.length; i += 0x8000) {
    bin += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  }
  return btoa(bin);
}

// ---------------------------------------------------------------------------
// PNG chunks
// ---------------------------------------------------------------------------

let crcTable: Uint32Array | null = null;
function crc32(bytes: Uint8Array) {
  if (!crcTable) {
    crcTable = new Uint32Array(256);
    for (let n = 0; n < 256; n++) {
      let c = n;
      for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
      crcTable[n] = c >>> 0;
    }
  }
  let crc = 0xffffffff;
  for (let i = 0; i < bytes.length; i++) crc = crcTable[(crc ^ bytes[i]) & 0xff] ^ (crc >>> 8);
  return (crc ^ 0xffffffff) >>> 0;
}

interface PngChunk {
  type: string;
  data: Uint8Array;
}

function isPng(bytes: Uint8Array) {
  return PNG_SIG.every((v, i) => bytes[i] === v);
}

function readChunks(bytes: Uint8Array): PngChunk[] {
  if (!isPng(bytes)) throw new Error("不是 PNG 文件");
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const chunks: PngChunk[] = [];
  let off = 8;
  while (off + 8 <= bytes.length) {
    const len = view.getUint32(off);
    const type = String.fromCharCode(...bytes.subarray(off + 4, off + 8));
    const data = bytes.subarray(off + 8, off + 8 + len);
    chunks.push({ type, data });
    off += 12 + len;
    if (type === "IEND") break;
  }
  return chunks;
}

function readTextChunks(bytes: Uint8Array) {
  const out: Record<string, string> = {};
  for (const chunk of readChunks(bytes)) {
    if (chunk.type !== "tEXt") continue;
    const zero = chunk.data.indexOf(0);
    if (zero < 0) continue;
    const key = new TextDecoder("latin1").decode(chunk.data.subarray(0, zero));
    const value = new TextDecoder("latin1").decode(chunk.data.subarray(zero + 1));
    out[key.toLowerCase()] = value;
  }
  return out;
}

function makeChunk(type: string, data: Uint8Array) {
  const out = new Uint8Array(12 + data.length);
  const view = new DataView(out.buffer);
  view.setUint32(0, data.length);
  for (let i = 0; i < 4; i++) out[4 + i] = type.charCodeAt(i);
  out.set(data, 8);
  view.setUint32(8 + data.length, crc32(out.subarray(4, 8 + data.length)));
  return out;
}

function textChunk(key: string, value: string) {
  const k = new TextEncoder().encode(key);
  const v = new TextEncoder().encode(value); // value 是 base64，纯 ASCII
  const data = new Uint8Array(k.length + 1 + v.length);
  data.set(k, 0);
  data.set(v, k.length + 1);
  return makeChunk("tEXt", data);
}

/** 移除旧的 chara / ccv3 文本块并写入新的 */
export function writeCardChunks(png: Uint8Array, texts: Record<string, string>) {
  const chunks = readChunks(png);
  const parts: Uint8Array[] = [new Uint8Array(PNG_SIG)];
  for (const chunk of chunks) {
    if (chunk.type === "tEXt") {
      const zero = chunk.data.indexOf(0);
      const key = new TextDecoder("latin1").decode(chunk.data.subarray(0, Math.max(0, zero))).toLowerCase();
      if (key === "chara" || key === "ccv3") continue;
    }
    if (chunk.type === "IEND") {
      for (const [k, v] of Object.entries(texts)) parts.push(textChunk(k, v));
    }
    parts.push(makeChunk(chunk.type, chunk.data));
  }
  const total = parts.reduce((n, p) => n + p.length, 0);
  const out = new Uint8Array(total);
  let off = 0;
  for (const p of parts) {
    out.set(p, off);
    off += p.length;
  }
  return out;
}

// ---------------------------------------------------------------------------
// 图片处理
// ---------------------------------------------------------------------------

function loadImage(src: string) {
  return new Promise<HTMLImageElement>((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("图片读取失败"));
    img.src = src;
  });
}

/** 缩成头像大小的 JPEG data URL，避免存储文件过大 */
export async function makeAvatar(src: string, maxSide = 512) {
  const img = await loadImage(src);
  const scale = Math.min(1, maxSide / Math.max(img.width, img.height));
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(img.width * scale));
  canvas.height = Math.max(1, Math.round(img.height * scale));
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("无法创建画布");
  ctx.fillStyle = "#13152c";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
  return canvas.toDataURL("image/jpeg", 0.88);
}

async function toPngBytes(src: string) {
  const img = await loadImage(src);
  const canvas = document.createElement("canvas");
  canvas.width = img.width || 400;
  canvas.height = img.height || 600;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("无法创建画布");
  ctx.drawImage(img, 0, 0);
  const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/png"));
  if (!blob) throw new Error("PNG 编码失败");
  return new Uint8Array(await blob.arrayBuffer());
}

/** 没有头像时生成一张带名字的占位图 */
export function placeholderAvatar(name: string) {
  const canvas = document.createElement("canvas");
  canvas.width = 400;
  canvas.height = 600;
  const ctx = canvas.getContext("2d");
  if (!ctx) return "";
  const grad = ctx.createLinearGradient(0, 0, 400, 600);
  grad.addColorStop(0, "#22253f");
  grad.addColorStop(1, "#0e0f21");
  ctx.fillStyle = grad;
  ctx.fillRect(0, 0, 400, 600);
  ctx.fillStyle = "#f5f3c2";
  ctx.font = "bold 140px serif";
  ctx.textAlign = "center";
  ctx.textBaseline = "middle";
  ctx.fillText((name || "?").slice(0, 1), 200, 300);
  return canvas.toDataURL("image/png");
}

// ---------------------------------------------------------------------------
// 卡片结构转换
// ---------------------------------------------------------------------------

type AnyObj = Record<string, any>;

function str(v: unknown) {
  return typeof v === "string" ? v : v == null ? "" : String(v);
}

function uid() {
  return crypto.randomUUID();
}

function keyList(v: unknown): string[] {
  if (Array.isArray(v)) return v.map(str).map((s) => s.trim()).filter(Boolean);
  if (typeof v === "string") return v.split(",").map((s) => s.trim()).filter(Boolean);
  return [];
}

/** 兼容 character_book（卡内世界书）与 SillyTavern 独立世界书 JSON */
export function parseLore(book: unknown): LoreEntry[] {
  if (!book || typeof book !== "object") return [];
  const b = book as AnyObj;
  const raw: AnyObj[] = Array.isArray(b.entries)
    ? b.entries
    : b.entries && typeof b.entries === "object"
      ? Object.values(b.entries)
      : [];
  return raw
    .filter((e) => e && typeof e === "object")
    .map((e) => ({
      id: uid(),
      keys: keyList(e.keys ?? e.key),
      content: str(e.content),
      comment: str(e.comment ?? e.name ?? e.memo),
      enabled: e.enabled !== undefined ? Boolean(e.enabled) : !e.disable,
      constant: Boolean(e.constant),
    }))
    .filter((e) => e.content.trim());
}

export interface NaiStudioExt {
  characterPrompts?: Array<Pick<CharPreset, "name" | "note" | "prompt" | "negative">>;
  style?: Pick<StylePreset, "name" | "prompt" | "negative"> | null;
}

export interface ParsedCard {
  card: TavernCard;
  nai: NaiStudioExt | null;
}

export function emptyCard(name = "新角色"): TavernCard {
  const now = Date.now();
  return {
    id: uid(),
    name,
    avatar: "",
    description: "",
    personality: "",
    scenario: "",
    firstMes: "",
    alternateGreetings: [],
    mesExample: "",
    systemPrompt: "",
    postHistoryInstructions: "",
    creatorNotes: "",
    creator: "",
    tags: [],
    lore: [],
    charPresetIds: [],
    stylePresetId: "",
    extensions: {},
    createdAt: now,
    updatedAt: now,
  };
}

export function cardFromJson(json: AnyObj): ParsedCard {
  const isV2 = json.spec === "chara_card_v2" || json.spec === "chara_card_v3";
  const d: AnyObj = isV2 && json.data ? json.data : json;
  if (!d || typeof d !== "object" || (!d.name && !d.char_name)) {
    throw new Error("没有找到角色卡数据（缺少 name）");
  }
  const card = emptyCard(str(d.name ?? d.char_name));
  card.description = str(d.description ?? d.char_persona);
  card.personality = str(d.personality);
  card.scenario = str(d.scenario ?? d.world_scenario);
  card.firstMes = str(d.first_mes ?? d.char_greeting);
  card.alternateGreetings = Array.isArray(d.alternate_greetings) ? d.alternate_greetings.map(str).filter(Boolean) : [];
  card.mesExample = str(d.mes_example ?? d.example_dialogue);
  card.systemPrompt = str(d.system_prompt);
  card.postHistoryInstructions = str(d.post_history_instructions);
  card.creatorNotes = str(d.creator_notes ?? d.creatorcomment);
  card.creator = str(d.creator);
  card.tags = Array.isArray(d.tags) ? d.tags.map(str).filter(Boolean) : [];
  card.lore = parseLore(d.character_book);
  const ext: AnyObj = d.extensions && typeof d.extensions === "object" ? { ...d.extensions } : {};
  const nai = (ext.nai_studio as NaiStudioExt | undefined) ?? null;
  delete ext.nai_studio;
  card.extensions = ext;
  return { card, nai };
}

export async function parseCardFile(file: File): Promise<ParsedCard> {
  const lower = file.name.toLowerCase();
  if (lower.endsWith(".json")) {
    return cardFromJson(JSON.parse(await file.text()));
  }
  const bytes = new Uint8Array(await file.arrayBuffer());
  if (!isPng(bytes)) throw new Error("只支持 PNG 或 JSON 角色卡");
  const texts = readTextChunks(bytes);
  const payload = texts.ccv3 || texts.chara;
  if (!payload) throw new Error("这张 PNG 里没有角色卡数据");
  const parsed = cardFromJson(JSON.parse(base64ToUtf8(payload)));
  const url = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
  try {
    parsed.card.avatar = await makeAvatar(url);
  } finally {
    URL.revokeObjectURL(url);
  }
  return parsed;
}

function loreToBook(card: TavernCard) {
  return {
    name: `${card.name} 世界书`,
    extensions: {},
    entries: card.lore.map((e, i) => ({
      keys: e.keys,
      content: e.content,
      comment: e.comment,
      name: e.comment,
      enabled: e.enabled,
      constant: e.constant,
      insertion_order: 100,
      selective: false,
      secondary_keys: [],
      case_sensitive: false,
      priority: 10,
      id: i,
      position: "before_char",
      extensions: {},
      use_regex: false,
    })),
  };
}

export function cardToV2(card: TavernCard, nai: NaiStudioExt) {
  const data = {
    name: card.name,
    description: card.description,
    personality: card.personality,
    scenario: card.scenario,
    first_mes: card.firstMes,
    mes_example: card.mesExample,
    creator_notes: card.creatorNotes,
    system_prompt: card.systemPrompt,
    post_history_instructions: card.postHistoryInstructions,
    alternate_greetings: card.alternateGreetings,
    character_book: card.lore.length ? loreToBook(card) : undefined,
    tags: card.tags,
    creator: card.creator,
    character_version: "",
    extensions: { ...card.extensions, nai_studio: nai },
  };
  return {
    spec: "chara_card_v2",
    spec_version: "2.0",
    // V1 兼容字段
    name: data.name,
    description: data.description,
    personality: data.personality,
    scenario: data.scenario,
    first_mes: data.first_mes,
    mes_example: data.mes_example,
    data,
  };
}

function cardToV3(card: TavernCard, nai: NaiStudioExt) {
  const v2 = cardToV2(card, nai);
  return {
    spec: "chara_card_v3",
    spec_version: "3.0",
    data: { ...v2.data, group_only_greetings: [], nickname: "", creator_notes_multilingual: {}, source: [] },
  };
}

/** 导出 PNG 角色卡，返回 base64 */
export async function exportCardPng(card: TavernCard, nai: NaiStudioExt) {
  const src = card.avatar || placeholderAvatar(card.name);
  const png = await toPngBytes(src);
  const out = writeCardChunks(png, {
    chara: utf8ToBase64(JSON.stringify(cardToV2(card, nai))),
    ccv3: utf8ToBase64(JSON.stringify(cardToV3(card, nai))),
  });
  return bytesToBase64(out);
}

export function exportCardJson(card: TavernCard, nai: NaiStudioExt) {
  return utf8ToBase64(JSON.stringify(cardToV2(card, nai), null, 2));
}
