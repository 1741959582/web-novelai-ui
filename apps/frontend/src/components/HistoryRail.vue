<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { useAppStore } from "@/stores/app";
import { useApngStore, type ApngTab } from "@/stores/apng";
import { useBatchStore } from "@/stores/batch";
import { readImageDataUrl, type SavedImage } from "@/api/tauri";
import NaiIcon from "@/components/NaiIcon.vue";
import type { HistoryItem } from "@/types/nai";

const store = useAppStore();
const apng = useApngStore();
const batch = useBatchStore();
const router = useRouter();
const thumbs = ref<Record<string, string>>({});
const newGroupName = ref("");
const renameName = ref("");
const renaming = ref(false);
const arranging = ref(false);
const busyId = ref("");
const picked = ref<string[]>([]);
const sending = ref(false);
let pickAnchor = "";

const sendTargets: { id: ApngTab; label: string }[] = [
  { id: "disguise", label: "APNG伪装" },
  { id: "gif", label: "合成GIF" },
  { id: "restore", label: "还原真图" },
  { id: "meta", label: "清除元数据" },
  { id: "mosaic", label: "打马赛克" },
];

const items = computed(() => store.visibleHistory());
const activeGroup = computed(() =>
  store.historyGroups.find((g) => g.id === store.selectedHistoryGroupId) ?? null,
);

async function loadThumbs() {
  const next = { ...thumbs.value };
  for (const item of items.value.slice(0, 48)) {
    if (next[item.id]) continue;
    try {
      next[item.id] = await readImageDataUrl(item.path);
    } catch {
      next[item.id] = "";
    }
  }
  thumbs.value = next;
}

watch(
  () => [store.historyOpen, items.value.map((h) => h.id).join("|")] as const,
  () => {
    const ids = new Set(items.value.map((item) => item.id));
    picked.value = picked.value.filter((id) => ids.has(id));
    if (store.historyOpen) void loadThumbs();
  },
  { immediate: true },
);

watch(
  () => activeGroup.value?.name ?? "",
  (name) => {
    if (!renaming.value) renameName.value = name;
  },
  { immediate: true },
);

async function createGroup() {
  const name = newGroupName.value.trim();
  if (!name) return;
  try {
    await store.createHistoryGroup(name);
    newGroupName.value = "";
  } catch (e) {
    store.status = e instanceof Error ? e.message : String(e);
  }
}

async function arrangeGroups() {
  if (arranging.value) return;
  arranging.value = true;
  try {
    await store.arrangeHistoryGroups();
  } catch (e) {
    store.status = e instanceof Error ? e.message : String(e);
  } finally {
    arranging.value = false;
  }
}

async function saveRename() {
  const group = activeGroup.value;
  const name = renameName.value.trim();
  if (!group || !name) return;
  try {
    await store.renameHistoryGroup(group.id, name);
    renaming.value = false;
  } catch (e) {
    store.status = e instanceof Error ? e.message : String(e);
  }
}

async function removeGroup() {
  const group = activeGroup.value;
  if (!group) return;
  if (!window.confirm(`删除分组「${group.name}」？图片会移回输出目录，不会被删除。`)) return;
  await store.deleteHistoryGroup(group.id);
  renaming.value = false;
}

async function changeItemGroup(item: HistoryItem, groupId: string) {
  busyId.value = item.id;
  try {
    await store.setHistoryItemGroup(item.id, groupId);
  } catch (e) {
    store.status = e instanceof Error ? e.message : String(e);
  } finally {
    busyId.value = "";
  }
}

function togglePick(id: string, shift = false) {
  if (shift && pickAnchor) {
    const ids = items.value.map((item) => item.id);
    const a = ids.indexOf(pickAnchor);
    const b = ids.indexOf(id);
    if (a >= 0 && b >= 0) {
      const [lo, hi] = a < b ? [a, b] : [b, a];
      const set = new Set(picked.value);
      ids.slice(lo, hi + 1).forEach((item) => set.add(item));
      picked.value = [...set];
      return;
    }
  }
  pickAnchor = id;
  picked.value = picked.value.includes(id)
    ? picked.value.filter((item) => item !== id)
    : [...picked.value, id];
}

