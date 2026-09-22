import { emptyMetadataReport, modelToNai, type CharCaption, type MetadataReport } from "@/types/nai";

const PNG_SIG = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
const STEALTH_SIGS = ["stealth_pnginfo", "stealth_pngcomp", "stealth_rgbinfo", "stealth_rgbcomp"] as const;

function latin1(bytes: Uint8Array) {
  return new TextDecoder("latin1").decode(bytes.slice());
}

function utf8(bytes: Uint8Array) {
  return new TextDecoder("utf-8").decode(bytes.slice());
}

function pick(map: Record<string, string>, keys: string[]) {
  for (const key of keys) {
    const hit = Object.entries(map).find(([name]) => name.toLowerCase() === key.toLowerCase());
    if (hit) return hit[1];
  }
  return "";
}

async function inflateZlib(data: Uint8Array) {
  const copy = Uint8Array.from(data);
  const ds = new DecompressionStream("deflate");
  const out = await new Response(new Blob([copy]).stream().pipeThrough(ds)).arrayBuffer();
  return new Uint8Array(out);
}

async function inflateGzip(data: Uint8Array) {
  const copy = Uint8Array.from(data);
  const ds = new DecompressionStream("gzip");
  const out = await new Response(new Blob([copy]).stream().pipeThrough(ds)).arrayBuffer();
  return new Uint8Array(out);
}

function parsePngChunks(buffer: ArrayBuffer) {
  const bytes = new Uint8Array(buffer);
  const chunks: { type: string; data: Uint8Array }[] = [];
  if (bytes.length < 8 || PNG_SIG.some((b, i) => bytes[i] !== b)) return chunks;
  const view = new DataView(buffer);
  let offset = 8;
  while (offset + 12 <= bytes.length) {
    const length = view.getUint32(offset, false);
    const dataStart = offset + 8;
    const dataEnd = dataStart + length;
    if (dataEnd + 4 > bytes.length) break;
    const type = latin1(bytes.subarray(offset + 4, offset + 8));
    chunks.push({ type, data: bytes.slice(dataStart, dataEnd) });
    if (type === "IEND") break;
    offset = dataEnd + 4;
  }
  return chunks;
}

function parsePngTexts(chunks: { type: string; data: Uint8Array }[], inflated: Record<string, Uint8Array> = {}) {
  const result: Record<string, string> = {};
  for (const chunk of chunks) {
    const data = chunk.data;
    if (chunk.type === "tEXt" && data.length > 0) {
      const z = data.indexOf(0);
      if (z >= 0) result[latin1(data.subarray(0, z))] = utf8(data.subarray(z + 1));
    } else if (chunk.type === "iTXt" && data.length > 3) {
      const z = data.indexOf(0);
      if (z >= 0 && z + 2 < data.length) {
        const compressed = data[z + 1] !== 0;
        let cursor = z + 3;
        const langEnd = data.indexOf(0, cursor);
        if (langEnd < 0) continue;
        cursor = langEnd + 1;
        const transEnd = data.indexOf(0, cursor);
        if (transEnd < 0) continue;
        const payload = data.subarray(transEnd + 1);
        const key = latin1(data.subarray(0, z));
        if (!compressed) result[key] = utf8(payload);
        else inflated[key] = payload;
      }
    } else if (chunk.type === "zTXt" && data.length > 2) {
      const z = data.indexOf(0);
      if (z >= 0) inflated[latin1(data.subarray(0, z))] = data.subarray(z + 2);
    }
  }
  return result;
}

function parseJpegUserComment(buffer: ArrayBuffer): Record<string, string> {
  const bytes = new Uint8Array(buffer);
  if (bytes.length < 4 || bytes[0] !== 0xff || bytes[1] !== 0xd8) return {};
  let i = 2;
  const result: Record<string, string> = {};
  while (i + 4 < bytes.length && bytes[i] === 0xff) {
    const marker = bytes[i + 1];
    if (marker === 0xda) break;
    const size = (bytes[i + 2] << 8) | bytes[i + 3];
    const payload = bytes.subarray(i + 4, i + 2 + size);
    if (marker === 0xe1 && latin1(payload.subarray(0, 6)).startsWith("Exif")) {
      const text = utf8(payload);
      const ascii = text.match(/ASCII\x00{0,3}([\s\S]+)/);
      const uni = text.match(/UNICODE\x00([\s\S]+)/);
      if (ascii?.[1]) result.UserComment = ascii[1].replace(/\0/g, "").trim();
      if (uni?.[1]) result.UserComment = uni[1].replace(/\0/g, "").trim();
    }
    i += 2 + size;
  }
  return result;
}

