// Helpers for the per-tag weight editor.
//
// NovelAI accepts two emphasis styles:
//   {tag} / [tag]  — each layer is about ×1.05 / ÷1.05
//   2::tag::       — numeric emphasis (V4+); commas inside the pair stay grouped
//   2::tag1, tag2:: — several tags share one number
//
// We model a segment as { core, parts, level, numeric }.

export interface WeightedTag {
  /** Inner text with braces stripped. Groups keep "tag1, tag2". */
  core: string;
  /** Individual tags inside a numeric group (or a single core). */
  parts: string[];
  /** Signed brace level: +n => n×"{}", -n => n×"[]". Ignored when numeric is set. */
  level: number;
  /** Number before `::`, e.g. 2 in `2::tag::`. null = brace / plain mode. */
  numeric: number | null;
  /** Raw original segment. */
  raw: string;
}

const PER_BRACE = 1.05;
const NUMERIC_STEP = 0.1;
const NUMERIC_MIN = -10;
const NUMERIC_MAX = 10;
const NUMERIC_OPEN_RE = /^(-?\d+(?:\.\d+)?)\s*::/;
const NUMERIC_FULL_RE = /^(-?\d+(?:\.\d+)?)\s*::\s*([\s\S]*?)\s*::$/;

export type MergeStyle = "brace" | "numeric";

/** Split a prompt into segments, keeping `n::a, b::` and `{a, b}` as one piece. */
export function splitPromptTags(prompt: string): string[] {
  const segs: string[] = [];
  const s = prompt;
  let i = 0;
  while (i < s.length) {
    while (i < s.length && (s[i] === "," || /\s/.test(s[i]))) i += 1;
    if (i >= s.length) break;

    const open = s.slice(i).match(NUMERIC_OPEN_RE);
    if (open) {
      const afterOpen = i + open[0].length;
      const close = s.indexOf("::", afterOpen);
      if (close !== -1) {
        segs.push(s.slice(i, close + 2).trim());
        i = close + 2;
        continue;
      }
    }

    let j = i;
    let depth = 0;
    while (j < s.length) {
      const ch = s[j];
      if (ch === "{" || ch === "[") depth += 1;
      else if ((ch === "}" || ch === "]") && depth > 0) depth -= 1;
      else if (ch === "," && depth === 0) break;
      j += 1;
    }
    const piece = s.slice(i, j).trim();
    if (piece) segs.push(piece);
    i = j;
  }
  return segs;
}

function splitInnerParts(core: string): string[] {
  return core
    .split(",")
    .map((t) => t.trim())
    .filter((t) => t.length > 0);
}

function tidyCore(core: string): string {
  return splitInnerParts(core).join(", ");
}

/** Parse one segment into braces and/or numeric emphasis. */
export function parseWeightedTag(raw: string): WeightedTag {
  const trimmed = raw.trim();
  const numeric = trimmed.match(NUMERIC_FULL_RE);
  if (numeric) {
    const core = tidyCore(numeric[2]);
    return {
      core,
      parts: splitInnerParts(core),
      level: 0,
      numeric: Number(numeric[1]),
      raw: trimmed,
    };
  }

  let s = trimmed;
  let level = 0;
  for (;;) {
    if (s.length >= 2 && s.startsWith("{") && s.endsWith("}")) {
      s = s.slice(1, -1).trim();
      level += 1;
    } else if (s.length >= 2 && s.startsWith("[") && s.endsWith("]")) {
      s = s.slice(1, -1).trim();
      level -= 1;
    } else {
      break;
    }
  }
  return { core: s, parts: splitInnerParts(s), level, numeric: null, raw: trimmed };
}

export function formatNumeric(n: number): string {
  const rounded = Math.round(n * 10) / 10;
  if (Object.is(rounded, -0) || rounded === 0) return "0";
  return String(rounded);
}

function clampNumeric(n: number): number {
  return Math.max(NUMERIC_MIN, Math.min(NUMERIC_MAX, Math.round(n * 10) / 10));
}

function clampLevel(level: number): number {
  return Math.max(-5, Math.min(5, level));
}

/** Render a tag back into a prompt segment. */
export function serializeWeightedTag(core: string, level: number, numeric: number | null = null): string {
  const c = tidyCore(core);
  if (!c) return "";
  if (numeric != null) return `${formatNumeric(numeric)}::${c}::`;
  if (level > 0) return "{".repeat(level) + c + "}".repeat(level);
  if (level < 0) return "[".repeat(-level) + c + "]".repeat(-level);
  return c;
}

