const WORD_CHAR = /[\w㐀-鿿-]/;

export function wordAtCursor(text: string, cursor: number): { word: string; start: number } {
  let s = cursor;
  while (s > 0 && WORD_CHAR.test(text[s - 1])) s--;
  return { word: text.slice(s, cursor), start: s };
}

export function fmtCount(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(0)}k`;
  return String(n);
}

const HAN = /[\u4e00-\u9fff]/;
const KANA = /[\u3040-\u30ff\uff66-\uff9d]/;
const HANGUL = /[\uac00-\ud7af\u1100-\u11ff\u3130-\u318f]/;

export function hasCjkText(text: string) {
  return HAN.test(text) || KANA.test(text) || HANGUL.test(text);
}

/** Language pair for a tag gloss. Chinese text is already the target language. */
export function pairToChinese(text: string): string | null {
  if (HANGUL.test(text)) return "ko|zh-CN";
  if (KANA.test(text)) return "ja|zh-CN";
  if (HAN.test(text) && !/[A-Za-z]/.test(text)) return null;
  if (/[A-Za-z]/.test(text)) return "en|zh-CN";
  return null;
}

export type TranslateLang = "zh" | "ja" | "ko";

export interface TranslatePiece {
  lang: TranslateLang | null;
  text: string;
}

function charKind(ch: string): "han" | "kana" | "ko" | "other" {
  if (HANGUL.test(ch)) return "ko";
  if (KANA.test(ch)) return "kana";
  if (HAN.test(ch)) return "han";
  return "other";
}

function isGlue(text: string) {
  return /^[\s,，.。!！?？;；:：、~～…·\-—–()（）[\]【】「」『』""''“”‘’]+$/.test(text);
}

/** Split text so Chinese, Japanese, and Korean can be translated separately from English. */
export function splitTranslatable(text: string): TranslatePiece[] {
  type Kind = "han" | "kana" | "ko" | "other";
  const atoms: { kind: Kind; text: string }[] = [];
  for (const ch of text) {
    const kind = charKind(ch);
    const last = atoms[atoms.length - 1];
    if (last && last.kind === kind) last.text += ch;
    else atoms.push({ kind, text: ch });
  }

  const langs: Array<TranslateLang | null> = atoms.map((atom) => {
    if (atom.kind === "ko") return "ko";
    if (atom.kind === "kana") return "ja";
    if (atom.kind === "han") return "zh";
    return null;
  });
  for (let i = 0; i < atoms.length; i += 1) {
    if (atoms[i].kind !== "han") continue;
    const prev = i > 0 ? atoms[i - 1].kind : "";
    const next = i + 1 < atoms.length ? atoms[i + 1].kind : "";
    if (prev === "kana" || next === "kana") langs[i] = "ja";
  }
  for (let i = 0; i < atoms.length; i += 1) {
    if (atoms[i].kind !== "other" || !isGlue(atoms[i].text)) continue;
    const prev = i > 0 ? langs[i - 1] : null;
    const next = i + 1 < langs.length ? langs[i + 1] : null;
    if (prev && prev === next) langs[i] = prev;
  }

  const out: TranslatePiece[] = [];
  for (let i = 0; i < atoms.length; i += 1) {
    const lang = langs[i];
    const last = out[out.length - 1];
    if (last && last.lang === lang) last.text += atoms[i].text;
    else out.push({ lang, text: atoms[i].text });
  }
  return out;
}

/** Keep each piece under MyMemory's 500-byte query limit. */
export function chunkForTranslate(text: string, maxBytes = 420): string[] {
  const size = (value: string) => new TextEncoder().encode(value).length;
  if (size(text) <= maxBytes) return [text];
  const bits = text.split(/([。！？!?；;\n]+)/);
  const sentences: string[] = [];
  for (let i = 0; i < bits.length; i += 2) {
    sentences.push(`${bits[i] || ""}${bits[i + 1] || ""}`);
  }
  const out: string[] = [];
  let buf = "";
  const pushBuf = () => {
    if (!buf) return;
    out.push(buf);
    buf = "";
  };
  for (const sentence of sentences) {
    if (!sentence) continue;
    if (size(sentence) > maxBytes) {
      pushBuf();
      let rest = sentence;
      while (rest) {
        let take = rest.length;
        while (take > 1 && size(rest.slice(0, take)) > maxBytes) take -= 1;
        out.push(rest.slice(0, take));
        rest = rest.slice(take);
      }
      continue;
    }
    if (buf && size(buf + sentence) > maxBytes) pushBuf();
    buf += sentence;
  }
  pushBuf();
  return out;
}

export function appendPromptChunk(value: string, chunk: string): string {
  const current = value.trim();
  const addition = chunk.trim().replace(/^,+\s*/, "").replace(/\s*,+$/, "");
  if (!addition) return value;
  if (!current) return `${addition}, `;
  return `${current.replace(/\s*,?\s*$/, "")}, ${addition}, `;
}
