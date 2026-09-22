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

export function hasCjkText(text: string) {
  return /[\u4e00-\u9fff\u3040-\u30ff\uac00-\ud7af]/.test(text);
}

export function appendPromptChunk(value: string, chunk: string): string {
  const current = value.trim();
  const addition = chunk.trim().replace(/^,+\s*/, "").replace(/\s*,+$/, "");
  if (!addition) return value;
  if (!current) return `${addition}, `;
  return `${current.replace(/\s*,?\s*$/, "")}, ${addition}, `;
}
