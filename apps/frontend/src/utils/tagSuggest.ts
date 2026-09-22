import { CAPSULE_EXTRA, CAPSULE_EXTRA2, CAPSULE_EXTRA3 } from "@/data/capsule-data";

export interface TagHit {
  tag: string;
  zh: string;
  category: string;
  count?: number;
  color?: string;
}

export const DANBOORU_CAT: Record<number, { label: string; color: string }> = {
  0: { label: "通用", color: "#4ade80" },
  1: { label: "画师", color: "#fb923c" },
  3: { label: "作品", color: "#a78bfa" },
  4: { label: "角色", color: "#60a5fa" },
  5: { label: "元信息", color: "#94a3b8" },
};

const CAT_COLOR: Record<string, string> = {
  人物: "#f472b6",
  发型发色: "#fb7185",
  五官妆容: "#38bdf8",
  表情情绪: "#fbbf24",
  服装: "#c084fc",
  场景: "#34d399",
  构图光影: "#60a5fa",
  质量画质: "#a3e635",
  动作姿势: "#fb923c",
  道具物品: "#22d3ee",
  特效: "#818cf8",
  生物: "#4ade80",
  风格画风: "#e879f9",
  魔法奇幻: "#f472b6",
};

const ALL_TAGS: TagHit[] = [];
const ZH_TO_EN = new Map<string, string>();

for (const group of [CAPSULE_EXTRA, CAPSULE_EXTRA2, CAPSULE_EXTRA3]) {
  for (const cat of group) {
    for (const sub of cat.subgroups) {
      for (const tag of sub.tags) {
        ALL_TAGS.push({ tag: tag.en, zh: tag.zh, category: cat.name });
        if (tag.zh && !ZH_TO_EN.has(tag.zh)) ZH_TO_EN.set(tag.zh, tag.en.replace(/_/g, " "));
      }
    }
  }
}

export function categoryColor(name: string) {
  return CAT_COLOR[name] ?? "#94a3b8";
}

export function danbooruCategory(id: number) {
  return DANBOORU_CAT[id] ?? { label: "通用", color: "#94a3b8" };
}

export function lookupZhTag(zh: string) {
  return ZH_TO_EN.get(zh.trim()) ?? "";
}

function norm(s: string) {
  return s.toLowerCase().replace(/_/g, " ").trim();
}

export function suggestTags(query: string, limit = 8): TagHit[] {
  const q = norm(query);
  if (!q) return [];
  const zh = /[\u4e00-\u9fff]/.test(query);
  const scored: Array<TagHit & { score: number }> = [];
  for (const item of ALL_TAGS) {
    const en = norm(item.tag);
    const label = item.zh;
    let score = 0;
    if (en === q || label === query) score = 120;
    else if (en.startsWith(q)) score = 100;
    else if (zh && label.startsWith(query)) score = 95;
    else if (en.includes(q)) score = 70;
    else if (zh && label.includes(query)) score = 65;
    if (score) scored.push({ ...item, score });
  }
  scored.sort((a, b) => b.score - a.score || a.tag.length - b.tag.length);
  const seen = new Set<string>();
  const out: TagHit[] = [];
  for (const item of scored) {
    if (seen.has(item.tag)) continue;
    seen.add(item.tag);
    out.push({ tag: item.tag.replace(/_/g, " "), zh: item.zh, category: item.category });
    if (out.length >= limit) break;
  }
  return out;
}

export function translateTagsByDict(text: string): { text: string; hit: number } {
  const segs = text.split(",");
  let hit = 0;
  const next = segs.map((raw) => {
    const trimmed = raw.trim();
    if (!trimmed) return raw;
    const mapped = ZH_TO_EN.get(trimmed);
    if (mapped) {
      hit += 1;
      return raw.replace(trimmed, mapped);
    }
    return raw;
  });
  return { text: next.join(","), hit };
}