function selectVisible() {
  picked.value = items.value.map((item) => item.id);
  pickAnchor = picked.value[0] || "";
}

async function dataUrlFor(item: HistoryItem) {
  if (thumbs.value[item.id]) return thumbs.value[item.id];
  const url = await readImageDataUrl(item.path);
  thumbs.value = { ...thumbs.value, [item.id]: url };
  return url;
}

async function addApng(item: HistoryItem) {
  const url = await dataUrlFor(item);
  const ok = await apng.addReal(url, item.id);
  await router.push("/apng");
  store.status = ok ? `已清掉元数据并加入伪装队列（${apng.items.length}）` : "伪装队列已满";
}

async function sendPicked(target: ApngTab) {
  const chosen = items.value.filter((item) => picked.value.includes(item.id));
  if (!chosen.length || sending.value) return;
  sending.value = true;
  store.status = `正在准备 ${chosen.length} 张…`;
  try {
    const files: SavedImage[] = [];
    for (const item of chosen) {
      files.push({ path: item.path, dataUrl: await dataUrlFor(item) });
    }
    const count = await apng.importFiles(target, files);
    await router.push("/apng");
    const label = sendTargets.find((item) => item.id === target)?.label || "APNG";
    store.status = count
      ? `已把 ${count} 张加入「${label}」`
      : "没有加进去，队列可能已满";
    if (count) picked.value = [];
  } catch (e) {
    store.status = e instanceof Error ? e.message : String(e);
  } finally {
    sending.value = false;
  }
}

async function sendToBatch() {
  const chosen = items.value.filter((item) => picked.value.includes(item.id));
  if (!chosen.length || sending.value) return;
  sending.value = true;
  store.status = `正在把 ${chosen.length} 条关键词加入批量任务…`;
  try {
    const count = await batch.addFromHistory(chosen);
    await router.push("/batch");
    store.status = count
      ? `已加入 ${count} 条批量任务，展开后可以改关键词`
      : "没有可导入的图片";
    if (count) picked.value = [];
  } catch (e) {
    store.status = e instanceof Error ? e.message : String(e);
  } finally {
    sending.value = false;
  }
}

async function copyItem(item: HistoryItem) {
  busyId.value = item.id;
  try {
    await store.copyHistoryImage(item);
  } finally {
    busyId.value = "";
  }
}

async function revealItem(item: HistoryItem) {
  busyId.value = item.id;
  try {
    await store.revealHistoryItem(item);
  } finally {
    busyId.value = "";
  }
}

async function deleteItem(item: HistoryItem) {
  const name = item.path.split(/[\\/]/).pop() || item.id;
  if (!window.confirm(`删除这张图片？\n${name}`)) return;
  busyId.value = item.id;
  try {
    await store.removeHistory(item.id);
    delete thumbs.value[item.id];
  } catch (e) {
    store.status = e instanceof Error ? e.message : String(e);
  } finally {
    busyId.value = "";
  }
}
</script>

