<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { NAI_MODELS, NAI_SAMPLERS, NAI_UC_PRESETS, isV4Plus, maxCharacterPrompts } from "@/types/nai";
import { useAppStore } from "@/stores/app";
import { useBatchStore, type BatchJob } from "@/stores/batch";
import NaiIcon from "@/components/NaiIcon.vue";
import PromptField from "@/components/PromptField.vue";
import { loadAutoComplete } from "@/utils/promptTools";
import { RESOLUTION_TABLE } from "@/utils/resolution";

const app = useAppStore();
const store = useBatchStore();
const router = useRouter();
const autoComplete = ref(loadAutoComplete());
const openId = ref("");
const added = ref(0);
const charTab = ref<Record<string, "prompt" | "uc">>({});
const maxShared = computed(() => maxCharacterPrompts(app.params.model));
const canShared = computed(() => isV4Plus(app.params.model));

const livePreview = computed(() => app.genPreview || app.previewUrl);
const canRun = computed(() => !store.running && !app.busy && store.pending.length > 0);
const progress = computed(() => Math.min(1, Math.max(0, app.genProgress)));
const sizeOptions = computed(() => {
  const seen = new Set<string>();
  const list: { label: string; value: string }[] = [];
  for (const family of Object.keys(RESOLUTION_TABLE) as Array<keyof typeof RESOLUTION_TABLE>) {
    for (const orientation of Object.keys(RESOLUTION_TABLE[family]) as Array<keyof (typeof RESOLUTION_TABLE)[typeof family]>) {
      const [width, height] = RESOLUTION_TABLE[family][orientation];
      const value = `${width}x${height}`;
      if (seen.has(value)) continue;
      seen.add(value);
      list.push({ label: `${width}×${height}`, value });
    }
  }
  return list;
});

function sizeValue(job: BatchJob) {
  return `${job.params.width}x${job.params.height}`;
}

function setSize(job: BatchJob, value: string) {
  const [width, height] = value.split("x").map(Number);
  if (!width || !height) return;
  job.params.width = width;
  job.params.height = height;
}

onMounted(() => {
  void store.boot();
});

function addBulk() {
  added.value = store.addFromBulk();
  app.status = added.value ? `已加入 ${added.value} 条任务` : "没有解析到任务";
}

function toggle(job: BatchJob) {
  openId.value = openId.value === job.id ? "" : job.id;
}

function label(job: BatchJob) {
  const text = job.prompt.replace(/\s+/g, " ").trim();
  return text || "（空主词）";
}

function statusLabel(job: BatchJob) {
  if (job.status === "running") return "生成中";
  if (job.status === "done") return "完成";
  if (job.status === "error") return "失败";
  return "排队";
}

function goGenerate() {
  void router.push("/");
}

async function start() {
  await store.runQueue();
  if (store.done.length && !store.pending.length) app.status = `批量完成，共 ${store.done.length} 张`;
}
</script>

