function escapeHtml(value: string) {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

function inline(value: string) {
  let text = escapeHtml(value);
  text = text.replace(/`([^`]+)`/g, "<code>$1</code>");
  text = text.replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>");
  text = text.replace(/\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/g, '<a href="$2" target="_blank" rel="noopener noreferrer">$1</a>');
  return text;
}

function prettyCode(code: string) {
  const raw = code.replace(/\n$/, "");
  const trimmed = raw.trim();
  if ((trimmed.startsWith("{") && trimmed.endsWith("}")) || (trimmed.startsWith("[") && trimmed.endsWith("]"))) {
    try {
      return JSON.stringify(JSON.parse(trimmed), null, 2);
    } catch {
      /* 不是完整 JSON，按原文换行显示 */
    }
  }
  return raw;
}

function codeBlock(code: string) {
  const body = escapeHtml(prettyCode(code));
  return `<div class="codeblock"><button type="button" class="copy">复制</button><pre><code>${body}</code></pre></div>`;
}

function looksLikeJson(source: string) {
  const trimmed = source.trim();
  if (!trimmed || trimmed.includes("```")) return false;
  if (!((trimmed.startsWith("{") && trimmed.endsWith("}")) || (trimmed.startsWith("[") && trimmed.endsWith("]")))) return false;
  try {
    JSON.parse(trimmed);
    return true;
  } catch {
    return false;
  }
}

/** 把模型回复转成安全的 HTML。先转义，再还原少量 Markdown。 */
export function renderMarkdown(source: string) {
  let text = source ?? "";
  if (looksLikeJson(text)) return codeBlock(text);
  if (((text.match(/```/g) || []).length) % 2 === 1) text += "\n```";
  const blocks: string[] = [];
  text = text.replace(/```[\w-]*\n?([\s\S]*?)```/g, (_, code: string) => {
    blocks.push(codeBlock(String(code)));
    return `\n@@BLOCK${blocks.length - 1}@@\n`;
  });

  const html: string[] = [];
  let list: string[] | null = null;
  const flush = () => {
    if (!list) return;
    html.push(`<ul>${list.map((item) => `<li>${inline(item)}</li>`).join("")}</ul>`);
    list = null;
  };

  for (const line of text.split("\n")) {
    const block = /^@@BLOCK(\d+)@@$/.exec(line.trim());
    if (block) {
      flush();
      html.push(blocks[Number(block[1])] ?? "");
      continue;
    }
    const item = /^\s*[-*]\s+(.*)$/.exec(line);
    if (item) {
      list = list ?? [];
      list.push(item[1]);
      continue;
    }
    flush();
    if (!line.trim()) continue;
    const heading = /^(#{1,3})\s+(.*)$/.exec(line);
    if (heading) {
      const level = heading[1].length;
      html.push(`<h${level}>${inline(heading[2])}</h${level}>`);
      continue;
    }
    html.push(`<p>${inline(line)}</p>`);
  }
  flush();
  return html.join("");
}