function paeth(a: number, b: number, c: number) {
  const p = a + b - c;
  const pa = Math.abs(p - a);
  const pb = Math.abs(p - b);
  const pc = Math.abs(p - c);
  if (pa <= pb && pa <= pc) return a;
  if (pb <= pc) return b;
  return c;
}

async function decodePngPixels(chunks: { type: string; data: Uint8Array }[]) {
  const ihdr = chunks.find((c) => c.type === "IHDR")?.data;
  if (!ihdr || ihdr.length < 13) return null;
  const view = new DataView(ihdr.buffer, ihdr.byteOffset, ihdr.byteLength);
  const width = view.getUint32(0);
  const height = view.getUint32(4);
  const bitDepth = ihdr[8];
  const colorType = ihdr[9];
  const interlace = ihdr[12];
  if (bitDepth !== 8 || interlace !== 0 || width <= 0 || height <= 0) return null;
  const channels = colorType === 6 ? 4 : colorType === 2 ? 3 : colorType === 4 ? 2 : colorType === 0 ? 1 : 0;
  if (!channels) return null;
  const idat = chunks.filter((c) => c.type === "IDAT");
  if (!idat.length) return null;
  const joined = new Uint8Array(idat.reduce((n, c) => n + c.data.length, 0));
  let o = 0;
  for (const chunk of idat) {
    joined.set(chunk.data, o);
    o += chunk.data.length;
  }
  let inflated: Uint8Array;
  try {
    inflated = await inflateZlib(joined);
  } catch {
    return null;
  }
  const bpp = channels;
  const stride = width * bpp;
  const pixels = new Uint8Array(height * stride);
  let src = 0;
  let prev: Uint8Array | null = null;
  for (let y = 0; y < height; y++) {
    if (src + 1 + stride > inflated.length) return null;
    const filter = inflated[src++];
    const row = inflated.subarray(src, src + stride);
    src += stride;
    const dest = pixels.subarray(y * stride, (y + 1) * stride);
    for (let i = 0; i < stride; i++) {
      const x = row[i];
      const a = i >= bpp ? dest[i - bpp] : 0;
      const b = prev ? prev[i] : 0;
      const c = prev && i >= bpp ? prev[i - bpp] : 0;
      let val = x;
      if (filter === 1) val = (x + a) & 255;
      else if (filter === 2) val = (x + b) & 255;
      else if (filter === 3) val = (x + ((a + b) >> 1)) & 255;
      else if (filter === 4) val = (x + paeth(a, b, c)) & 255;
      dest[i] = val;
    }
    prev = dest;
  }
  return { width, height, channels, pixels };
}

function bitsToBytes(bits: string) {
  const out = new Uint8Array(Math.floor(bits.length / 8));
  for (let i = 0; i < out.length; i++) out[i] = Number.parseInt(bits.slice(i * 8, i * 8 + 8), 2);
  return out;
}

function readChannelBits(
  img: { width: number; height: number; channels: number; pixels: Uint8Array },
  mode: "alpha" | "rgb",
  count: number,
  columnMajor = true,
) {
  const { width, height, channels, pixels } = img;
  let bits = "";
  const walk = (x: number, y: number) => {
    const i = (y * width + x) * channels;
    if (mode === "alpha") {
      if (channels < 4) return false;
      bits += String(pixels[i + 3] & 1);
    } else {
      bits += String(pixels[i] & 1) + String(pixels[i + 1] & 1) + String(pixels[i + 2] & 1);
    }
    return bits.length >= count;
  };
  if (columnMajor) {
    for (let x = 0; x < width; x++) {
      for (let y = 0; y < height; y++) {
        if (walk(x, y)) return bits.slice(0, count);
      }
    }
  } else {
    for (let y = 0; y < height; y++) {
      for (let x = 0; x < width; x++) {
        if (walk(x, y)) return bits.slice(0, count);
      }
    }
  }
  return bits.length >= count ? bits.slice(0, count) : null;
}

