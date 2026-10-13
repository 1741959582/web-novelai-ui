<script setup lang="ts">
import { computed, onMounted, onUnmounted, reactive, ref, watch } from "vue";
import {
  ruyiAccounts,
  ruyiClashNodes,
  ruyiEnsure,
  ruyiExitIp,
  ruyiNurtureStart,
  ruyiNurtureStatus,
  ruyiNurtureStop,
  ruyiNodeIp,
  ruyiPixivCreate,
  ruyiPixivDelete,
  ruyiPixivOpen,
  ruyiPixivRefresh,
  ruyiPixivUpdate,
  ruyiProxyMap,
  ruyiProxySet,
  ruyiStatus,
  ruyiTwitterCreate,
  ruyiTwitterDelete,
  ruyiTwitterOpen,
  ruyiTwitterRefresh,
  ruyiTwitterUpdate,
  type FingerprintAccount,
  type RuyiStatus,
} from "@/api/tauri";

type RunState = "logged_in" | "waiting_login" | "logged_out" | "banned";

interface TwitterAccount extends FingerprintAccount {
  state: RunState;
}

const runStatusOptions = [
  { value: "", label: "全部" },
  { value: "logged_in", label: "已登录" },
  { value: "waiting_login", label: "待登录" },
  { value: "logged_out", label: "未登录" },
  { value: "banned", label: "已封号" },
];

const filters = reactive({ runStatus: "", keyword: "" });
const orientation = ref<0 | 1>(1);
const site = ref<"x" | "pixiv">("x");
const accounts = ref<TwitterAccount[]>([]);
const status = ref<RuyiStatus | null>(null);
const loading = ref(false);
const note = ref("");
const menuId = ref("");
const busyId = ref("");
const importOpen = ref(false);
const importBusy = ref(false);
const importForm = reactive({ label: "", proxy: "" });
const importIp = ref("");
const importIpBusy = ref(false);
let importIpSeq = 0;
const clashNodes = ref<string[]>([]);
const clashMessage = ref("");
const proxyMap = ref<Record<string, string>>({});
const exitIps = ref<Record<string, string>>({});
const editOpen = ref(false);
const editBusy = ref(false);
const editForm = reactive({ accountId: "", label: "", cookie: "" });
const deleteTarget = ref<TwitterAccount | null>(null);
let pollTimer = 0;
let liveTimer = 0;
let nurtureTimer = 0;
const nurtureOpen = ref(false);
const nurtureRunning = ref(false);
const nurtureLog = ref<string[]>([]);
const nurtureIds = ref<string[]>([]);
const nurtureForm = reactive({
  mode: "browse" as "browse" | "interact",
  feed: "for_you" as "for_you" | "following",
  rounds: 8,
  likes: 3,
  reposts: 1,
  follows: 2,
  delaySecs: 5,
  handles: "",
});
const frameUrls = ref<Record<string, string>>({});
const liveIds = ref<Record<string, boolean>>({});
const liveAt = new Map<string, number>();
const streamEpoch = ref<Record<string, number>>({});
const streamTasks = new Map<string, AbortController>();

function findBytes(buf: Uint8Array, marker: number, from = 0) {
  for (let i = from; i < buf.length - 1; i += 1) {
    if (buf[i] === 0xff && buf[i + 1] === marker) return i;
  }
  return -1;
}

function concatBytes(left: Uint8Array, right: Uint8Array) {
  const next = new Uint8Array(left.length + right.length);
  next.set(left, 0);
  next.set(right, left.length);
  return next;
}

function abortStream(id: string) {
  streamTasks.get(id)?.abort();
  streamTasks.delete(id);
}

function markLive(id: string, on: boolean) {
  if (!on) liveAt.delete(id);
  if (!!liveIds.value[id] === on) return;
  liveIds.value = { ...liveIds.value, [id]: on };
}

function stopStream(id: string) {
  abortStream(id);
  markLive(id, false);
  const prev = frameUrls.value[id];
  if (prev) URL.revokeObjectURL(prev);
  const next = { ...frameUrls.value };
  delete next[id];
  frameUrls.value = next;
}

