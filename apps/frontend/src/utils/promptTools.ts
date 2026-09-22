import { translateText } from "@/api/tauri";
import { hasCjkText } from "./textUtils";
import { lookupZhTag, translateTagsByDict } from "./tagSuggest";

const LS_PRESETS = "nai-positive-presets";
const LS_CHUNKS = "nai-prompt-chunks";
const LS_AC = "nai-autocomplete";

export interface PromptPreset {
  id: string;
  name: string;
  prompt: string;
}

export interface PromptChunk {
  id: string;
  name: string;
  content: string;
}

function uid() {
  return globalThis.crypto?.randomUUID?.() ?? `p-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
}

function readJson<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return fallback;
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

export function loadPresets(): PromptPreset[] {
  return readJson(LS_PRESETS, []);
}

export function savePresets(list: PromptPreset[]) {
  localStorage.setItem(LS_PRESETS, JSON.stringify(list));
}

export function loadChunks(): PromptChunk[] {
  return readJson(LS_CHUNKS, []);
}

export function saveChunks(list: PromptChunk[]) {
  localStorage.setItem(LS_CHUNKS, JSON.stringify(list));
}

export function loadAutoComplete(): boolean {
  const raw = localStorage.getItem(LS_AC);
  return raw !== "0";
}

export function saveAutoComplete(on: boolean) {
  localStorage.setItem(LS_AC, on ? "1" : "0");
}

export function makePreset(name: string, prompt: string): PromptPreset {
  return { id: uid(), name: name.trim() || "未命名预设", prompt };
}

export function makeChunk(name: string, content: string): PromptChunk {
  return { id: uid(), name: name.trim() || "未命名提词", content };
}

export async function translatePromptToEnglish(text: string): Promise<{ text: string; note: string }> {
  const trimmed = text.trim();
  if (!trimmed) return { text, note: "提示词为空" };
  if (!hasCjkText(trimmed)) return { text, note: "已经是英文，无需翻译" };

  const dict = translateTagsByDict(text);
  let next = dict.text;
  const segs = next.split(",");
  let apiHit = 0;
  let failed = 0;
  const out = await Promise.all(
    segs.map(async (seg) => {
      const piece = seg.trim();
      if (!piece || !hasCjkText(piece)) return seg;
      const mapped = lookupZhTag(piece);
      if (mapped) return seg.replace(piece, mapped);
      try {
        const translated = (await translateText(piece)).trim();
        if (translated && translated !== piece) {
          apiHit += 1;
          return seg.replace(piece, translated);
        }
      } catch {
        failed += 1;
      }
      return seg;
    }),
  );
  next = out.join(",");
  if (!hasCjkText(next) || apiHit || dict.hit) {
    if (!next.endsWith(",") && !next.endsWith(", ")) next = `${next}, `;
    if (failed) return { text: next, note: "部分词条已译成英文" };
    return { text: next, note: "已自动检测并译成英文" };
  }
  return { text: next, note: failed ? "翻译失败，可先用提词选英文标签" : "没有可翻译的中文词条" };
}
