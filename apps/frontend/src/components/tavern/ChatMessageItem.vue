<script setup lang="ts">
import { computed, ref } from "vue";
import { useRouter } from "vue-router";
import type { ChatMessage, DraftChar } from "@/api/tavern";
import { revealInFolder } from "@/api/tauri";
import { useAppStore } from "@/stores/app";
import { msgText, useTavernStore } from "@/stores/tavern";

const props = defineProps<{ message: ChatMessage; isLast: boolean }>();
const emit = defineEmits<{ zoom: [url: string] }>();

const tavern = useTavernStore();
const app = useAppStore();
const router = useRouter();

const m = computed(() => props.message);
const isUser = computed(() => m.value.role === "user");
const text = computed(() => msgText(m.value));
const reasoning = computed(() => m.value.reasoning?.[m.value.swipeIdx] ?? "");
const streaming = computed(() => tavern.busy && props.isLast && !isUser.value);
const name = computed(() => (isUser.value ? tavern.config.userName || "User" : tavern.activeCard?.name || "AI"));
const avatar = computed(() => (isUser.value ? "" : tavern.activeCard?.avatar || ""));
const editing = ref(false);
const editText = ref("");
const showDraft = ref(true);

function escapeHtml(s: string) {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

/** 轻量渲染：*动作* 斜体、“对白”高亮、换行 */
function render(s: string) {
  return escapeHtml(s)
    .replace(/\*\*([^*\n]+)\*\*/g, "<b>$1</b>")
    .replace(/\*([^*\n]+)\*/g, "<em>$1</em>")
    .replace(/(“[^”\n]*”|「[^」\n]*」|"[^"\n]*")/g, '<span class="q">$1</span>')
    .replace(/\n/g, "<br>");
}

function startEdit() {
  editText.value = text.value;
  editing.value = true;
}

function saveEdit() {
  tavern.editMessage(m.value, editText.value);
  editing.value = false;
}

function remove() {
  if (window.confirm("删除这条消息？")) tavern.deleteMessage(m.value);
}

function removeImage(id: string) {
  if (window.confirm("从对话中移除这张图？（图片文件仍保留在输出目录）")) tavern.removeImage(m.value, id);
}

async function toI2i(path: string) {
  await app.usePathAs("i2i", path);
  await router.push("/");
}

function addChar() {
  if (!m.value.draft) return;
  m.value.draft.chars.push({ presetId: "", name: "other", tags: "", x: 0.5, y: 0.5 });
}

function removeChar(c: DraftChar) {
  if (!m.value.draft) return;
  m.value.draft.chars = m.value.draft.chars.filter((x) => x !== c);
}

function setPreset(c: DraftChar, id: string) {
  c.presetId = id;
  c.name = tavern.state.charPresets.find((p) => p.id === id)?.name ?? "other";
}

function copyPrompt() {
  if (!m.value.draft) return;
  const req = tavern.buildNaiRequest(m.value.draft);
  const lines = [
    req.stylePrompt && `画风: ${req.stylePrompt}`,
    `场景: ${req.positivePrompt}`,
    ...req.charCaptions.map((c, i) => `角色${i + 1}: ${c.prompt}`),
    req.negativePrompt && `负面: ${req.negativePrompt}`,
  ].filter(Boolean);
  void navigator.clipboard.writeText(lines.join("\n"));
  tavern.notice = "提示词已复制";
}
</script>

<template>
  <article class="msg" :class="{ user: isUser }">
    <div class="ava">
      <img v-if="avatar" :src="avatar" alt="" />
      <span v-else>{{ name.slice(0, 1) }}</span>
    </div>
    <div class="body">
      <header>
        <b>{{ name }}</b>
        <span class="tools">
          <button v-if="!isUser" type="button" title="构思插图" :disabled="m.draftState === 'loading' || streaming" @click="tavern.planImage(m)">🎨</button>
          <button v-if="!isUser" type="button" title="手写插图提示词" @click="tavern.manualDraft(m)">✍️</button>
          <button type="button" title="编辑" :disabled="streaming" @click="startEdit">✏️</button>
          <button type="button" title="删除" :disabled="streaming" @click="remove">🗑️</button>
        </span>
      </header>

      <details v-if="reasoning && tavern.config.showReasoning" class="think">
        <summary>思考过程</summary>
        <pre>{{ reasoning }}</pre>
      </details>

      <div v-if="editing" class="edit">
        <textarea v-model="editText" rows="6" />
        <div class="row-btns">
          <button type="button" class="btn primary" @click="saveEdit">保存</button>
          <button type="button" class="btn" @click="editing = false">取消</button>
        </div>
      </div>
      <div v-else class="text">
        <span v-html="render(text)" />
        <span v-if="streaming" class="caret">▍</span>
      </div>

      <nav v-if="!isUser && (m.swipes.length > 1 || isLast)" class="swipes">
        <button type="button" :disabled="m.swipeIdx === 0 || tavern.busy" @click="tavern.swipe(m, -1)">‹</button>
        <span>{{ m.swipeIdx + 1 }} / {{ m.swipes.length }}</span>
        <button type="button" :disabled="tavern.busy || (!isLast && m.swipeIdx >= m.swipes.length - 1)" :title="isLast && m.swipeIdx >= m.swipes.length - 1 ? '生成新回复' : ''" @click="tavern.swipe(m, 1)">›</button>
      </nav>

      <div v-if="m.images.length" class="images">
        <figure v-for="img in m.images" :key="img.id">
          <img v-if="tavern.imageUrls[img.path]" :src="tavern.imageUrls[img.path]" alt="" @click="emit('zoom', tavern.imageUrls[img.path])" />
          <div v-else class="img-missing" @click="tavern.ensureImage(img.path)">图片加载中 / 已丢失</div>
          <figcaption>
            <button type="button" title="送到生成页做图生图" @click="toI2i(img.path)">图生图</button>
            <button type="button" title="在文件夹中显示" @click="revealInFolder(img.path)">打开位置</button>
            <button type="button" @click="removeImage(img.id)">移除</button>
          </figcaption>
        </figure>
      </div>

      <section v-if="m.draftState" class="draft">
        <p v-if="m.draftState === 'loading'" class="hint">AI 正在构思画面…</p>
        <template v-else>
          <p v-if="m.draftError" class="err">{{ m.draftError }}</p>
          <details v-if="m.draft" :open="showDraft && !m.images.length" class="draft-box" @toggle="showDraft = ($event.target as HTMLDetailsElement).open">
            <summary>插图提示词<span v-if="m.draftState === 'generating'" class="hint"> · NovelAI 生成中…</span></summary>
            <div class="grid2">
              <label>构图
                <select v-model="m.draft.orientation">
                  <option value="portrait">竖图</option>
                  <option value="landscape">横图</option>
                  <option value="square">方图</option>
                </select>
              </label>
              <label>额外负面<input v-model="m.draft.negative" placeholder="可留空" /></label>
            </div>
            <label class="full">场景 / 构图 / 背景
              <textarea v-model="m.draft.scene" rows="2" placeholder="1girl, solo, cowboy shot, classroom, sunset, ..." />
            </label>
            <div v-for="(c, i) in m.draft.chars" :key="i" class="char-row">
              <select :value="c.presetId" @change="setPreset(c, ($event.target as HTMLSelectElement).value)">
                <option value="">临时角色（other）</option>
                <option v-for="p in tavern.state.charPresets" :key="p.id" :value="p.id">{{ p.name }}</option>
              </select>
              <input v-model="c.tags" :placeholder="c.presetId ? '此刻的表情、动作…' : '完整外貌 + 动作'" />
              <input v-model.number="c.x" class="num" type="number" step="0.1" min="0.1" max="0.9" title="横向位置 0~1" />
              <input v-model.number="c.y" class="num" type="number" step="0.1" min="0.1" max="0.9" title="纵向位置 0~1" />
              <button type="button" class="x" @click="removeChar(c)">×</button>
            </div>
            <div class="row-btns">
              <button type="button" class="btn" @click="addChar">+ 角色</button>
              <span class="grow" />
              <button type="button" class="btn" @click="copyPrompt">复制提示词</button>
              <button type="button" class="btn" :disabled="m.draftState === 'generating'" @click="tavern.planImage(m)">重新构思</button>
              <button type="button" class="btn primary" :disabled="m.draftState === 'generating' || app.busy" @click="tavern.drawImage(m)">
                {{ m.draftState === "generating" ? "生成中…" : m.images.length ? "再来一张" : "生成插图" }}
              </button>
            </div>
          </details>
          <div v-else-if="m.draftState === 'error'" class="row-btns">
            <button type="button" class="btn" @click="tavern.planImage(m)">重试构思</button>
            <button type="button" class="btn" @click="tavern.manualDraft(m)">手写提示词</button>
          </div>
        </template>
      </section>
    </div>
  </article>
</template>

<style scoped>
.msg { display: flex; gap: 12px; padding: 12px 4px; border-bottom: 1px solid #ffffff0d; }
.ava { flex: none; width: 44px; height: 44px; border-radius: 50%; overflow: hidden; background: var(--bg3); display: grid; place-items: center; color: var(--heading); font-weight: 700; }
.ava img { width: 100%; height: 100%; object-fit: cover; object-position: top; }
.msg.user .ava { background: #3d4270; }
.body { flex: 1; min-width: 0; }
header { display: flex; align-items: center; gap: 8px; margin-bottom: 4px; }
header b { color: var(--heading); }
.tools { margin-left: auto; display: flex; gap: 2px; opacity: 0.35; transition: opacity 0.15s; }
.msg:hover .tools { opacity: 1; }
.tools button { background: none; border: 0; padding: 2px 5px; font-size: 13px; }
.tools button:disabled { opacity: 0.3; cursor: default; }
.text { line-height: 1.75; word-break: break-word; font-size: 14.5px; }
.text :deep(em) { color: #b9b5d6; }
.text :deep(.q) { color: #f5f3c2; }
.caret { animation: blink 1s steps(2) infinite; color: var(--heading); }
@keyframes blink { 50% { opacity: 0; } }
.think { margin: 4px 0 6px; font-size: 12px; color: var(--muted); }
.think pre { white-space: pre-wrap; font-family: inherit; max-height: 240px; overflow: auto; background: #0003; padding: 8px; border-radius: 6px; }
.edit textarea { width: 100%; background: #16182d; border-radius: 8px; padding: 8px; line-height: 1.6; resize: vertical; }
.row-btns { display: flex; gap: 6px; align-items: center; margin-top: 6px; flex-wrap: wrap; }
.row-btns .btn { padding: 5px 10px; font-size: 12.5px; }
.grow { flex: 1; }
.swipes { display: flex; align-items: center; gap: 8px; margin-top: 4px; font-size: 12px; color: var(--muted); }
.swipes button { background: #2e3152; border: 0; border-radius: 6px; width: 26px; height: 22px; color: var(--heading); }
.swipes button:disabled { opacity: 0.3; cursor: default; }
.images { display: flex; flex-wrap: wrap; gap: 10px; margin-top: 10px; }
figure { margin: 0; width: 240px; max-width: 100%; }
figure img { width: 100%; border-radius: 8px; cursor: zoom-in; display: block; }
.img-missing { height: 120px; display: grid; place-items: center; background: #0003; border-radius: 8px; color: var(--muted); font-size: 12px; cursor: pointer; }
figcaption { display: flex; gap: 4px; margin-top: 4px; }
figcaption button { flex: 1; background: #2e3152; border: 0; border-radius: 6px; padding: 4px 0; font-size: 11.5px; color: var(--heading); }
.draft { margin-top: 10px; }
.draft-box { background: #16182d; border-radius: 10px; padding: 8px 10px; }
.draft-box summary { cursor: pointer; font-size: 12.5px; color: var(--heading); }
.draft label { display: flex; flex-direction: column; gap: 4px; font-size: 11.5px; color: var(--muted); margin-top: 6px; }
.draft input, .draft select, .draft textarea { background: #0e0f21; border-radius: 6px; padding: 6px 8px; font-size: 12.5px; }
.draft textarea { resize: vertical; }
.grid2 { display: grid; grid-template-columns: 120px 1fr; gap: 8px; }
.char-row { display: flex; gap: 6px; margin-top: 6px; align-items: center; }
.char-row select { width: 130px; flex: none; }
.char-row input:not(.num) { flex: 1; min-width: 0; }
.char-row .num { width: 54px; flex: none; }
.char-row .x { background: none; border: 0; color: var(--danger); font-size: 16px; }
.hint { color: var(--muted); font-size: 12.5px; margin: 0; }
.err { color: var(--danger); font-size: 12.5px; margin: 0 0 4px; word-break: break-all; }
</style>
