<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import {
  referenceCatalogDownload,
  referenceCatalogLoad,
  referencePresetDelete,
  referencePresetList,
  referencePresetSave,
  type CatalogAsset,
  type CatalogManifest,
  type ReferencePreset,
} from "@/api/tauri";
import { useAppStore } from "@/stores/app";

const store = useAppStore();
const router = useRouter();
const tab = ref<"online" | "local">("online");
const presets = ref<ReferencePreset[]>([]);
const catalog = ref<CatalogManifest | null>(null);
const name = ref("");
const group = ref("");
const error = ref("");
const saving = ref(false);
const loading = ref(false);
const query = ref("");
const game = ref("__all__");
const category = ref("__all__");
const shownCount = ref(24);
const scroller = ref<HTMLElement | null>(null);
const sentinel = ref<HTMLElement | null>(null);
const busyId = ref("");
const progress = ref<Record<string, string>>({});

const groups = computed(() => [...new Set(presets.value.map((p) => p.group).filter(Boolean))]);
const filter = ref("");
const shown = computed(() => (filter.value ? presets.value.filter((p) => p.group === filter.value) : presets.value));
const downloaded = computed(() => new Set(presets.value.map((p) => p.sourceId).filter(Boolean)));
const games = computed(() => catalog.value?.games ?? []);
const categories = computed(() => {
  if (game.value === "__all__") return [...new Set((catalog.value?.assets ?? []).map((a) => a.category))];
  return games.value.find((g) => g.id === game.value)?.categories ?? [];
});
const filtered = computed(() => {
  const q = query.value.trim().toLowerCase();
  return (catalog.value?.assets ?? []).filter((item) =>
    (game.value === "__all__" || item.game === game.value)
    && (category.value === "__all__" || item.category === category.value)
    && (!q || item.search.includes(q) || item.name.toLowerCase().includes(q)),
  );
});
const visible = computed(() => filtered.value.slice(0, shownCount.value));
const hasMore = computed(() => shownCount.value < filtered.value.length);
let observer: IntersectionObserver | null = null;

function loadMore() {
  if (!hasMore.value) return;
  shownCount.value = Math.min(filtered.value.length, shownCount.value + 24);
}

function bindScroll() {
  observer?.disconnect();
  if (!scroller.value || !sentinel.value) return;
  observer = new IntersectionObserver((entries) => {
    if (entries.some((entry) => entry.isIntersecting)) loadMore();
  }, { root: scroller.value, rootMargin: "280px" });
  observer.observe(sentinel.value);
}

watch([query, game, category], () => {
  shownCount.value = 24;
});

watch([tab, catalog, shownCount, loading], async () => {
  await nextTick();
  bindScroll();
});

async function refreshLocal() {
  presets.value = await referencePresetList();
}

async function loadCatalog(refresh = false) {
  loading.value = true;
  error.value = "";
  try {
    catalog.value = await referenceCatalogLoad(refresh);
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    loading.value = false;
  }
}