async function readStealthPayload(img: { width: number; height: number; channels: number; pixels: Uint8Array }) {
  const sigBits = 15 * 8;
  const modes: Array<"alpha" | "rgb"> = img.channels === 4 ? ["alpha", "rgb"] : ["rgb"];
  for (const columnMajor of [true, false]) {
    for (const mode of modes) {
      const header = readChannelBits(img, mode, sigBits, columnMajor);
      if (!header) continue;
      const sig = utf8(bitsToBytes(header));
      if (!(STEALTH_SIGS as readonly string[]).includes(sig)) continue;
      const compressed = sig.endsWith("comp");
      const lenBits = readChannelBits(img, mode, sigBits + 32, columnMajor);
      if (!lenBits) continue;
      const paramLen = Number.parseInt(lenBits.slice(sigBits), 2);
      if (!Number.isFinite(paramLen) || paramLen <= 0 || paramLen > 8_000_000) continue;
      const all = readChannelBits(img, mode, sigBits + 32 + paramLen, columnMajor);
      if (!all) continue;
      const payload = bitsToBytes(all.slice(sigBits + 32));
      try {
        const bytes = compressed ? await inflateGzip(payload) : payload;
        const text = utf8(bytes).replace(/\0+$/, "").trim();
        if (text) return text;
      } catch {
        try {
          const text = utf8(payload).replace(/\0+$/, "").trim();
          if (text) return text;
        } catch {
          /* ignore */
        }
      }
    }
  }
  return "";
}

function mergeStealthText(meta: Record<string, string>, text: string) {
  if (!text) return;
  try {
    const parsed = JSON.parse(text) as unknown;
    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      const obj = parsed as Record<string, unknown>;
      if (typeof obj.Comment === "string") meta.Comment = obj.Comment;
      else if (obj.v4_prompt || obj.prompt || obj.seed != null) meta.Comment = text;
      if (typeof obj.Description === "string") meta.Description = obj.Description;
      if (typeof obj.Software === "string") meta.Software = obj.Software;
      if (typeof obj.Source === "string") meta.Source = obj.Source;
      if (!meta.Comment && !meta.Description) meta.Comment = text;
      if (!meta.Software) meta.Software = "NovelAI";
      return;
    }
  } catch {
    /* not json */
  }
  if (/negative prompt:/i.test(text) || /steps:/i.test(text)) meta.parameters = text;
  else meta.Description = meta.Description || text;
  meta.Software = meta.Software || "NovelAI";
}

function parseComment(raw: string): Record<string, unknown> {
  try {
    let value: unknown = JSON.parse(raw);
    if (typeof value === "string") value = JSON.parse(value);
    if (!value || typeof value !== "object" || Array.isArray(value)) return {};
    const obj = value as Record<string, unknown>;
    if (typeof obj.Comment === "string") {
      try {
        const inner = JSON.parse(obj.Comment);
        if (inner && typeof inner === "object" && !Array.isArray(inner)) {
          return { ...obj, ...(inner as Record<string, unknown>) };
        }
      } catch {
        /* keep outer */
      }
    }
    return obj;
  } catch {
    return {};
  }
}

function asString(value: unknown) {
  return typeof value === "string" ? value : "";
}

function asNum(value: unknown) {
  const n = typeof value === "number" ? value : typeof value === "string" ? Number(value) : NaN;
  return Number.isFinite(n) ? n : null;
}

function asObject(value: unknown): Record<string, unknown> | undefined {
  if (value && typeof value === "object" && !Array.isArray(value)) return value as Record<string, unknown>;
  if (typeof value === "string") {
    try {
      const parsed = JSON.parse(value) as unknown;
      if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) return parsed as Record<string, unknown>;
    } catch {
      /* ignore */
    }
  }
  return undefined;
}

function captionItems(value: unknown): unknown[] {
  const obj = asObject(value);
  if (Array.isArray(obj?.char_captions)) return obj.char_captions;
  if (Array.isArray(value)) return value;
  return [];
}

function parseCharacters(comment: Record<string, unknown>): CharCaption[] {
  const v4 = asObject(comment.v4_prompt);
  const v4neg = asObject(comment.v4_negative_prompt);
  const promptCap = asObject(v4?.caption) ?? v4;
  const negativeCap = asObject(v4neg?.caption) ?? v4neg;
  const positives = captionItems(promptCap).length ? captionItems(promptCap) : captionItems(comment.char_captions);
  const negatives = captionItems(negativeCap);
  const useCoords = Boolean(v4?.use_coords ?? comment.use_coords);
  return positives.flatMap((raw, index) => {
    const item = raw && typeof raw === "object" ? (raw as Record<string, unknown>) : {};
    const cap = asString(item.char_caption) || asString(item.caption) || asString(item.prompt);
    if (!cap.trim()) return [];
    const negItem = negatives[index] && typeof negatives[index] === "object" ? (negatives[index] as Record<string, unknown>) : {};
    const centers = Array.isArray(item.centers) ? item.centers : [];
    const center = centers[0] && typeof centers[0] === "object" ? (centers[0] as Record<string, unknown>) : {};
    return [
      {
        id: crypto.randomUUID(),
        prompt: cap,
        negativePrompt: asString(negItem.char_caption) || asString(negItem.caption),
        useCoords,
        x: asNum(center.x) ?? 0.5,
        y: asNum(center.y) ?? 0.5,
        enabled: true,
      },
    ];
  });
}