function writeTag(tag: Pick<WeightedTag, "core" | "level" | "numeric">): string {
  return serializeWeightedTag(tag.core, tag.level, tag.numeric);
}

function joinSegs(segs: string[]): string {
  return segs.filter(Boolean).join(", ");
}

/** Approximate strength multiplier for a brace level (1.00, 1.05, 0.95, ...). */
export function weightMultiplier(level: number): number {
  return Math.pow(PER_BRACE, level);
}

/** Human-readable brace multiplier like "×1.16" / "×0.91" / "" for neutral. */
export function formatMultiplier(level: number): string {
  if (level === 0) return "";
  return `×${weightMultiplier(level).toFixed(2)}`;
}

/** Weight label for a chip: `2` / `1.1` in :: mode, `×1.05` for braces. */
export function formatTagWeight(tag: WeightedTag): string {
  if (tag.numeric != null) return formatNumeric(tag.numeric);
  if (tag.level === 0) return "×1.00";
  return `×${weightMultiplier(tag.level).toFixed(2)}`;
}

function braceToNumeric(level: number): number {
  if (level === 0) return 1.1;
  return clampNumeric(weightMultiplier(level));
}

function numericToBrace(n: number): number {
  if (!Number.isFinite(n) || n <= 0) return n < 0 ? -1 : 1;
  const level = Math.round(Math.log(n) / Math.log(PER_BRACE));
  if (level === 0) return n < 1 ? -1 : 1;
  return clampLevel(level);
}

function mergeWeight(
  tags: WeightedTag[],
  anchor: WeightedTag,
  style: MergeStyle,
): Pick<WeightedTag, "core" | "level" | "numeric"> {
  const core = tags.flatMap((t) => t.parts).join(", ");
  if (style === "numeric") {
    const numeric =
      anchor.numeric ??
      (anchor.level !== 0 ? braceToNumeric(anchor.level) : 1.1);
    return { core, level: 0, numeric };
  }
  const level =
    anchor.numeric != null
      ? numericToBrace(anchor.numeric)
      : anchor.level !== 0
        ? anchor.level
        : 1;
  return { core, level, numeric: null };
}

/** Change one tag's text. Empty text deletes it. Weight / :: style is kept. */
export function renameTagInPrompt(prompt: string, index: number, nextText: string): string {
  const segs = splitPromptTags(prompt);
  if (index < 0 || index >= segs.length) return prompt;
  const core = nextText.trim();
  if (!core) return removeTagFromPrompt(prompt, index);
  const tag = parseWeightedTag(segs[index]);
  segs[index] = writeTag({ ...tag, core });
  return joinSegs(segs);
}

/** Change one keyword inside a merged group. Empty text removes that keyword. */
export function renamePartInGroup(prompt: string, index: number, partIndex: number, nextText: string): string {
  const segs = splitPromptTags(prompt);
  if (index < 0 || index >= segs.length) return prompt;
  const tag = parseWeightedTag(segs[index]);
  if (partIndex < 0 || partIndex >= tag.parts.length) return prompt;
  const parts = [...tag.parts];
  const core = nextText.trim();
  if (!core) {
    if (parts.length < 2) return removeTagFromPrompt(prompt, index);
    parts.splice(partIndex, 1);
  } else {
    parts[partIndex] = core;
  }
  segs[index] = writeTag({ ...tag, core: parts.join(", ") });
  return joinSegs(segs);
}

export function removeTagFromPrompt(prompt: string, index: number): string {
  const segs = splitPromptTags(prompt);
  if (index < 0 || index >= segs.length) return prompt;
  segs.splice(index, 1);
  return segs.length ? `${joinSegs(segs)}, ` : "";
}

export function setTagLevelInPrompt(prompt: string, index: number, level: number): string {
  const segs = splitPromptTags(prompt);
  if (index < 0 || index >= segs.length) return prompt;
  const tag = parseWeightedTag(segs[index]);
  segs[index] = writeTag({ ...tag, level: clampLevel(level), numeric: null });
  return joinSegs(segs);
}

/** − / + : braces change by 1 layer; `n::tag::` changes the number by 0.1. */
export function bumpTagWeight(prompt: string, index: number, delta: number): string {
  const segs = splitPromptTags(prompt);
  if (index < 0 || index >= segs.length) return prompt;
  const tag = parseWeightedTag(segs[index]);
  if (tag.numeric != null) {
    segs[index] = writeTag({ ...tag, numeric: clampNumeric(tag.numeric + delta * NUMERIC_STEP) });
  } else {
    segs[index] = writeTag({ ...tag, level: clampLevel(tag.level + delta) });
  }
  return joinSegs(segs);
}