function formatBytes(n: number) {
  if (!n) return "";
  return n < 1024 * 1024 ? `${Math.ceil(n / 1024)} KB` : `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

async function saveCurrent() {
  saving.value = true;
  error.value = "";
  try {
    await referencePresetSave({
      id: "",
      name: name.value.trim() || `参考 ${presets.value.length + 1}`,
      group: group.value.trim(),
      createdAt: "",
      vibeImages: store.vibeImages.map((v) => ({
        previewUrl: v.previewUrl,
        base64: v.base64,
        infoExtracted: v.infoExtracted,
        strength: v.strength,
        enabled: v.enabled,
        name: v.name,
      })),
      preciseReferences: store.preciseReferences.map((p) => ({
        previewUrl: p.previewUrl,
        base64: p.base64,
        type: p.type,
        strength: p.strength,
        fidelity: p.fidelity,
        enabled: p.enabled,
      })),
      normalizeVibe: store.normalizeVibe,
    });
    name.value = "";
    await refreshLocal();
    store.status = "已保存参考预设";
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e);
  } finally {
    saving.value = false;
  }
}

function applyPrecise(item: ReferencePreset) {
  const shot = item.preciseReferences[0];
  if (!shot) return false;
  const dataUrl = shot.previewUrl || `data:image/png;base64,${shot.base64}`;
  if (!store.addPreciseFromDataUrl(dataUrl)) return false;
  const last = store.preciseReferences[store.preciseReferences.length - 1];
  if (last) {
    store.updatePreciseReference(last.id, {
      type: (shot.type as "character" | "style" | "character&style") || "character",
      strength: shot.strength,
      fidelity: shot.fidelity,
    });
  }
  store.status = `已加入精准参考「${item.name}」`;
  return true;
}

async function downloadAsset(asset: CatalogAsset, apply = true) {
  if (downloaded.value.has(asset.id)) {
    const existing = presets.value.find((p) => p.sourceId === asset.id);
    if (existing && apply) {
      applyPrecise(existing);
      await router.push("/");
    }
    return;
  }
  busyId.value = asset.id;
  progress.value = { ...progress.value, [asset.id]: "下载中…" };
  try {
    const base64 = await referenceCatalogDownload(asset.downloadUrls);
    const dataUrl = `data:image/png;base64,${base64}`;
    const saved = await referencePresetSave({
      id: "",
      name: asset.name,
      group: `${asset.game} · ${asset.category}`,
      createdAt: "",
      sourceId: asset.id,
      vibeImages: [],
      preciseReferences: [{
        previewUrl: dataUrl,
        base64,
        type: "character",
        strength: 1,
        fidelity: 1,
        enabled: true,
      }],
      normalizeVibe: true,
    });
    await refreshLocal();
    progress.value = { ...progress.value, [asset.id]: "已下载" };
    store.status = `已下载「${asset.name}」到本机预设`;
    if (apply) {
      applyPrecise(saved);
      await router.push("/");
    }
  } catch (e) {
    progress.value = { ...progress.value, [asset.id]: e instanceof Error ? e.message : String(e) };
  } finally {
    busyId.value = "";
  }
}

async function apply(item: ReferencePreset) {
  if (item.sourceId && item.preciseReferences.length && !item.vibeImages.length) applyPrecise(item);
  else store.applyReferencePreset(item);
  await router.push("/");
}

async function remove(id: string) {
  await referencePresetDelete(id);
  await refreshLocal();
}

onMounted(async () => {
  await refreshLocal();
  await loadCatalog();
  await nextTick();
  bindScroll();
});

onUnmounted(() => observer?.disconnect());
</script>

<template>
  <div class="page">
    <header class="head">
      <h2>参考预设</h2>
    </header>

    <div class="tabs">
      <button type="button" :class="{ on: tab === 'online' }" @click="tab = 'online'">在线目录{{ catalog ? ` ${catalog.assets.length}` : "" }}</button>
      <button type="button" :class="{ on: tab === 'local' }" @click="tab = 'local'">本机预设 {{ presets.length }}</button>
    </div>
    <p v-if="error" class="err">{{ error }}</p>

    <section v-if="tab === 'online'" class="body">
      <div class="bar">
        <select v-model="game" @change="category = '__all__'">
          <option value="__all__">全部游戏</option>
          <option v-for="item in games" :key="item.id" :value="item.id">{{ item.name }}</option>
        </select>
        <select v-model="category">
          <option value="__all__">全部分类</option>
          <option v-for="item in categories" :key="item" :value="item">{{ item }}</option>
        </select>
        <input v-model="query" placeholder="搜索角色、形态或游戏" />
        <button type="button" class="act" :disabled="loading" @click="loadCatalog(true)">{{ loading ? "读取中…" : catalog ? "刷新目录" : "读取在线目录" }}</button>
      </div>
      <p class="hint">{{ loading && !catalog ? "正在读取在线目录…" : `当前 ${filtered.length} 条 · 已下载 ${downloaded.size}` }}</p>
      <div ref="scroller" class="scroller">
      <div class="grid online">
        <article v-for="asset in visible" :key="asset.id" class="card">
          <img :src="asset.thumbnailUrl" :alt="asset.name" @error="($event.target as HTMLImageElement).style.opacity = '0.2'" />
          <b>{{ asset.name }}</b>
          <small>{{ asset.game }} · {{ asset.category }}{{ asset.bytes ? ` · ${formatBytes(asset.bytes)}` : "" }}</small>
          <div class="row">
            <button type="button" :disabled="busyId === asset.id" @click="downloadAsset(asset, true)">
              {{ downloaded.has(asset.id) ? "套用到生成" : busyId === asset.id ? "下载中…" : "下载并套用" }}
            </button>
            <button v-if="!downloaded.has(asset.id)" type="button" :disabled="busyId === asset.id" @click="downloadAsset(asset, false)">只下载</button>
          </div>
          <em v-if="progress[asset.id]">{{ progress[asset.id] }}</em>
        </article>
      </div>
      <div ref="sentinel" class="sentinel" />
      <p v-if="hasMore" class="hint more">拉到底继续加载</p>
      <p v-else-if="visible.length" class="hint more">已经到底了 · {{ visible.length }} 条</p>
      </div>
    </section>

    <section v-else class="body">
      <div class="save">
        <input v-model="name" placeholder="预设名称" />
        <input v-model="group" placeholder="分组（可选）" list="groups" />
        <datalist id="groups">
          <option v-for="g in groups" :key="g" :value="g" />
        </datalist>
        <button type="button" class="act" :disabled="saving" @click="saveCurrent">保存当前参考</button>
      </div>
      <div class="filters">
        <button type="button" :class="{ on: !filter }" @click="filter = ''">全部 {{ presets.length }}</button>
        <button v-for="g in groups" :key="g" type="button" :class="{ on: filter === g }" @click="filter = g">{{ g }}</button>
      </div>
      <div class="scroller">
      <p v-if="!shown.length" class="hint">本机还没有预设。到「在线目录」下载角色图，或先在生成页加参考再保存。</p>
      <div class="grid">
        <article v-for="item in shown" :key="item.id" class="card">
          <div class="thumbs">
            <img v-for="(img, i) in [...item.vibeImages, ...item.preciseReferences].slice(0, 4)" :key="i" :src="img.previewUrl || `data:image/png;base64,${img.base64}`" alt="" />
          </div>
          <b>{{ item.name }}</b>
          <small>{{ item.group || "未分组" }} · 氛围 {{ item.vibeImages.length }} · 精准 {{ item.preciseReferences.length }}</small>
          <div class="row">
            <button type="button" @click="apply(item)">套用到生成</button>
            <button type="button" class="danger" @click="remove(item.id)">删除</button>
          </div>
        </article>
      </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.page { height: 100%; min-height: 0; display: flex; flex-direction: column; overflow: hidden; padding: 12px 16px 0; }
.head { flex: none; }
h2 { margin: 0; font-size: 18px; }
p, .hint { margin: 0 0 8px; color: rgba(255,255,255,0.5); font-size: 12px; line-height: 1.4; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.body { flex: 1; min-height: 0; display: flex; flex-direction: column; }
.scroller { flex: 1; min-height: 0; overflow: auto; padding-bottom: 16px; }
.sentinel { height: 1px; }
.hint.more { text-align: center; padding: 12px 0; }
.tabs, .bar, .save, .row, .filters { display: flex; flex-wrap: nowrap; gap: 8px; align-items: center; margin-bottom: 8px; overflow-x: auto; }
.tabs, .bar, .save, .filters { flex: none; }
.tabs button, .act, .filters button, .row button, .pager button {
  border: 0;
  border-radius: 8px;
  background: #2e3152;
  color: var(--heading);
  padding: 8px 10px;
}
.tabs button.on, .filters button.on { background: #3d4270; }
select, input {
  flex: 1;
  min-width: 120px;
  background: #16182d;
  border: 0;
  border-radius: 8px;
  color: #fff;
  padding: 8px 10px;
}
.err { color: var(--danger); }
.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(180px, 1fr)); gap: 10px; }
.grid.online { grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); }
.card {
  background: #16182d;
  border-radius: 10px;
  padding: 10px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.card img { width: 100%; aspect-ratio: 3 / 4; object-fit: cover; border-radius: 8px; background: #101226; }
.thumbs { display: grid; grid-template-columns: repeat(4, 1fr); gap: 4px; min-height: 48px; }
.thumbs img { aspect-ratio: 1; }
.card b, .card small, .card em { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.card b { font-size: 13px; }
.card small, .card em { color: rgba(255,255,255,0.42); font-size: 11px; font-style: normal; }
.danger { color: var(--danger); }
.pager { justify-content: center; }
.pager button:disabled, .row button:disabled { opacity: 0.4; }
</style>