export function dataUrlToBuffer(dataUrl: string): ArrayBuffer {
  const raw = dataUrl.includes(",") ? dataUrl.slice(dataUrl.indexOf(",") + 1) : dataUrl;
  const bin = atob(raw);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i += 1) bytes[i] = bin.charCodeAt(i);
  return bytes.buffer;
}

function reportFromMeta(meta: Record<string, string>): MetadataReport {
  const software = pick(meta, ["Software", "software"]);
  const source = pick(meta, ["Source", "source"]);
  const description = pick(meta, ["Description", "description"]);
  const commentRaw = pick(meta, ["Comment", "comment"]);
  const parameters = pick(meta, ["parameters", "UserComment"]);
  const comment = parseComment(commentRaw);
  const v4 = asObject(comment.v4_prompt);
  const v4cap = asObject(v4?.caption);
  const v4neg = asObject(comment.v4_negative_prompt);
  const v4negCap = asObject(v4neg?.caption);
  let prompt = asString(v4cap?.base_caption) || asString(comment.prompt) || description;
  let negative = asString(v4negCap?.base_caption) || asString(comment.uc) || asString(comment.negative_prompt);
  if (!prompt && parameters) {
    const split = parameters.split(/\nNegative prompt:/i);
    prompt = (split[0] || "").trim();
    if (!negative && split[1]) negative = split[1].split("\n")[0]?.trim() || "";
  }
  const characterCaptions = parseCharacters(comment);
  const model =
    modelToNai(asString(comment.model)) ||
    modelToNai(source) ||
    modelToNai(software) ||
    source;
  const looksNovelAi =
    /novelai/i.test(`${software} ${source} ${commentRaw} ${description}`) ||
    Boolean(v4) ||
    Boolean(commentRaw);
  const hasMetadata =
    looksNovelAi ||
    Boolean(prompt.trim()) ||
    Boolean(negative.trim()) ||
    asNum(comment.seed) != null ||
    asNum(comment.steps) != null ||
    characterCaptions.length > 0;
  return {
    ...emptyMetadataReport(),
    kind: looksNovelAi ? "novelai" : parameters ? "stable-diffusion" : "unknown",
    software,
    prompt,
    negative,
    model,
    seed: asNum(comment.seed),
    width: asNum(comment.width),
    height: asNum(comment.height),
    steps: asNum(comment.steps),
    sampler: asString(comment.sampler),
    cfgScale: asNum(comment.scale ?? comment.cfg_scale),
    cfgRescale: asNum(comment.cfg_rescale),
    characterCaptions,
    hasMetadata,
    rawText: Object.entries(meta)
      .map(([k, v]) => `${k}: ${v}`)
      .join("\n\n"),
    entries: Object.entries(meta),
  };
}

export async function inspectImageBuffer(buffer: ArrayBuffer): Promise<MetadataReport> {
  const chunks = parsePngChunks(buffer);
  const compressed: Record<string, Uint8Array> = {};
  const meta = {
    ...(chunks.length ? {} : parseJpegUserComment(buffer)),
    ...parsePngTexts(chunks, compressed),
  };
  for (const [key, payload] of Object.entries(compressed)) {
    try {
      meta[key] = utf8(await inflateZlib(payload));
    } catch {
      /* ignore */
    }
  }
  let report = reportFromMeta(meta);
  const ihdr = chunks.find((c) => c.type === "IHDR")?.data;
  const mega = ihdr && ihdr.length >= 8
    ? (new DataView(ihdr.buffer, ihdr.byteOffset, ihdr.byteLength).getUint32(0)
        * new DataView(ihdr.buffer, ihdr.byteOffset, ihdr.byteLength).getUint32(4))
      / 1_000_000
    : 0;
  if (report.hasMetadata || mega > 3.2 || buffer.byteLength > 8 * 1024 * 1024) {
    return report;
  }
  const pixels = chunks.length ? await decodePngPixels(chunks) : null;
  if (pixels) {
    const stealth = await readStealthPayload(pixels);
    if (stealth) {
      mergeStealthText(meta, stealth);
      report = reportFromMeta(meta);
    }
  }
  return report;
}