async function startStream(id: string) {
  abortStream(id);
  const epoch = Date.now();
  streamEpoch.value = { ...streamEpoch.value, [id]: epoch };
  const controller = new AbortController();
  streamTasks.set(id, controller);
  const port = status.value?.port || 8765;
  const url = `http://127.0.0.1:${port}/run/browser/${encodeURIComponent(id)}/stream?fps=8&quality=0.45&t=${epoch}`;
  try {
    const response = await fetch(url, { signal: controller.signal, cache: "no-store" });
    if (!response.ok || !response.body) throw new Error(`预览流 HTTP ${response.status}`);
    const reader = response.body.getReader();
    let buf = new Uint8Array(0);
    while (!controller.signal.aborted) {
      const chunk = await reader.read();
      if (chunk.done) break;
      buf = concatBytes(buf, chunk.value);
      while (buf.length > 4) {
        const start = findBytes(buf, 0xd8);
        if (start < 0) {
          buf = buf.slice(-3);
          break;
        }
        const end = findBytes(buf, 0xd9, start + 2);
        if (end < 0) {
          if (start > 0) buf = buf.slice(start);
          break;
        }
        const jpeg = buf.slice(start, end + 2);
        buf = buf.slice(end + 2);
        if (controller.signal.aborted) break;
        const nextUrl = URL.createObjectURL(new Blob([jpeg], { type: "image/jpeg" }));
        const prev = frameUrls.value[id];
        frameUrls.value = { ...frameUrls.value, [id]: nextUrl };
        liveAt.set(id, Date.now());
        markLive(id, true);
        if (prev) URL.revokeObjectURL(prev);
      }
    }
  } catch {
    /* 浏览器关掉或重新打开时，旧连接会中断 */
  } finally {
    if (streamTasks.get(id) === controller) {
      streamTasks.delete(id);
      markLive(id, false);
      if (!controller.signal.aborted) {
        window.setTimeout(() => {
          const stillOpen = accounts.value.some((row) => row.account_id === id && row.browser_open);
          if (stillOpen && !streamTasks.has(id)) void startStream(id);
        }, 1000);
      }
    }
  }
}

const openAccountKey = computed(() =>
  accounts.value
    .filter((row) => row.browser_open)
    .map((row) => row.account_id)
    .sort()
    .join("|"),
);

watch(openAccountKey, () => {
  const open = new Set(
    accounts.value.filter((row) => row.browser_open).map((row) => row.account_id),
  );
  for (const id of streamTasks.keys()) {
    if (!open.has(id)) stopStream(id);
  }
  for (const id of open) {
    if (!streamTasks.has(id)) void startStream(id);
  }
});

watch(site, () => {
  menuId.value = "";
  nurtureOpen.value = false;
  void loadAccounts();
});

const visibleAccounts = computed(() => {
  const keyword = filters.keyword.trim().toLowerCase();
  return accounts.value.filter((row) => {
    if (filters.runStatus && row.state !== filters.runStatus) return false;
    if (!keyword) return true;
    return [row.label, row.username, row.account_id, row.profile_dir]
      .join("\n")
      .toLowerCase()
      .includes(keyword);
  });
});

