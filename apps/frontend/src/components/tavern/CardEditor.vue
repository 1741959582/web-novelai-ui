<script setup lang="ts">
import { computed, ref } from "vue";
import type { LoreEntry, TavernCard } from "@/api/tavern";
import { useTavernStore } from "@/stores/tavern";
import { makeAvatar, parseLore } from "@/utils/charaCard";

const props = defineProps<{ card: TavernCard }>();
const emit = defineEmits<{ close: [] }>();
const tavern = useTavernStore();
const tab = ref<"basic" | "prompt" | "image" | "lore">("basic");
const err = ref("");
const c = computed(() => props.card);

const tagsText = computed({
  get: () => c.value.tags.join(", "),
  set: (v: string) => (c.value.tags = v.split(/[,，]/).map((s) => s.trim()).filter(Boolean)),
});

function readFileAsDataUrl(file: File) {
  return new Promise<string>((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(String(r.result));
    r.onerror = () => reject(r.error);
    r.readAsDataURL(file);
  });
}

async function pickAvatar(e: Event) {
  const file = (e.target as HTMLInputElement).files?.[0];
  (e.target as HTMLInputElement).value = "";
  if (!file) return;
  try {
    c.value.avatar = await makeAvatar(await readFileAsDataUrl(file));
  } catch (error) {
    err.value = String(error);
  }
}

function addGreeting() {
  c.value.alternateGreetings.push("");
}

function removeGreeting(i: number) {
  c.value.alternateGreetings.splice(i, 1);
}

function toggleCast(id: string, on: boolean) {
  const set = new Set(c.value.charPresetIds);
  if (on) set.add(id);
  else set.delete(id);
  c.value.charPresetIds = [...set];
}

function addLore() {
  c.value.lore.push({ id: crypto.randomUUID(), keys: [], content: "", comment: "", enabled: true, constant: false });
}

function removeLore(e: LoreEntry) {
  c.value.lore = c.value.lore.filter((x) => x !== e);
}

function loreKeys(e: LoreEntry) {
  return e.keys.join(", ");
}

function setLoreKeys(e: LoreEntry, v: string) {
  e.keys = v.split(/[,，]/).map((s) => s.trim()).filter(Boolean);
}

async function importLore(ev: Event) {
  const file = (ev.target as HTMLInputElement).files?.[0];
  (ev.target as HTMLInputElement).value = "";
  if (!file) return;
  try {
    const data = JSON.parse(await file.text());
    const entries = parseLore(data.character_book ?? data.data?.character_book ?? data);
    if (!entries.length) throw new Error("文件里没有世界书条目");
    c.value.lore.push(...entries);
    err.value = "";
    tavern.notice = `导入了 ${entries.length} 条世界书`;
  } catch (e) {
    err.value = `世界书导入失败：${e instanceof Error ? e.message : String(e)}`;
  }
}

function close() {
  tavern.touchCard(c.value);
  if (c.value.id === tavern.state.activeCardId) tavern.refreshEmptyChat();
  emit("close");
}
</script>

