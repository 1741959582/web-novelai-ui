<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useAppStore } from "@/stores/app";
import { useApngStore } from "@/stores/apng";
import { readImageDataUrl } from "@/api/tauri";
import NaiIcon from "@/components/NaiIcon.vue";
import type { HistoryItem } from "@/types/nai";

const store = useAppStore();
const apng = useApngStore();
const thumbs = ref<Record<string, string>>({});
const newGroupName = ref("");
const renameName = ref("");
const renaming = ref(false);
const busyId = ref("");

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
  if (!window.confirm(`删除分组「${group.name}」？图片不会被删除。`)) return;
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

async function addApng(item: HistoryItem) {
  const url = thumbs.value[item.id] || await readImageDataUrl(item.path);
  const ok = await apng.addReal(url, item.id);
  store.status = ok ? `已清掉元数据并加入伪装队列（${apng.items.length}）` : "伪装队列已满";
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

      <p v-if="!items.length" class="hint">
        {{ store.history.length ? "这个分组里还没有图片。" : "生成后的图片会出现在这里。" }}
      </p>
      <div v-for="item in items" :key="item.id" class="item" :class="{ busy: busyId === item.id }">
        <button class="thumb" type="button" title="预览；双击载入参数" @click="store.showHistory(item)" @dblclick="store.applyHistory(item)">
          <img v-if="thumbs[item.id]" :src="thumbs[item.id]" alt="" />
        </button>
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
.hint { color: var(--muted); font-size: 13px; }
.groups {
  display: grid;
  gap: 6px;
  margin-bottom: 12px;
}
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
.thumb {
  padding: 0;
  border: 0;
  background: transparent;
  width: 48px;
  height: 64px;
}
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