function errorText(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

function cookieMap(header: string) {
  const map = new Map<string, string>();
  const raw = header.trim();
  if (!raw) return map;
  const text = raw.replace(/^cookie:\s*/i, "");
  for (const part of text.includes(";") ? text.split(";") : text.split(/\r?\n/)) {
    const item = part.trim();
    const eq = item.indexOf("=");
    if (eq > 0) map.set(item.slice(0, eq).trim(), item.slice(eq + 1).trim());
  }
  return map;
}

function isTwitter(row: FingerprintAccount) {
  const type = (row.type || "").toLowerCase();
  return type === "x" || type === "twitter";
}

function isPixiv(row: FingerprintAccount) {
  return (row.type || "").toLowerCase() === "pixiv";
}

function onThisSite(row: FingerprintAccount) {
  return site.value === "pixiv" ? isPixiv(row) : isTwitter(row);
}

function sessionLoggedIn(row: FingerprintAccount) {
  const cookies = cookieMap(row.cookie_header || "");
  if (isPixiv(row)) return /^\d+_[A-Za-z0-9]{8,}/.test(cookies.get("PHPSESSID") || "");
  return (cookies.get("auth_token") || "").trim().length >= 8;
}

function mapState(row: FingerprintAccount): RunState {
  const statusText = (row.status || "").toLowerCase();
  if (statusText === "logged_in" || statusText === "active" || statusText === "running") return "logged_in";
  if (statusText === "banned") return "banned";
  if (statusText === "logged_out") return "logged_out";
  if (sessionLoggedIn(row)) return "logged_in";
  return "waiting_login";
}

function stateLabel(state: RunState, raw?: string) {
  if ((raw || "").toLowerCase() === "pending") return "待启动";
  if (state === "logged_in") return "已登录";
  if (state === "logged_out") return "未登录";
  if (state === "banned") return "已封号";
  return "待登录";
}

async function loadAccounts(silent = false) {
  if (!silent) loading.value = true;
  try {
    const store = await ruyiAccounts();
    accounts.value = (store.accounts || []).filter(onThisSite).map((row) => ({ ...row, state: mapState(row) }));
  } catch (error) {
    if (!silent) note.value = errorText(error);
  } finally {
    if (!silent) loading.value = false;
  }
}

async function refreshAll() {
  status.value = await ruyiStatus();
  await loadAccounts();
  note.value = "列表已刷新";
}

function openImport() {
  importForm.label = "";
  importForm.proxy = "";
  importIp.value = "";
  importOpen.value = true;
  void previewImportIp();
}

async function previewImportIp() {
  const seq = ++importIpSeq;
  const node = importForm.proxy;
  importIpBusy.value = true;
  try {
    const result = await ruyiNodeIp(node);
    if (seq !== importIpSeq) return;
    importIp.value = result.ip;
  } catch (error) {
    if (seq !== importIpSeq) return;
    importIp.value = "";
    note.value = errorText(error);
  } finally {
    if (seq === importIpSeq) importIpBusy.value = false;
  }
}

async function loadClash() {
  try {
    const [nodes, saved] = await Promise.all([ruyiClashNodes(), ruyiProxyMap()]);
    clashNodes.value = nodes.nodes || [];
    clashMessage.value = nodes.message;
    proxyMap.value = saved || {};
  } catch (error) {
    clashMessage.value = errorText(error);
  }
}

async function checkIp(accountId: string) {
  busyId.value = `ip-${accountId}`;
  try {
    const result = await ruyiExitIp(accountId);
    exitIps.value = { ...exitIps.value, [accountId]: result.ip };
    note.value = `${result.via} 的出口 IP 是 ${result.ip}。在这个指纹浏览器里打开 ${result.checkUrl} 看到的是同一个地址。`;
  } catch (error) {
    note.value = errorText(error);
  } finally {
    busyId.value = "";
  }
}

async function setProxy(accountId: string, proxyName: string) {
  proxyMap.value = { ...proxyMap.value, [accountId]: proxyName };
  try {
    await ruyiProxySet(accountId, proxyName);
    note.value = proxyName ? `已记住节点 ${proxyName}，下次打开这个窗口时走该 IP` : "已改回本机直连，下次打开时生效";
  } catch (error) {
    note.value = errorText(error);
  }
}

function labelTaken(label: string, excludeId = "") {
  const key = label.trim();
  return accounts.value.some((row) => row.account_id !== excludeId && (row.label || "").trim() === key);
}

async function submitImport() {
  const label = importForm.label.trim();
  if (!label) {
    note.value = "请输入备注名";
    return;
  }
  if (labelTaken(label)) {
    note.value = "该备注名已被其他账号使用";
    return;
  }
  importBusy.value = true;
  try {
    const created = site.value === "pixiv"
      ? await ruyiPixivCreate(label, "", importForm.proxy)
      : await ruyiTwitterCreate(label, "", importForm.proxy);
    importOpen.value = false;
    await Promise.all([loadAccounts(), loadClash()]);
    if (created.accountId && created.ip) {
      exitIps.value = { ...exitIps.value, [created.accountId]: created.ip };
    }
    note.value = created.ip
      ? `已把 ${created.via} 写入指纹浏览器，出口 IP ${created.ip}`
      : `已把 ${created.via} 写入指纹浏览器，出口 IP 暂时没有查到`;
  } catch (error) {
    note.value = errorText(error);
    await loadAccounts();
  } finally {
    importBusy.value = false;
  }
}

function openEdit(row: TwitterAccount) {
  menuId.value = "";
  editForm.accountId = row.account_id;
  editForm.label = row.label || "";
  editForm.cookie = row.cookie_header || "";
  editOpen.value = true;
}

async function submitEdit() {
  const label = editForm.label.trim();
  if (!label) {
    note.value = "请输入备注名";
    return;
  }
  if (labelTaken(label, editForm.accountId)) {
    note.value = "该备注名已被其他账号使用";
    return;
  }
  editBusy.value = true;
  try {
    await (site.value === "pixiv" ? ruyiPixivUpdate : ruyiTwitterUpdate)(editForm.accountId, label, editForm.cookie.trim());
    if (editForm.cookie.trim()) {
      await (site.value === "pixiv" ? ruyiPixivOpen : ruyiTwitterOpen)(editForm.accountId);
      note.value = site.value === "pixiv" ? "账号已保存，正在用 Cookie 打开 Pixiv" : "账号已保存，正在用 Cookie 打开推特";
    } else {
      note.value = "账号已保存";
    }
    editOpen.value = false;
    await loadAccounts();
  } catch (error) {
    note.value = errorText(error);
  } finally {
    editBusy.value = false;
  }
}

async function openBrowser(row: TwitterAccount) {
  menuId.value = "";
  busyId.value = row.account_id;
  try {
    const opened = await (site.value === "pixiv" ? ruyiPixivOpen : ruyiTwitterOpen)(row.account_id);
    note.value = opened.proxy ? `请求已发送，${opened.proxy}` : "请求已发送";
    await loadAccounts(true);
  } catch (error) {
    note.value = errorText(error);
  } finally {
    busyId.value = "";
  }
}

async function refreshAccount(row: TwitterAccount) {
  menuId.value = "";
  busyId.value = row.account_id;
  try {
    await (site.value === "pixiv" ? ruyiPixivRefresh : ruyiTwitterRefresh)(row.account_id);
    note.value = "更新请求已提交";
    await loadAccounts();
  } catch (error) {
    note.value = errorText(error);
  } finally {
    busyId.value = "";
  }
}

async function confirmDelete() {
  const row = deleteTarget.value;
  if (!row) return;
  busyId.value = row.account_id;
  try {
    await (site.value === "pixiv" ? ruyiPixivDelete : ruyiTwitterDelete)(row.account_id);
    note.value = "删除请求已提交";
    deleteTarget.value = null;
    await loadAccounts();
  } catch (error) {
    note.value = errorText(error);
  } finally {
    busyId.value = "";
  }
}

async function pollNurture() {
  try {
    const status = await ruyiNurtureStatus();
    nurtureRunning.value = status.running;
    nurtureLog.value = status.log;
    if (status.message) note.value = status.message;
  } catch (error) {
    note.value = errorText(error);
  }
}

function openNurture() {
  const loggedIn = accounts.value.filter((row) => row.state === "logged_in").map((row) => row.account_id);
  nurtureIds.value = (loggedIn.length ? loggedIn : accounts.value.map((row) => row.account_id)).slice(0, 4);
  nurtureOpen.value = true;
  void pollNurture();
  if (nurtureTimer) window.clearInterval(nurtureTimer);
  nurtureTimer = window.setInterval(() => {
    if (nurtureOpen.value || nurtureRunning.value) void pollNurture();
  }, 1000);
}

function toggleNurtureAccount(id: string) {
  if (nurtureIds.value.includes(id)) {
    nurtureIds.value = nurtureIds.value.filter((item) => item !== id);
    return;
  }
  if (nurtureIds.value.length >= 4) {
    note.value = "一次最多选 4 个账号";
    return;
  }
  nurtureIds.value = [...nurtureIds.value, id];
}

async function startNurture() {
  try {
    const status = await ruyiNurtureStart({
      mode: nurtureForm.mode,
      accountIds: nurtureIds.value,
      handles: nurtureForm.handles,
      feed: nurtureForm.mode === "interact" ? "following" : nurtureForm.feed,
      rounds: Number(nurtureForm.rounds) || 8,
      likes: Number(nurtureForm.likes) || 0,
      reposts: Number(nurtureForm.reposts) || 0,
      follows: Number(nurtureForm.follows) || 0,
      delaySecs: Number(nurtureForm.delaySecs) || 5,
    });
    nurtureRunning.value = status.running;
    nurtureLog.value = status.log;
    note.value = status.message || "养号已开始";
  } catch (error) {
    note.value = errorText(error);
  }
}

async function stopNurture() {
  try {
    const status = await ruyiNurtureStop();
    nurtureRunning.value = status.running;
    nurtureLog.value = status.log;
    note.value = status.message || "已停止";
  } catch (error) {
    note.value = errorText(error);
  }
}

onMounted(async () => {
  try {
    status.value = await ruyiStatus();
    if (!status.value.running) status.value = await ruyiEnsure();
  } catch (error) {
    note.value = errorText(error);
  }
  await Promise.all([loadAccounts(), loadClash()]);
  pollTimer = window.setInterval(() => {
    void loadAccounts(true);
  }, 3000);
  liveTimer = window.setInterval(() => {
    const now = Date.now();
    for (const [id, at] of liveAt) {
      if (now - at > 1500) markLive(id, false);
    }
  }, 500);
});

onUnmounted(() => {
  if (pollTimer) window.clearInterval(pollTimer);
  if (liveTimer) window.clearInterval(liveTimer);
  if (nurtureTimer) window.clearInterval(nurtureTimer);
  if (nurtureRunning.value) void ruyiNurtureStop();
  for (const id of [...streamTasks.keys()]) stopStream(id);
});
</script>

<template>
  <div class="page" @click="menuId = ''">
    <section class="bar">
      <label>
        运行状态
        <select v-model="filters.runStatus">
          <option v-for="item in runStatusOptions" :key="item.value" :value="item.value">{{ item.label }}</option>
        </select>
      </label>
      <label>
        关键词
        <input v-model="filters.keyword" placeholder="账号名称" />
      </label>
    </section>

    <section class="panel">
      <div class="toolbar">
        <div class="tools">
          <div class="orient sites">
            <button type="button" :class="{ on: site === 'x' }" @click="site = 'x'">推特</button>
            <button type="button" :class="{ on: site === 'pixiv' }" @click="site = 'pixiv'">Pixiv</button>
          </div>
          <button class="btn primary" type="button" @click.stop="openImport">添加/导入</button>
          <button v-if="site === 'x'" class="btn" type="button" @click.stop="openNurture">养号</button>
          <button class="btn" type="button" @click.stop="note = '账号设置稍后接入'">账号设置</button>
          <button class="btn" type="button" :disabled="loading" @click.stop="refreshAll">刷新</button>
        </div>
        <div class="orient">
          <span>预览布局</span>
          <button type="button" :class="{ on: orientation === 0 }" @click="orientation = 0">横屏</button>
          <button type="button" :class="{ on: orientation === 1 }" @click="orientation = 1">竖屏</button>
        </div>
      </div>
      <p v-if="clashMessage" class="hint proxy-hint">{{ clashMessage }}。不选节点就用本机 IP，选了以后每次打开都走这个节点。</p>

      <div class="grid" :class="{ tall: orientation === 1 }">
        <p v-if="loading" class="empty">正在读取{{ site === "pixiv" ? " Pixiv " : "推特" }}账号</p>
        <p v-else-if="!visibleAccounts.length" class="empty">还没有{{ site === "pixiv" ? " Pixiv " : "推特" }}账号。点「添加/导入」创建。</p>
        <article v-for="row in visibleAccounts" :key="row.account_id" class="card" @click.stop>
          <div class="preview" :class="{ portrait: orientation === 1 }">
            <img
              v-if="frameUrls[row.account_id]"
              :key="streamEpoch[row.account_id]"
              :src="frameUrls[row.account_id]"
              alt=""
            />
            <div v-else class="placeholder">
              <b>{{ site === "pixiv" ? "P" : "X" }}</b>
              <span>{{ row.browser_open ? "正在连接预览" : "浏览器未打开" }}</span>
            </div>
            <i class="live" :class="{ on: liveIds[row.account_id] }"><s />LIVE</i>
            <div v-if="busyId === row.account_id" class="busy">处理中</div>
          </div>
          <div class="info">
            <div class="avatar">{{ (row.label || row.username || (site === "pixiv" ? "P" : "X")).slice(0, 1) }}</div>
            <div class="text">
              <strong>{{ row.label || row.username || row.account_id }}</strong>
              <span>{{ site === "x" && row.username ? `@${row.username}` : (row.username || row.last_error || (site === "pixiv" ? "Pixiv" : "推特")) }}</span>
              <select :value="proxyMap[row.account_id] || ''" @change="setProxy(row.account_id, ($event.target as HTMLSelectElement).value)">
                <option value="">本机直连</option>
                <option v-for="node in clashNodes" :key="node" :value="node">{{ node }}</option>
              </select>
              <span class="ip-line">
                出口 IP {{ exitIps[row.account_id] || "未检测" }}
                <button type="button" :disabled="!!busyId" @click="checkIp(row.account_id)">查看</button>
              </span>
            </div>
            <em :class="row.state">{{ stateLabel(row.state, row.status) }}</em>
          </div>
          <div class="actions">
            <button class="btn primary" type="button" :disabled="!!busyId" @click="openBrowser(row)">
              {{ row.state === "logged_in" ? "打开" : "登录" }}
            </button>
            <button class="btn more" type="button" @click.stop="menuId = menuId === row.account_id ? '' : row.account_id">···</button>
            <div v-if="menuId === row.account_id" class="menu">
              <button type="button" @click="openEdit(row)">编辑账号</button>
              <button type="button" @click="refreshAccount(row)">更新账号</button>
              <button type="button" class="danger" @click="menuId = ''; deleteTarget = row">删除账号</button>
            </div>
          </div>
        </article>
      </div>
    </section>

    <p v-if="note" class="note">{{ note }}</p>

    <div v-if="importOpen" class="mask" @click.self="importOpen = false">
      <form class="dialog" @submit.prevent="submitImport">
        <h3>导入{{ site === "pixiv" ? " Pixiv " : "推特" }}指纹浏览器</h3>
        <label>
          备注名
          <input v-model="importForm.label" maxlength="128" placeholder="请输入备注名" />
        </label>
        <label>
          代理节点
          <select v-model="importForm.proxy" @change="previewImportIp">
            <option value="">本机直连</option>
            <option v-for="node in clashNodes" :key="node" :value="node">{{ node }}</option>
          </select>
        </label>
        <p class="hint">出口 IP {{ importIpBusy ? "查询中…" : (importIp || "未检测") }}。保存后这个节点会写进指纹浏览器，窗口打开就走这个 IP。</p>
        <div class="dialog-actions">
          <button class="btn" type="button" @click="importOpen = false">取消</button>
          <button class="btn primary" type="submit" :disabled="importBusy">{{ importBusy ? "保存中…" : "保存" }}</button>
        </div>
      </form>
    </div>

    <div v-if="editOpen" class="mask" @click.self="editOpen = false">
      <form class="dialog" @submit.prevent="submitEdit">
        <h3>编辑账号</h3>
        <label>
          备注名
          <input v-model="editForm.label" maxlength="128" placeholder="请输入备注名" />
        </label>
        <label>
          Cookie
          <textarea v-model="editForm.cookie" rows="4" :placeholder="site === 'pixiv' ? 'PHPSESSID=用户编号_令牌' : 'auth_token=...; ct0=...'" />
        </label>
        <p class="hint">修改后下次打开{{ site === "pixiv" ? " Pixiv " : "推特" }}指纹浏览器时生效。登录成功后，这个账号的指纹配置会留在本机 profile 里。</p>
        <div class="dialog-actions">
          <button class="btn" type="button" @click="editOpen = false">取消</button>
          <button class="btn primary" type="submit" :disabled="editBusy">{{ editBusy ? "保存中…" : "保存" }}</button>
        </div>
      </form>
    </div>

    <div v-if="nurtureOpen" class="mask" @click.self="nurtureOpen = false">
      <form class="dialog nurture" @submit.prevent="startNurture">
        <h3>推特养号</h3>
        <div class="seg">
          <button type="button" :class="{ on: nurtureForm.mode === 'browse' }" @click="nurtureForm.mode = 'browse'">浏览</button>
          <button type="button" :class="{ on: nurtureForm.mode === 'interact' }" @click="nurtureForm.mode = 'interact'">点赞转发关注</button>
        </div>
        <div class="checks">
          <label v-for="row in accounts" :key="row.account_id" class="check">
            <input
              type="checkbox"
              :checked="nurtureIds.includes(row.account_id)"
              @change="toggleNurtureAccount(row.account_id)"
            />
            <span>{{ row.label || row.username || row.account_id }}</span>
          </label>
        </div>
        <label v-if="nurtureForm.mode === 'browse'">
          时间线
          <select v-model="nurtureForm.feed">
            <option value="for_you">为你推荐</option>
            <option value="following">正在关注</option>
          </select>
        </label>
        <label>
          {{ nurtureForm.mode === "browse" ? "浏览轮数" : "每个目标滚动轮数" }}
          <input v-model.number="nurtureForm.rounds" type="number" min="1" max="40" />
        </label>
        <div v-if="nurtureForm.mode === 'interact'" class="counts">
          <label>点赞<input v-model.number="nurtureForm.likes" type="number" min="0" max="15" /></label>
          <label>转发<input v-model.number="nurtureForm.reposts" type="number" min="0" max="8" /></label>
          <label>关注<input v-model.number="nurtureForm.follows" type="number" min="0" max="15" /></label>
        </div>
        <label v-if="nurtureForm.mode === 'interact'">
          用户或帖子
          <textarea v-model="nurtureForm.handles" rows="4" placeholder="每行一个用户名，或一条 x.com 帖子链接。留空就处理正在关注的时间线。" />
        </label>
        <label>
          动作间隔（秒）
          <input v-model.number="nurtureForm.delaySecs" type="number" min="3" max="20" />
        </label>
        <p class="hint">浏览会滚动时间线，并偶尔点开帖子再返回。互动会点赞、转发；填了用户名时还会去对方主页关注。多个账号按顺序一个一个养。账号需要已经登录。</p>
        <pre v-if="nurtureLog.length" class="log">{{ nurtureLog.join("\n") }}</pre>
        <div class="dialog-actions">
          <button class="btn" type="button" @click="nurtureOpen = false">关闭</button>
          <button v-if="nurtureRunning" class="btn danger" type="button" @click="stopNurture">停止</button>
          <button class="btn primary" type="submit" :disabled="nurtureRunning">开始</button>
        </div>
      </form>
    </div>

    <div v-if="deleteTarget" class="mask" @click.self="deleteTarget = null">
      <div class="dialog">
        <h3>确认删除该账号？</h3>
        <p>将删除「{{ deleteTarget.label || deleteTarget.account_id }}」并同步服务端数据。</p>
        <div class="dialog-actions">
          <button class="btn" type="button" @click="deleteTarget = null">取消</button>
          <button class="btn danger" type="button" :disabled="!!busyId" @click="confirmDelete">删除</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.page { height: 100%; min-height: 0; display: flex; flex-direction: column; gap: 12px; padding: 16px; box-sizing: border-box; }
.bar, .toolbar, .tools, .orient, .dialog-actions, .actions, .info { display: flex; align-items: center; gap: 10px; }
.bar, .panel { background: var(--bg1); border: 1px solid var(--bg3); border-radius: 12px; }
.bar { padding: 14px 16px; flex-wrap: wrap; }
.panel { flex: 1; min-height: 0; display: flex; flex-direction: column; overflow: hidden; }
.toolbar { justify-content: space-between; gap: 12px; padding: 14px 16px; flex-wrap: nowrap; border-bottom: 1px solid var(--bg3); }
.toolbar:has(+ .proxy-hint) { border-bottom: 0; }
.orient { margin-left: auto; flex-shrink: 0; }
.proxy-hint { margin: 0; padding: 0 16px 12px; border-bottom: 1px solid var(--bg3); }
.text select { min-width: 0; max-width: 180px; padding: 4px 6px; }
.ip-line { display: flex; align-items: center; gap: 8px; color: var(--heading); font-size: 12px; }
.ip-line button { border: 0; background: transparent; color: var(--heading); padding: 0; text-decoration: underline; }
.tools { flex-wrap: wrap; }
label { display: flex; flex-direction: column; gap: 6px; font-size: 13px; color: var(--muted); }
input, select, textarea { min-width: 180px; background: var(--bg0); border: 1px solid var(--bg3); border-radius: 8px; padding: 8px 10px; }
.orient span { color: var(--muted); font-size: 13px; }
.orient button { border: 1px solid var(--bg3); background: transparent; border-radius: 8px; padding: 6px 10px; }
.orient button.on { background: var(--heading); color: var(--bg0); border-color: var(--heading); }
.sites { margin-left: 0; }
.grid { flex: 1; overflow: auto; display: grid; grid-template-columns: repeat(auto-fill, minmax(320px, 1fr)); gap: 16px; align-content: start; padding: 16px; }
.card { display: flex; flex-direction: column; gap: 12px; padding: 14px; border-radius: 22px; background: var(--bg2); border: 1px solid var(--bg3); position: relative; }
.preview { position: relative; border-radius: 16px; overflow: hidden; background: #10111c; aspect-ratio: 16 / 9; }
.preview.portrait { aspect-ratio: 9 / 16; max-height: 420px; }
.preview img { position: absolute; inset: 0; z-index: 1; width: 100%; height: 100%; object-fit: contain; background: #10111c; }
.placeholder { width: 100%; height: 100%; }
.placeholder { display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 8px; color: var(--muted); }
.placeholder b { font-size: 28px; color: var(--heading); }
.live { position: absolute; top: 8px; left: 8px; z-index: 2; display: flex; align-items: center; gap: 6px; font-style: normal; font-size: 10px; font-weight: 700; letter-spacing: 0.08em; padding: 4px 8px; border-radius: 4px; background: rgba(0,0,0,0.6); color: #ef4444; }
.live s { width: 6px; height: 6px; border-radius: 99px; background: #ef4444; text-decoration: none; }
.live.on { color: var(--ok); }
.live.on s { background: var(--ok); }
.busy { position: absolute; inset: 0; display: grid; place-items: center; background: rgba(8, 9, 18, 0.55); }
.info { align-items: flex-start; }
.avatar { width: 40px; height: 40px; border-radius: 10px; display: grid; place-items: center; background: var(--bg0); color: var(--heading); font-weight: 700; }
.text { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 4px; }
.text strong, .text span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.text span, .hint, .empty, .note { color: var(--muted); }
em { font-style: normal; font-size: 12px; }
em.logged_in { color: var(--ok); }
em.banned { color: var(--danger); }
em.waiting_login { color: var(--heading); }
.actions { position: relative; }
.actions .btn.primary { flex: 1; }
.menu { position: absolute; right: 0; bottom: 42px; z-index: 3; min-width: 140px; padding: 6px; border-radius: 10px; background: var(--bg0); border: 1px solid var(--bg3); display: flex; flex-direction: column; }
.menu button { text-align: left; background: transparent; border: 0; padding: 8px 10px; border-radius: 8px; }
.menu button:hover { background: var(--bg3); }
.menu .danger, .btn.danger { color: var(--danger); }
.empty { grid-column: 1 / -1; margin: 24px 0; }
.note { margin: 0; }
.mask { position: fixed; inset: 0; z-index: 20; background: rgba(0,0,0,0.45); display: grid; place-items: center; padding: 20px; }
.dialog { width: min(440px, 100%); display: flex; flex-direction: column; gap: 12px; padding: 18px; border-radius: 16px; background: var(--bg1); border: 1px solid var(--bg3); }
.dialog.nurture { width: min(560px, 100%); max-height: min(86vh, 760px); overflow: auto; }
.seg, .counts, .checks { display: flex; gap: 8px; flex-wrap: wrap; }
.seg button, .check { border: 1px solid var(--bg3); background: transparent; border-radius: 8px; padding: 6px 10px; }
.seg button.on { background: var(--heading); color: var(--bg0); border-color: var(--heading); }
.check { flex-direction: row; align-items: center; min-width: 0; }
.check input { min-width: 0; }
.counts label { flex: 1; min-width: 90px; }
.counts input { width: 100%; min-width: 0; box-sizing: border-box; }
.log { margin: 0; max-height: 140px; overflow: auto; white-space: pre-wrap; font-size: 12px; color: var(--muted); background: var(--bg0); border-radius: 8px; padding: 8px 10px; }
.dialog h3 { margin: 0; }
.dialog label input, .dialog label textarea { width: 100%; box-sizing: border-box; }
.dialog-actions { justify-content: flex-end; }
</style>