<template>
  <div class="mask" @click.self="close">
    <div class="dlg">
      <header>
        <h3>编辑角色卡</h3>
        <div class="tabs">
          <button type="button" :class="{ on: tab === 'basic' }" @click="tab = 'basic'">基本</button>
          <button type="button" :class="{ on: tab === 'prompt' }" @click="tab = 'prompt'">高级提示词</button>
          <button type="button" :class="{ on: tab === 'image' }" @click="tab = 'image'">插图设定</button>
          <button type="button" :class="{ on: tab === 'lore' }" @click="tab = 'lore'">世界书 {{ c.lore.length }}</button>
        </div>
        <button type="button" class="btn primary" @click="close">完成</button>
      </header>
      <p v-if="err" class="err">{{ err }}</p>

      <div class="scroll">
        <template v-if="tab === 'basic'">
          <div class="top">
            <label class="avatar">
              <img v-if="c.avatar" :src="c.avatar" alt="" />
              <span v-else>上传头像</span>
              <input type="file" accept="image/*" hidden @change="pickAvatar" />
            </label>
            <div class="flex1">
              <label>名字<input v-model="c.name" /></label>
              <label>标签（逗号分隔）<input v-model="tagsText" /></label>
              <label>作者备注<input v-model="c.creatorNotes" /></label>
            </div>
          </div>
          <label>角色描述（外貌、身份、背景…）<textarea v-model="c.description" rows="6" /></label>
          <label>性格<textarea v-model="c.personality" rows="2" /></label>
          <label>场景<textarea v-model="c.scenario" rows="2" /></label>
          <label>开场白<textarea v-model="c.firstMes" rows="4" /></label>
          <div class="sub">
            <span>备选开场白（新对话里可以左右切换）</span>
            <button type="button" class="btn small" @click="addGreeting">+ 添加</button>
          </div>
          <div v-for="(_, i) in c.alternateGreetings" :key="i" class="greet">
            <textarea v-model="c.alternateGreetings[i]" rows="3" />
            <button type="button" class="x" @click="removeGreeting(i)">×</button>
          </div>
        </template>

        <template v-else-if="tab === 'prompt'">
          <p class="hint">可以用 <code v-pre>{{char}}</code> 和 <code v-pre>{{user}}</code> 代指角色名和你的名字。</p>
          <label>对话示例（用 &lt;START&gt; 分隔多段）<textarea v-model="c.mesExample" rows="8" /></label>
          <label>角色专属系统提示词（追加在主提示词之后）<textarea v-model="c.systemPrompt" rows="4" /></label>
          <label>历史后指令（附在你最新一句话后面，用来强调规则）<textarea v-model="c.postHistoryInstructions" rows="3" /></label>
        </template>

        <template v-else-if="tab === 'image'">
          <p class="hint">新建对话时，这里勾选的角色 tag 预设会自动出场，画风自动选中。导出角色卡时它们会一起写进卡片，别人导入后也能直接用。</p>
          <div class="sub"><span>默认出场角色</span></div>
          <p v-if="!tavern.state.charPresets.length" class="hint">还没有角色 tag 预设，先到左侧「角色tag」里新建。</p>
          <label v-for="p in tavern.state.charPresets" :key="p.id" class="check">
            <input type="checkbox" :checked="c.charPresetIds.includes(p.id)" @change="toggleCast(p.id, ($event.target as HTMLInputElement).checked)" />
            <b>{{ p.name }}</b>
            <small>{{ p.prompt }}</small>
          </label>
          <label class="mt">默认画风
            <select v-model="c.stylePresetId">
              <option value="">不使用</option>
              <option value="__follow__">跟随生成页画风</option>
              <option v-for="s in tavern.state.stylePresets" :key="s.id" :value="s.id">{{ s.name }}</option>
            </select>
          </label>
        </template>

        <template v-else>
          <p class="hint">当最近几条对话里出现关键词时，对应内容会自动加入设定；勾选「常驻」则始终加入。</p>
          <div class="sub">
            <button type="button" class="btn small" @click="addLore">+ 新条目</button>
            <label class="btn small file">导入世界书 JSON<input type="file" accept=".json" hidden @change="importLore" /></label>
          </div>
          <div v-for="e in c.lore" :key="e.id" class="lore" :class="{ off: !e.enabled }">
            <div class="lore-head">
              <input v-model="e.comment" placeholder="备注名" class="flex1" />
              <label class="inline"><input v-model="e.enabled" type="checkbox" />启用</label>
              <label class="inline"><input v-model="e.constant" type="checkbox" />常驻</label>
              <button type="button" class="x" @click="removeLore(e)">×</button>
            </div>
            <input :value="loreKeys(e)" placeholder="关键词，逗号分隔" @change="setLoreKeys(e, ($event.target as HTMLInputElement).value)" />
            <textarea v-model="e.content" rows="3" placeholder="内容" />
          </div>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
.mask { position: fixed; inset: 0; background: #000a; display: grid; place-items: center; z-index: 50; }
.dlg { width: min(760px, 94vw); height: min(820px, 90vh); background: var(--bg1); border: 1px solid var(--bg3); border-radius: 12px; display: flex; flex-direction: column; overflow: hidden; }
header { display: flex; align-items: center; gap: 12px; padding: 12px 16px; border-bottom: 1px solid var(--bg3); }
header h3 { margin: 0; font-size: 17px; }
.tabs { display: flex; gap: 4px; flex: 1; }
.tabs button { border: 0; background: #2e3152; color: var(--heading); border-radius: 8px; padding: 6px 10px; font-size: 13px; }
.tabs button.on { background: #3d4270; }
.scroll { flex: 1; overflow: auto; padding: 12px 16px 20px; }
label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; color: var(--muted); margin-bottom: 10px; }
input, textarea, select { background: #0e0f21; border-radius: 8px; padding: 8px 10px; font-size: 13.5px; color: #fff; }
textarea { resize: vertical; line-height: 1.6; }
.top { display: flex; gap: 14px; }
.avatar { width: 130px; height: 180px; flex: none; border-radius: 10px; overflow: hidden; background: #0e0f21; display: grid; place-items: center; cursor: pointer; }
.avatar img { width: 100%; height: 100%; object-fit: cover; }
.flex1 { flex: 1; min-width: 0; }
.sub { display: flex; align-items: center; gap: 8px; justify-content: space-between; font-size: 12px; color: var(--muted); margin: 6px 0; }
.btn.small { padding: 4px 10px; font-size: 12px; }
.btn.file { display: inline-block; margin: 0; color: var(--text); }
.greet { display: flex; gap: 6px; margin-bottom: 8px; }
.greet textarea { flex: 1; }
.x { background: none; border: 0; color: var(--danger); font-size: 18px; }
.check { flex-direction: row; align-items: center; gap: 8px; margin: 0 0 6px; color: var(--text); }
.check small { color: var(--muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.mt { margin-top: 14px; }
.lore { background: #16182d; border-radius: 10px; padding: 8px; margin-bottom: 8px; display: flex; flex-direction: column; gap: 6px; }
.lore.off { opacity: 0.5; }
.lore-head { display: flex; gap: 8px; align-items: center; }
.inline { flex-direction: row; align-items: center; gap: 4px; margin: 0; color: var(--text); }
.hint { color: var(--muted); font-size: 12.5px; line-height: 1.6; margin: 0 0 10px; }
.err { color: var(--danger); padding: 6px 16px 0; margin: 0; font-size: 13px; }
</style>