<template>
  <div class="page">
    <header class="top">
      <h2>批量生成</h2>
      <span class="stat">{{ store.progressText }}</span>
      <span class="spacer" />
      <button class="btn" type="button" :disabled="store.running" @click="store.importCurrent()">导入当前生成页</button>
      <button class="btn" type="button" :disabled="store.running" @click="store.addEmpty()">加空任务</button>
      <button v-if="!store.running" class="btn primary" type="button" :disabled="!canRun" @click="start">
        开始排队生成
      </button>
      <button v-else class="btn danger" type="button" @click="store.requestStop()">当前完成后停止</button>
    </header>

    <div v-if="store.running || app.busy" class="live">
      <div class="live-img">
        <img v-if="livePreview" :src="livePreview" alt="" />
        <div v-else class="wait">正在生成 {{ store.done.length + 1 }}/{{ store.jobs.length }}…</div>
      </div>
      <div class="live-meta">
        <strong>{{ store.current ? label(store.current) : "排队中" }}</strong>
        <span>{{ Math.round(progress * 100) }}% · {{ app.genStep }}/{{ app.genSteps }} steps</span>
        <div class="bar"><i :style="{ width: `${Math.max(3, Math.round(progress * 100))}%` }" /></div>
        <p>{{ app.status }}</p>
      </div>
    </div>

    <div class="body">
      <section class="left">
        <div class="left-scroll">
        <h3>批量录入</h3>
        <p class="hint">一行一条主词。要用角色词时用 --- 分段，可写 角色: / 负面: / 尺寸: 832x1216</p>
        <textarea v-model="store.bulkText" rows="10" placeholder="1girl, school uniform, looking at viewer&#10;1girl, maid, indoor&#10;---&#10;2girls, cafe&#10;角色: long hair, smile&#10;角色: short hair, hat" />

        <section class="chars">
          <header>
            <div>
              <strong>角色提示词</strong>
              <p class="hint">和生成页一样，点加号新增角色，会写进每条新任务。</p>
            </div>
            <button
              class="icon-sq"
              type="button"
              title="新增角色"
              :disabled="!canShared || store.sharedCharacters.length >= maxShared"
              @click="store.addSharedCharacter()"
            >
              <NaiIcon name="plus" :size="16" />
            </button>
          </header>
          <p v-if="!canShared" class="hint">当前生成页的模型不支持角色提示词。</p>
          <article v-for="(c, i) in store.sharedCharacters" :key="c.id" class="char" :class="{ off: !c.enabled }">
            <div class="char-head">
              <strong>角色 {{ i + 1 }}</strong>
              <span class="spacer" />
              <button class="ghost" type="button" title="上移" @click="store.moveSharedCharacter(c.id, -1)"><NaiIcon name="up" :size="14" /></button>
              <button class="ghost" type="button" title="下移" @click="store.moveSharedCharacter(c.id, 1)"><NaiIcon name="down" :size="14" /></button>
              <button class="ghost" type="button" :class="{ on: c.enabled }" title="启用" @click="store.updateSharedCharacter(c.id, { enabled: !c.enabled })">
                <NaiIcon name="check" :size="14" />
              </button>
              <button class="ghost" type="button" title="删除" @click="store.removeSharedCharacter(c.id)">
                <NaiIcon name="trash" :size="14" />
              </button>
            </div>
            <div class="text-tabs">
              <button type="button" :class="{ on: (charTab[c.id] || 'prompt') === 'prompt' }" @click="charTab[c.id] = 'prompt'">Prompt</button>
              <button type="button" :class="{ on: charTab[c.id] === 'uc' }" @click="charTab[c.id] = 'uc'">Undesired Content</button>
            </div>
            <PromptField
              v-if="(charTab[c.id] || 'prompt') === 'prompt'"
              :model-value="c.prompt"
              :enabled="autoComplete"
              placeholder="Character prompt..."
              @update:model-value="store.updateSharedCharacter(c.id, { prompt: $event })"
            />
            <PromptField
              v-else
              :model-value="c.negativePrompt"
              :enabled="autoComplete"
              placeholder="lowres, extra fingers..."
              @update:model-value="store.updateSharedCharacter(c.id, { negativePrompt: $event })"
            />
          </article>
        </section>
        <label>默认负面
          <textarea v-model="store.sharedNeg" rows="2" placeholder="lowres, worst quality, bad anatomy" />
        </label>
        <label class="inline">每条任务张数
          <select v-model.number="store.sharedCopies">
            <option v-for="n in 4" :key="n" :value="n">{{ n }}</option>
          </select>
        </label>
        <p class="hint">新任务会带上<strong>当前生成页</strong>的模型、分辨率、步数、采样器和引导值。</p>
        <div class="row">
          <button class="btn primary" type="button" :disabled="store.running" @click="addBulk">加入队列</button>
          <button class="btn" type="button" :disabled="store.running || !store.jobs.length" title="用当前配置覆盖未完成任务" @click="store.applySharedToAll()">
            覆盖未完成
          </button>
        </div>
        </div>
      </section>

      <section class="right">
        <div class="queue-head">
          <h3>生成队列</h3>
          <div class="row">
            <button class="ghost" type="button" :disabled="store.running" @click="store.resetErrors()">重试失败</button>
            <button class="ghost" type="button" :disabled="store.running" @click="store.clearDone()">清已完成</button>
            <button class="ghost" type="button" :disabled="store.running" @click="store.clearAll()">清空</button>
          </div>
        </div>
        <div class="queue-scroll">
        <p v-if="!store.jobs.length" class="empty">还没有任务。左边贴主词后点「加入队列」，或从生成页导入一组。</p>
        <article
          v-for="(job, i) in store.jobs"
          :key="job.id"
          class="job"
          :class="[job.status, { open: openId === job.id }]"
        >
          <button class="job-main" type="button" @click="toggle(job)">
            <img v-if="job.previewUrl" :src="job.previewUrl" alt="" />
            <span v-else class="ph">{{ i + 1 }}</span>
            <div class="job-text">
              <strong>{{ label(job) }}</strong>
              <small>
                {{ job.params.width }}×{{ job.params.height }} · {{ job.params.steps }} steps ·
                {{ job.characters.filter((c) => c.prompt.trim()).length }} 角色 · ×{{ job.copies }}
              </small>
              <small v-if="job.error" class="err">{{ job.error }}</small>
            </div>
            <em>{{ statusLabel(job) }}</em>
          </button>
          <div class="job-acts">
            <button class="ghost" type="button" :disabled="store.running" title="上移" @click="store.move(job.id, -1)">↑</button>
            <button class="ghost" type="button" :disabled="store.running" title="下移" @click="store.move(job.id, 1)">↓</button>
            <button class="ghost" type="button" :disabled="store.running" @click="store.duplicate(job.id)">复制</button>
            <button class="ghost" type="button" @click="store.sendToGenerate(job.id); goGenerate()">写回生成页</button>
            <button class="ghost" type="button" :disabled="store.running && job.status === 'running'" @click="store.remove(job.id)">删除</button>
          </div>
          <div v-if="openId === job.id" class="editor">
            <label>主关键词
              <PromptField v-model="job.prompt" :enabled="autoComplete" placeholder="主提示词" />
            </label>
            <label>负面
              <PromptField v-model="job.negativePrompt" :enabled="autoComplete" placeholder="lowres, worst quality..." />
            </label>
            <label>画风 / Style
              <PromptField v-model="job.stylePrompt" :enabled="autoComplete" placeholder="可选" />
            </label>
            <div class="chars">
              <header>
                <span>角色提示词</span>
                <button
                  class="icon-sq"
                  type="button"
                  title="新增角色"
                  :disabled="!isV4Plus(job.params.model) || job.characters.length >= maxCharacterPrompts(job.params.model)"
                  @click="store.addCharacter(job.id)"
                >
                  <NaiIcon name="plus" :size="16" />
                </button>
              </header>
              <article v-for="(c, ci) in job.characters" :key="c.id" class="char" :class="{ off: !c.enabled }">
                <div class="char-head">
                  <strong>角色 {{ ci + 1 }}</strong>
                  <span class="spacer" />
                  <button class="ghost" type="button" title="上移" @click="store.moveCharacter(job.id, c.id, -1)"><NaiIcon name="up" :size="14" /></button>
                  <button class="ghost" type="button" title="下移" @click="store.moveCharacter(job.id, c.id, 1)"><NaiIcon name="down" :size="14" /></button>
                  <button class="ghost" type="button" :class="{ on: c.enabled }" title="启用" @click="store.updateCharacter(job.id, c.id, { enabled: !c.enabled })">
                    <NaiIcon name="check" :size="14" />
                  </button>
                  <button class="ghost" type="button" title="删除" @click="store.removeCharacter(job.id, c.id)">
                    <NaiIcon name="trash" :size="14" />
                  </button>
                </div>
                <div class="text-tabs">
                  <button type="button" :class="{ on: (charTab[c.id] || 'prompt') === 'prompt' }" @click="charTab[c.id] = 'prompt'">Prompt</button>
                  <button type="button" :class="{ on: charTab[c.id] === 'uc' }" @click="charTab[c.id] = 'uc'">Undesired Content</button>
                </div>
                <PromptField
                  v-if="(charTab[c.id] || 'prompt') === 'prompt'"
                  :model-value="c.prompt"
                  :enabled="autoComplete"
                  placeholder="Character prompt..."
                  @update:model-value="store.updateCharacter(job.id, c.id, { prompt: $event })"
                />
                <PromptField
                  v-else
                  :model-value="c.negativePrompt"
                  :enabled="autoComplete"
                  placeholder="lowres, extra fingers..."
                  @update:model-value="store.updateCharacter(job.id, c.id, { negativePrompt: $event })"
                />
              </article>
            </div>
            <div class="cfg">
              <label>模型
                <select v-model="job.params.model">
                  <option v-for="m in NAI_MODELS" :key="m.value" :value="m.value">{{ m.label }}</option>
                </select>
              </label>
              <label>尺寸
                <select :value="sizeValue(job)" @change="setSize(job, ($event.target as HTMLSelectElement).value)">
                  <option
                    v-if="!sizeOptions.some((s) => s.value === sizeValue(job))"
                    :value="sizeValue(job)"
                  >
                    {{ job.params.width }}×{{ job.params.height }}
                  </option>
                  <option v-for="s in sizeOptions" :key="s.value" :value="s.value">{{ s.label }}</option>
                </select>
              </label>
              <label>采样器
                <select v-model="job.params.sampler">
                  <option v-for="s in NAI_SAMPLERS" :key="s.value" :value="s.value">{{ s.short }}</option>
                </select>
              </label>
              <label>步数
                <input v-model.number="job.params.steps" type="number" min="1" max="50" />
              </label>
              <label>Guidance
                <input v-model.number="job.params.cfgScale" type="number" min="0" max="10" step="0.1" />
              </label>
              <label>UC
                <select v-model.number="job.params.ucPreset">
                  <option v-for="u in NAI_UC_PRESETS" :key="u.value" :value="u.value">{{ u.label }}</option>
                </select>
              </label>
              <label>种子模式
                <select v-model="job.params.seedMode">
                  <option value="random">随机</option>
                  <option value="fixed">固定</option>
                </select>
              </label>
              <label>种子
                <input v-model.number="job.params.seed" type="number" :disabled="job.params.seedMode !== 'fixed'" />
              </label>
              <label>张数
                <select v-model.number="job.copies">
                  <option v-for="n in 4" :key="n" :value="n">{{ n }}</option>
                </select>
              </label>
            </div>
          </div>
        </article>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.page {
  height: 100%;
  min-height: 0;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--bg0);
}
.top, .queue-head, .row, .char-head, .chars header {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: nowrap;
}
.top {
  flex: none;
  padding: 8px 12px;
  overflow-x: auto;
}
.top > * { flex: none; white-space: nowrap; }
.top h2 { margin: 0; font-size: 18px; }
.spacer { flex: 1 0 8px; }
.queue-head { justify-content: space-between; margin-bottom: 8px; }
.hint {
  color: var(--muted);
  font-size: 12px;
  margin: 0;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.stat { color: var(--heading); font-weight: 700; }
.body {
  flex: 1;
  min-height: 0;
  display: flex;
}
.left {
  width: 400px;
  min-width: 320px;
  max-width: 42vw;
  background: var(--bg1);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.left-scroll {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 8px 14px 14px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.left-scroll textarea { flex: 1; min-height: 72px; }
.right {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  padding: 8px 12px 12px;
  overflow: hidden;
}
.queue-scroll { flex: 1; min-height: 0; overflow: auto; }
h3 { margin: 0; font-size: 16px; }
.left-scroll textarea, .editor :deep(textarea), .cfg input, .cfg select, .inline select {
  width: 100%;
  background: var(--bg0);
  border: 1px solid var(--bg3);
  border-radius: 8px;
  padding: 8px 10px;
}
label { display: grid; gap: 6px; font-size: 12px; color: var(--muted); }
.inline { grid-template-columns: 1fr auto; align-items: center; }
.live {
  flex: none;
  display: grid;
  grid-template-columns: 96px 1fr;
  gap: 12px;
  margin: 0 12px 8px;
  background: var(--bg2);
  border: 1px solid var(--bg3);
  border-radius: 12px;
  padding: 8px;
}
.live-img {
  height: 120px;
  border-radius: 8px;
  overflow: hidden;
  background: var(--bg0);
  display: grid;
  place-items: center;
}
.live-img img { width: 100%; height: 100%; object-fit: contain; }
.wait, .empty { color: var(--muted); font-size: 13px; }
.live-meta { display: grid; align-content: center; gap: 4px; min-width: 0; }
.live-meta strong, .job-text strong {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.live-meta p { margin: 0; color: var(--muted); font-size: 12px; }
.bar { height: 6px; background: var(--bg0); border-radius: 99px; overflow: hidden; }
.bar i { display: block; height: 100%; background: var(--heading); }
.job {
  border: 1px solid var(--bg3);
  border-radius: 10px;
  margin-bottom: 8px;
  background: var(--bg1);
}
.job.running { border-color: var(--heading); }
.job.done { border-color: #2a5a45; }
.job.error { border-color: var(--danger); }
.job-main {
  width: 100%;
  display: grid;
  grid-template-columns: 56px 1fr auto;
  gap: 10px;
  align-items: center;
  background: transparent;
  border: 0;
  text-align: left;
  padding: 8px;
}
.job-main img, .ph {
  width: 56px;
  height: 56px;
  border-radius: 8px;
  object-fit: cover;
  background: var(--bg0);
}
.ph { display: grid; place-items: center; color: var(--muted); font-weight: 700; }
.job-text { min-width: 0; display: grid; gap: 2px; }
.job-text small { color: var(--muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.job-text .err { color: var(--danger); }
.job-main em {
  font-style: normal;
  font-size: 12px;
  color: var(--muted);
}
.job.running em { color: var(--heading); }
.job.done em { color: var(--ok); }
.job.error em { color: var(--danger); }
.job-acts { display: flex; gap: 4px; padding: 0 8px 8px; flex-wrap: nowrap; overflow-x: auto; }
.job-acts .ghost { white-space: nowrap; }
.editor { display: grid; gap: 10px; padding: 0 10px 12px; }
.cfg { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
.chars { display: grid; gap: 8px; }
.chars header { justify-content: space-between; align-items: flex-start; color: var(--heading); font-size: 13px; }
.chars header strong { color: var(--heading); font-size: 16px; }
.chars header > div { min-width: 0; flex: 1; }
.icon-sq {
  width: 32px;
  height: 32px;
  display: grid;
  place-items: center;
  flex: none;
  background: var(--bg2);
  border: 1px solid var(--bg3);
  border-radius: 6px;
  color: #fff;
}
.icon-sq:disabled { opacity: 0.4; }
.char { display: grid; gap: 6px; padding: 8px; background: var(--bg0); border-radius: 8px; }
.char.off { opacity: 0.55; }
.text-tabs { display: flex; gap: 12px; }
.text-tabs button { background: transparent; border: 0; color: var(--muted); padding: 0; }
.text-tabs button.on { color: var(--heading); }
.ghost.on { color: var(--heading); }
.ghost {
  background: transparent;
  border: 0;
  color: var(--muted);
  padding: 4px 6px;
}
.ghost:hover { color: var(--heading); }
@media (max-width: 860px) {
  .body { flex-direction: column; }
  .left { width: auto; max-width: none; min-height: 0; flex: 1; }
  .right { flex: 1; }
  .cfg { grid-template-columns: 1fr; }
}
</style>
