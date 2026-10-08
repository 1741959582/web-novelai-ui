import { translateText } from "@/api/tauri";
import { parseWeightedTag, serializeWeightedTag, weightSpans } from "./promptWeight";
import { chunkForTranslate, hasCjkText, splitTranslatable, type TranslateLang } from "./textUtils";
import { lookupZhTag } from "./tagSuggest";

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
  return translateWeightedPrompt(text);
}

const LANGPAIR: Record<TranslateLang, string> = {
  zh: "zh-CN|en",
  ja: "ja|en",
  ko: "ko|en",
};

function usableTranslation(source: string, translated: string) {
  const next = translated.trim();
  if (!next || next === source || /MYMEMORY WARNING/i.test(next)) return "";
  return next;
}

async function translateChunk(source: string, lang: TranslateLang): Promise<string> {
  const pair = LANGPAIR[lang];
  for (let attempt = 0; attempt < 2; attempt += 1) {
    try {
      const translated = usableTranslation(source, await translateText(source, pair));
      if (translated) return translated;
    } catch {
      /* retry once, then keep the original chunk */
    }
    if (attempt === 0) await new Promise((resolve) => setTimeout(resolve, 350));
  }
  return "";
}

/** Translate Chinese, Japanese, and Korean into English, keeping weight syntax in place. */
export async function translateWeightedPrompt(text: string): Promise<{ text: string; note: string }> {
  if (!text.trim()) return { text, note: "提示词为空" };
  if (!hasCjkText(text)) return { text, note: "已经是英文，无需翻译" };

  let apiHit = 0;
  let dictHit = 0;
  let failed = 0;

  async function translateRun(source: string, lang: TranslateLang): Promise<string> {
    if (lang === "zh") {
      const mapped = lookupZhTag(source);
      if (mapped) {
        dictHit += 1;
        return mapped;
      }
    }
    const chunks = chunkForTranslate(source);
    const done: string[] = [];
    for (const chunk of chunks) {
      const translated = await translateChunk(chunk, lang);
      if (translated) {
        apiHit += 1;
        done.push(translated);
      } else {
        failed += 1;
        done.push(chunk);
      }
    }
    return done.join("");
  }

  async function tr(piece: string): Promise<string> {
    if (!hasCjkText(piece)) return piece;
    const parts = splitTranslatable(piece);
    const done = await Promise.all(
      parts.map((part) => (part.lang ? translateRun(part.text, part.lang) : Promise.resolve(part.text))),
    );
    return done.join("");
  }

  const out = await Promise.all(
    weightSpans(text).map(async (span) => {
      if (span.weight == null && !span.disabled) return tr(span.raw);
      const tag = parseWeightedTag(span.raw);
      if (!hasCjkText(tag.core)) return span.raw;
      const parts = tag.parts.length ? tag.parts : [tag.core];
      const next = await Promise.all(parts.map((part) => tr(part)));
      const body = serializeWeightedTag(next.join(", "), tag.level, tag.numeric);
      return span.disabled ? `~~${body}~~` : body;
    }),
  );
  const next = out.join("");
  if (failed && !apiHit && !dictHit) return { text, note: "翻译失败，可先用提词选英文标签" };
  if (!apiHit && !dictHit) return { text: next, note: "没有可翻译的中文、日文或韩文" };
  if (failed) return { text: next, note: "部分内容已译成英文" };
  return { text: next, note: "已译成英文" };
}
