import { parseWeightedTag, splitPromptTags } from "@/utils/promptWeight";

const QUALITY = new Set([
  "masterpiece",
  "best quality",
  "high quality",
  "absurdres",
  "highres",
  "amazing quality",
  "very aesthetic",
  "aesthetic",
]);
const SUBJECT = /^(?:\d+girls?|\d+boys?|\d+others?|solo|multiple girls|multiple boys|no humans)\b/i;
const CONTENT = /\b(hair|eyes|background|looking|shirt|dress|skirt|smile|standing|sitting|sky|indoors|outdoors)\b/i;

function pieceText(tag: string) {
  return parseWeightedTag(tag).core.trim();
}

function isArtistCore(core: string) {
  const low = core.replace(/,+\s*$/g, "").trim().toLowerCase();
  if (!low || QUALITY.has(low) || SUBJECT.test(low) || CONTENT.test(low)) return false;
  if (/^year\s+20\d{2}$/.test(low)) return true;
  if (low.startsWith("artist:") || low.startsWith("by ")) return true;
  const words = low.split(/\s+/);
  return words.length <= 3 && low.length <= 40 && /^[\w][\w .'-]*$/i.test(low);
}

function isArtistPiece(tag: string) {
  const parsed = parseWeightedTag(tag);
  const parts = parsed.parts.map((part) => part.replace(/,+\s*$/g, "").trim()).filter(Boolean);
  if (!parts.length || !parts.every(isArtistCore)) return false;
  if (parsed.numeric != null) return true;
  return parts.every((part) => /^year\s+20\d{2}$/i.test(part) || /^(artist:|by\s)/i.test(part));
}

/** Leading artist / year weights from a generated prompt, such as `1::year 2025::, 0.4::letdie1414::`. */
export function extractArtistString(prompt: string) {
  const tags = splitPromptTags(prompt);
  const leading: string[] = [];
  for (const tag of tags) {
    if (!isArtistPiece(tag)) break;
    leading.push(tag.trim());
  }
  if (leading.length) return leading.join(", ");
  return tags.filter(isArtistPiece).join(", ");
}

export function artistBlockName(artist: string) {
  const name = splitPromptTags(artist)
    .map(pieceText)
    .find((core) => core && !/^year\s+20\d{2}$/i.test(core))
    ?.replace(/^artist:\s*/i, "")
    .trim();
  const short = (name || "画师串").slice(0, 24);
  return short === "画师串" ? short : `画师串 · ${short}`;
}

export function promptBlockName(positive: string) {
  const artist = extractArtistString(positive);
  if (artist) return artistBlockName(artist);
  const short = positive.trim().replace(/\s+/g, " ").slice(0, 24);
  return short ? `画师串 · ${short}` : "画师串";
}