export function resetTagWeight(prompt: string, index: number): string {
  const segs = splitPromptTags(prompt);
  if (index < 0 || index >= segs.length) return prompt;
  const tag = parseWeightedTag(segs[index]);
  if (tag.numeric != null) {
    segs[index] = writeTag({ ...tag, numeric: 1 });
  } else {
    segs[index] = writeTag({ ...tag, level: 0 });
  }
  return joinSegs(segs);
}

/** Switch a tag between `{tag}` / plain and `1.1::tag::`. Groups split when leaving ::. */
export function toggleNumericEmphasis(prompt: string, index: number): string {
  const segs = splitPromptTags(prompt);
  if (index < 0 || index >= segs.length) return prompt;
  const tag = parseWeightedTag(segs[index]);
  if (tag.numeric != null) {
    const plain = tag.parts.length ? tag.parts : [tag.core];
    segs.splice(index, 1, ...plain);
  } else {
    segs[index] = writeTag({ core: tag.core, level: 0, numeric: braceToNumeric(tag.level) });
  }
  return joinSegs(segs);
}

/** Merge this tag with the next one. */
export function mergeTagWithNext(prompt: string, index: number, style: MergeStyle = "numeric"): string {
  return mergeTags(prompt, [index, index + 1], style, index);
}

/**
 * Merge any selected tags into one group at `anchorIndex` (or the first selected).
 * `brace` → `{a, b}` / `[a, b]`; `numeric` → `1.1::a, b::`.
 */
export function mergeTags(
  prompt: string,
  indices: number[],
  style: MergeStyle,
  anchorIndex?: number,
): string {
  const segs = splitPromptTags(prompt);
  const unique = [...new Set(indices)]
    .filter((i) => i >= 0 && i < segs.length)
    .sort((a, b) => a - b);
  if (unique.length < 2) return prompt;
  const anchor = anchorIndex != null && unique.includes(anchorIndex) ? anchorIndex : unique[0];
  const tags = unique.map((i) => parseWeightedTag(segs[i]));
  const written = writeTag(mergeWeight(tags, parseWeightedTag(segs[anchor]), style));
  const next = [...segs];
  let insertAt = anchor;
  for (const i of unique.filter((i) => i !== anchor).sort((a, b) => b - a)) {
    next.splice(i, 1);
    if (i < insertAt) insertAt -= 1;
  }
  next[insertAt] = written;
  return joinSegs(next);
}

/** Pull one keyword out of `{a, b, c}` / `n::a, b, c::`. The rest stay grouped. */
export function extractPartFromGroup(prompt: string, index: number, partIndex: number): string {
  const segs = splitPromptTags(prompt);
  if (index < 0 || index >= segs.length) return prompt;
  const tag = parseWeightedTag(segs[index]);
  if (partIndex < 0 || partIndex >= tag.parts.length || tag.parts.length < 2) return prompt;
  const parts = [...tag.parts];
  const [taken] = parts.splice(partIndex, 1);
  const rest = writeTag(
    tag.numeric != null
      ? { core: parts.join(", "), level: 0, numeric: tag.numeric }
      : { core: parts.join(", "), level: tag.level, numeric: null },
  );
  segs.splice(index, 1, rest, taken);
  return joinSegs(segs);
}

/** Extract one keyword from a group, then merge it into another tag. */
export function extractPartAndMerge(
  prompt: string,
  groupIndex: number,
  partIndex: number,
  targetIndex: number,
  style: MergeStyle,
): string {
  if (targetIndex === groupIndex) return extractPartFromGroup(prompt, groupIndex, partIndex);
  const next = extractPartFromGroup(prompt, groupIndex, partIndex);
  if (next === prompt) return prompt;
  const extractedAt = groupIndex + 1;
  const target = targetIndex > groupIndex ? targetIndex + 1 : targetIndex;
  return mergeTags(next, [extractedAt, target], style, target);
}

/** Split `{a, b}` or `n::a, b::` back into one tag per part, keeping the same style. */
export function splitNumericGroup(prompt: string, index: number): string {
  const segs = splitPromptTags(prompt);
  if (index < 0 || index >= segs.length) return prompt;
  const tag = parseWeightedTag(segs[index]);
  if (tag.parts.length < 2) return prompt;
  segs.splice(
    index,
    1,
    ...tag.parts.map((part) =>
      writeTag(
        tag.numeric != null
          ? { core: part, level: 0, numeric: tag.numeric }
          : { core: part, level: tag.level, numeric: null },
      ),
    ),
  );
  return joinSegs(segs);
}