<template>
  <aside class="rail" :class="{ open: store.historyOpen }">
    <button class="tab" type="button" title="History" @click="store.historyOpen = !store.historyOpen">
      History
      <NaiIcon name="history" :size="12" />
    </button>
    <div v-show="store.historyOpen" class="body">
      <header>
        <strong>History</strong>
        <button class="ghost" type="button" @click="store.showSessionDialog = true">Sessions</button>
      </header>

      <div class="groups">
        <select v-model="store.selectedHistoryGroupId" aria-label="历史分组">
          <option value="">全部分组</option>
          <option value="__ungrouped">未分组</option>
          <option v-for="group in store.historyGroups" :key="group.id" :value="group.id">
            {{ group.name }}
          </option>
        </select>
        <div class="create">
          <input
            v-model="newGroupName"
            placeholder="新分组名"
            @keydown.enter.prevent="createGroup"
          />
          <button type="button" class="ghost" :disabled="!newGroupName.trim()" @click="createGroup">创建</button>
        </div>
        <button type="button" class="ghost arrange" :disabled="arranging" @click="arrangeGroups">
          {{ arranging ? "正在整理…" : "放入分组文件夹" }}
        </button>
        <div v-if="activeGroup" class="group-acts">
          <template v-if="renaming">
            <input v-model="renameName" @keydown.enter.prevent="saveRename" @keydown.esc="renaming = false" />
            <button type="button" class="ghost" @click="saveRename">保存</button>
            <button type="button" class="ghost" @click="renaming = false">取消</button>
          </template>
          <template v-else>
            <button type="button" class="ghost" @click="renaming = true">重命名</button>
            <button type="button" class="ghost danger" @click="removeGroup">删分组</button>
          </template>
        </div>
      </div>

      <div v-if="items.length" class="pick-bar">
        <div class="pick-head">
          <span>已选 {{ picked.length }}</span>
          <button type="button" class="ghost" @click="selectVisible">全选</button>
          <button type="button" class="ghost" :disabled="!picked.length" @click="picked = []">取消</button>
        </div>
        <div class="pick-actions">
          <button
            v-for="target in sendTargets"
            :key="target.id"
            type="button"
            :disabled="!picked.length || sending"
            @click="sendPicked(target.id)"
          >
            {{ target.label }}
          </button>
          <button type="button" :disabled="!picked.length || sending" @click="sendToBatch">导入批量</button>
        </div>
      </div>

      <p v-if="!items.length" class="hint">
        {{ store.history.length ? "这个分组里还没有图片。" : "生成后的图片会出现在这里。" }}
      </p>
      <div v-for="item in items" :key="item.id" class="item" :class="{ busy: busyId === item.id, picked: picked.includes(item.id) }">
        <div class="thumb-wrap">
          <button class="thumb" type="button" title="预览；双击载入参数" @click="store.showHistory(item)" @dblclick="store.applyHistory(item)">
            <img v-if="thumbs[item.id]" :src="thumbs[item.id]" alt="" />
          </button>
          <button
            type="button"
            class="tick"
            :class="{ on: picked.includes(item.id) }"
            :aria-pressed="picked.includes(item.id)"
            title="多选，按住 Shift 可连选"
            @click.stop="togglePick(item.id, $event.shiftKey)"
            @dblclick.stop
          >
            <span v-if="picked.includes(item.id)">✓</span>
          </button>
        </div>
        <div class="meta">
          <button class="title" type="button" title="在资源管理器中显示" @click="revealItem(item)">
            {{ item.width }}×{{ item.height }}
          </button>
          <div class="sub">{{ item.prompt || "(no prompt)" }}</div>
          <select
            class="group-pick"
            :value="item.groupId || ''"
            :disabled="busyId === item.id"
            aria-label="移动到分组"
            @change="changeItemGroup(item, ($event.target as HTMLSelectElement).value)"
          >
            <option value="">未分组</option>
            <option v-for="group in store.historyGroups" :key="group.id" :value="group.id">
              {{ group.name }}
            </option>
          </select>
          <div class="acts">
            <button class="import" type="button" @click.stop="store.importFromHistory(item)">导入元数据</button>
            <button class="icon" type="button" title="添加到 APNG" @click.stop="addApng(item)">
              <NaiIcon name="layers" :size="12" />
            </button>
            <button class="icon" type="button" title="复制图片" @click.stop="copyItem(item)">
              <NaiIcon name="copy" :size="12" />
            </button>
            <button class="icon" type="button" title="打开文件目录" @click.stop="revealItem(item)">
              <NaiIcon name="folder" :size="12" />
            </button>
            <button class="icon danger" type="button" title="删除图片" @click.stop="deleteItem(item)">
              <NaiIcon name="trash" :size="12" />
            </button>
          </div>
        </div>
      </div>
    </div>
  </aside>
</template>

<style scoped>
.rail {
  position: fixed;
  top: 52px;
  right: 0;
  bottom: 0;
  z-index: 24;
  width: 0;
  pointer-events: none;
}
.tab {
  pointer-events: auto;
  position: absolute;
  top: 12px;
  right: 0;
  z-index: 26;
  writing-mode: vertical-rl;
  transform: rotate(180deg);
  background: var(--bg1);
  color: #fff;
  border: 1px solid var(--bg3);
  border-right: 0;
  border-radius: 0 6px 6px 0;
  padding: 14px 8px;
  letter-spacing: 0.4px;
  display: flex;
  align-items: center;
  gap: 6px;
}
.rail.open .tab { right: 300px; }
.body {
  pointer-events: auto;
  position: absolute;
  top: 0;
  right: 0;
  bottom: 0;
  width: 300px;
  overflow: auto;
  background: var(--bg1);
  border-left: 1px solid var(--bg3);
  padding: 14px 12px;
  box-shadow: -12px 0 28px rgba(0, 0, 0, 0.28);
}
header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 10px; }
.ghost {
  background: transparent;
  border: 1px solid var(--bg3);
  border-radius: 4px;
  padding: 4px 8px;
  color: var(--muted);
}
.ghost:disabled { opacity: 0.45; }
.ghost.danger { color: #f3b4b4; border-color: #5a2f3a; }
.ghost.arrange { width: 100%; color: #f5f3c2; }
.hint { color: var(--muted); font-size: 13px; }
.groups {
  display: grid;
  gap: 6px;
  margin-bottom: 12px;
}
.pick-bar {
  position: sticky;
  top: 0;
  z-index: 2;
  margin: 0 -12px 10px;
  padding: 8px 12px;
  background: #191b31;
  border-bottom: 1px solid var(--bg3);
  display: grid;
  gap: 6px;
}
.pick-head {
  display: flex;
  align-items: center;
  gap: 6px;
  color: rgba(255,255,255,0.72);
  font-size: 12px;
}
.pick-head span { margin-right: auto; font-weight: 700; color: #f5f3c2; }
.pick-actions { display: flex; flex-wrap: wrap; gap: 4px; }
.pick-actions button {
  border: 0;
  border-radius: 6px;
  background: #2e3152;
  color: #f5f3c2;
  padding: 4px 7px;
  font-size: 11px;
}
.pick-actions button:disabled { opacity: 0.4; }
.groups select,
.groups input,
.group-pick {
  width: 100%;
  min-width: 0;
  background: #12132a;
  border: 1px solid var(--bg3);
  border-radius: 4px;
  padding: 5px 8px;
  color: #fff;
}
.create,
.group-acts {
  display: flex;
  gap: 6px;
  align-items: center;
}
.create input,
.group-acts input { flex: 1; }
.item {
  display: grid;
  grid-template-columns: 48px 1fr;
  gap: 8px;
  width: 100%;
  text-align: left;
  background: transparent;
  border: 0;
  border-radius: 6px;
  padding: 6px;
  margin-bottom: 6px;
}
.item:hover,
.item.busy { background: var(--bg3); }
.item.picked {
  background: rgba(245, 243, 194, 0.16);
  box-shadow: inset 0 0 0 2px #f5f3c2;
}
.thumb-wrap {
  position: relative;
  width: 48px;
  height: 64px;
}
.thumb {
  padding: 0;
  border: 0;
  background: transparent;
  width: 48px;
  height: 64px;
}
.tick {
  position: absolute;
  left: 2px;
  top: 2px;
  z-index: 1;
  width: 18px;
  height: 18px;
  padding: 0;
  display: grid;
  place-items: center;
  border: 2px solid #f5f3c2;
  border-radius: 4px;
  background: rgba(14, 15, 33, 0.88);
  color: #0e0f21;
  font-size: 13px;
  font-weight: 800;
  line-height: 1;
}
.tick.on { background: #f5f3c2; }
.item.picked img { box-shadow: 0 0 0 2px #f5f3c2; }
.item img { width: 48px; height: 64px; object-fit: cover; border-radius: 4px; }
.meta { min-width: 0; }
.title {
  display: block;
  width: 100%;
  padding: 0;
  border: 0;
  background: transparent;
  color: inherit;
  font-size: 13px;
  text-align: left;
}
.title:hover { color: #f5f3c2; }
.sub { color: var(--muted); font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.group-pick { margin-top: 6px; font-size: 11px; }
.acts {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-top: 6px;
}
.import {
  background: #f5f3c2;
  color: #0e0f21;
  border: 0;
  border-radius: 4px;
  padding: 3px 8px;
  font-size: 11px;
  font-weight: 700;
}
.icon {
  width: 22px;
  height: 22px;
  padding: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--bg3);
  border-radius: 4px;
  background: transparent;
  color: var(--muted);
}
.icon:hover { color: #fff; background: #1c1e38; }
.icon.danger:hover { color: #f3b4b4; border-color: #5a2f3a; }
</style>
