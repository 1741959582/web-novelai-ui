<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, reactive, ref, watch } from "vue";
import * as echarts from "echarts";
import {
  ruyiAccounts,
  ruyiClashNodes,
  ruyiEnsure,
  ruyiExitIp,
  ruyiNurtureCancel,
  ruyiNurtureStart,
  ruyiNurtureStatus,
  ruyiNurtureStop,
  type NurtureTaskStatus,
  ruyiNodeIp,
  ruyiPixivCreate,
  ruyiPixivDelete,
  ruyiPixivLikes,
  ruyiPixivOpen,
  ruyiPixivOpenTab,
  ruyiPixivRefresh,
  ruyiPixivStats,
  ruyiPixivUpdate,
  type PixivDay,
  type PixivLike,
  ruyiProxyMap,
  ruyiProxyProductSet,
  ruyiProxySet,
  type ClashNodes,
  type ProxyChoice,
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
const proxyProduct = ref("");
const proxyChoices = ref<ProxyChoice[]>([
  { id: "freecat", name: "自由猫", running: false },
  { id: "clash", name: "Clash Verge", running: false },
]);
const proxySaving = ref(false);
const proxyHint = computed(() => {
  if (proxyProduct.value === "freecat") {
    if (!clashNodes.value.length && clashMessage.value.includes("外部控制器")) return clashMessage.value;
    return `${clashMessage.value}。下拉列表是自由猫的节点，不选就用本机 IP。自由猫只有一个出口，最后选中的节点会给正在使用它的窗口共用。`;
  }
  if (proxyProduct.value === "clash") {
    return `${clashMessage.value}。下拉列表是 Clash Verge 的节点，不选就用本机 IP。切换节点会改掉 Clash Verge 当前的出口。`;
  }
  return "请选择用自由猫还是 Clash Verge。没打开的不能选。选好之后才读取那个工具的节点，不选节点就用本机 IP。";
});
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
const queueOpen = ref(false);
const queueHost = ref<HTMLElement | null>(null);
const nurtureTasks = ref<NurtureTaskStatus[]>([]);
const nurtureMessage = ref("");
const nurtureIds = ref<string[]>([]);
const nurtureForm = reactive({
  mode: "browse" as "browse" | "interact" | "plaza",
  feed: "for_you" as "for_you" | "following",
  rounds: 8,
  likes: 3,
  reposts: 1,
  follows: 2,
  delaySecs: 5,
  minutes: 10,
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
  await Promise.all([loadAccounts(), loadClash()]);
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

function applyClash(nodes: ClashNodes) {
  clashNodes.value = nodes.ok ? nodes.nodes || [] : [];
  clashMessage.value = nodes.message;
  proxyProduct.value = nodes.product || "";
  if (nodes.choices?.length) proxyChoices.value = nodes.choices;
}

async function loadClash() {
  try {
    const [nodes, saved] = await Promise.all([ruyiClashNodes(), ruyiProxyMap()]);
    applyClash(nodes);
    proxyMap.value = saved || {};
  } catch (error) {
    clashMessage.value = errorText(error);
  }
}

async function chooseProxy(product: string) {
  if (proxySaving.value || proxyProduct.value === product) return;
  proxySaving.value = true;
  try {
    applyClash(await ruyiProxyProductSet(product));
    note.value = product === "clash" ? "已改用 Clash Verge" : "已改用自由猫";
  } catch (error) {
    note.value = errorText(error);
  } finally {
    proxySaving.value = false;
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
  if (site.value === "x" && nurtureLocked(row.account_id)) {
    note.value = "这个账号正在养号，完成或取消后才能打开";
    queueOpen.value = true;
    return;
  }
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

function nurtureLocked(accountId: string) {
  return nurtureTasks.value.some((task) => task.accountId === accountId && (task.status === "queued" || task.status === "running"));
}

function nurtureStatusText(status: string) {
  if (status === "running") return "养号中";
  if (status === "queued") return "等待中";
  if (status === "done") return "已完成";
  if (status === "cancelled") return "已取消";
  if (status === "failed") return "失败";
  return status;
}

function formatRemain(task: NurtureTaskStatus) {
  if (task.status === "queued") return `计划 ${task.minutes} 分钟`;
  if (task.status !== "running") return task.message || nurtureStatusText(task.status);
  const total = Math.max(0, task.remainingSecs);
  const minutes = Math.floor(total / 60);
  const seconds = total % 60;
  return `剩余 ${minutes} 分 ${String(seconds).padStart(2, "0")} 秒`;
}

const nurtureActiveTasks = computed(() =>
  nurtureTasks.value.filter((task) => task.status === "queued" || task.status === "running" || task.status === "failed"),
);
const nurtureActiveCount = computed(() => nurtureActiveTasks.value.length);
const detailId = ref(0);
const detailOpen = ref(false);
const detailTask = computed(() => nurtureTasks.value.find((task) => task.id === detailId.value) || null);

function openTaskDetail(id: number) {
  detailId.value = id;
  detailOpen.value = true;
}

function actionLabel(action: string) {
  if (action === "like") return "点赞";
  if (action === "repost") return "转发";
  if (action === "follow") return "关注";
  if (action === "open") return "打开";
  return "记录";
}

function eventCount(task: NurtureTaskStatus, action: string) {
  return (task.events || []).filter((event) => event.action === action).length;
}

function formatEventTime(at: number) {
  const date = new Date(at * 1000);
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`;
}

function eventWho(event: NurtureTaskStatus["events"][number]) {
  if (event.handle) return `@${event.handle}`;
  return event.name || "一位用户";
}

function onQueueOutside(event: PointerEvent) {
  if (!queueOpen.value) return;
  const host = queueHost.value;
  if (host && event.target instanceof Node && host.contains(event.target)) return;
  queueOpen.value = false;
}

async function pollNurture() {
  try {
    const status = await ruyiNurtureStatus();
    nurtureTasks.value = status.tasks || [];
    nurtureMessage.value = status.message || "";
  } catch (error) {
    note.value = errorText(error);
  }
}

function openNurture() {
  const loggedIn = accounts.value.filter((row) => row.state === "logged_in" && !nurtureLocked(row.account_id)).map((row) => row.account_id);
  nurtureIds.value = loggedIn.slice(0, 20);
  nurtureOpen.value = true;
  void pollNurture();
}

function toggleNurtureAccount(id: string) {
  if (nurtureIds.value.includes(id)) {
    nurtureIds.value = nurtureIds.value.filter((item) => item !== id);
    return;
  }
  if (nurtureIds.value.length >= 20) {
    note.value = "一次最多选 20 个账号";
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
      feed: nurtureForm.mode === "interact" ? "following" : nurtureForm.mode === "plaza" ? "for_you" : nurtureForm.feed,
      rounds: Number(nurtureForm.rounds) || 8,
      likes: Number(nurtureForm.likes) || 0,
      reposts: Number(nurtureForm.reposts) || 0,
      follows: Number(nurtureForm.follows) || 0,
      delaySecs: Number(nurtureForm.delaySecs) || 5,
      minutes: Number(nurtureForm.minutes) || 10,
    });
    nurtureTasks.value = status.tasks || [];
    nurtureMessage.value = status.message || "";
    note.value = status.message || "养号任务已加入队列";
    nurtureOpen.value = false;
    queueOpen.value = true;
  } catch (error) {
    note.value = errorText(error);
  }
}

async function cancelNurtureTask(taskId: number) {
  try {
    const status = await ruyiNurtureCancel(taskId);
    nurtureTasks.value = status.tasks || [];
    nurtureMessage.value = status.message || "";
    note.value = status.message || "已取消这个养号任务";
  } catch (error) {
    note.value = errorText(error);
  }
}

async function stopNurture() {
  try {
    const status = await ruyiNurtureStop();
    nurtureTasks.value = status.tasks || [];
    nurtureMessage.value = status.message || "";
    note.value = status.message || "已取消全部养号任务";
  } catch (error) {
    note.value = errorText(error);
  }
}

const insightOpen = ref(false);
const insightTab = ref<"likes" | "follows" | "stats">("likes");
const insightAccount = ref<TwitterAccount | null>(null);
const insightLikes = ref<PixivLike[]>([]);
const insightFollows = ref<PixivLike[]>([]);
const insightDays = ref<PixivDay[]>([]);
const insightFollowers = ref(0);
const insightNote = ref("");
const insightError = ref("");
const insightBusy = ref(false);
const insightHasMore = ref(false);
const insightLoadingOlder = ref(false);
const insightStart = ref(dayOffset(-6));
const insightEnd = ref(dayOffset(0));
const chartHost = ref<HTMLDivElement | null>(null);
const likeListEl = ref<HTMLDivElement | null>(null);
const followListEl = ref<HTMLDivElement | null>(null);
let insightRequest = 0;
let insightChart: echarts.ECharts | null = null;

function dayOffset(offset: number) {
  const date = new Date();
  date.setDate(date.getDate() + offset);
  const pad = (value: number) => String(value).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

function formatPixivTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value || "";
  const pad = (item: number) => String(item).padStart(2, "0");
  return `${date.getMonth() + 1}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function insightDay(value: string) {
  const time = Date.parse(value || "");
  if (!Number.isNaN(time)) {
    const date = new Date(time);
    const pad = (item: number) => String(item).padStart(2, "0");
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
  }
  const match = String(value || "").match(/\d{4}-\d{2}-\d{2}/);
  return match ? match[0] : "";
}

const rangedFollows = computed(() => {
  const start = insightStart.value;
  const end = insightEnd.value;
  return insightFollows.value.filter((item) => {
    const day = insightDay(item.at);
    return !!day && day >= start && day <= end;
  });
});

const followTotal = computed(() => rangedFollows.value.reduce((sum, item) => sum + (item.count || 1), 0));

function paintInsightChart() {
  if (insightTab.value !== "stats" || !chartHost.value) return;
  if (!insightChart) insightChart = echarts.init(chartHost.value);
  const followDays = new Map<string, number>();
  for (const item of rangedFollows.value) {
    const day = insightDay(item.at);
    if (!day) continue;
    followDays.set(day, (followDays.get(day) || 0) + (item.count || 1));
  }
  const start = insightStart.value;
  const end = insightEnd.value;
  const likeDays = new Map<string, number>();
  for (const item of insightLikes.value) {
    const day = insightDay(item.at);
    if (!day || day < start || day > end) continue;
    likeDays.set(day, (likeDays.get(day) || 0) + (item.count || 1));
  }
  const days = [...new Set([...insightDays.value.map((day) => day.date), ...followDays.keys(), ...likeDays.keys()])].sort();
  if (!days.length) {
    insightChart.clear();
    return;
  }
  const byDate = new Map(insightDays.value.map((day) => [day.date, day]));
  const series = [
    { name: "浏览", data: days.map((day) => byDate.get(day)?.views ?? null) },
    { name: "点赞", data: days.map((day) => likeDays.get(day) ?? byDate.get(day)?.likes ?? null) },
    { name: "回复", data: days.map((day) => byDate.get(day)?.comments ?? null) },
    { name: "收藏", data: days.map((day) => byDate.get(day)?.bookmarks ?? null) },
    { name: "涨粉", data: days.map((day) => followDays.get(day) ?? null) },
  ];
  insightChart.setOption({
    backgroundColor: "transparent",
    tooltip: { trigger: "axis" },
    legend: { top: 0, textStyle: { color: "#d4d4d8" } },
    grid: { left: 48, right: 16, top: 48, bottom: 28 },
    xAxis: { type: "category", data: days.map((day) => day.slice(5)), axisLabel: { color: "#a1a1aa" } },
    yAxis: { type: "value", axisLabel: { color: "#a1a1aa" }, splitLine: { lineStyle: { color: "#3f3f46" } } },
    series: series.map((item) => ({ name: item.name, type: "bar", data: item.data })),
  }, true);
  insightChart.resize();
}

function pixivTime(value: string) {
  const time = Date.parse(value || "");
  return Number.isNaN(time) ? 0 : time;
}

function sortPixivItems(items: PixivLike[]) {
  return [...items].sort((left, right) => pixivTime(right.at) - pixivTime(left.at));
}

function mergePixivItems(current: PixivLike[], incoming: PixivLike[]) {
  const byId = new Map<string, PixivLike>();
  for (const item of current) byId.set(item.id || `${item.at}|${item.url}`, item);
  for (const item of incoming) {
    const key = item.id || `${item.at}|${item.url}`;
    const previous = byId.get(key);
    if (previous?.cover?.startsWith("data:") && !String(item.cover || "").startsWith("data:")) byId.set(key, { ...item, cover: previous.cover });
    else byId.set(key, item);
  }
  return sortPixivItems([...byId.values()]);
}

function insightListElement() {
  return insightTab.value === "follows" ? followListEl.value : likeListEl.value;
}

function visibleInsightId(box: HTMLDivElement | null) {
  if (!box) return "";
  const articles = box.querySelectorAll("article");
  for (const article of articles) {
    if (article instanceof HTMLElement && article.offsetTop + article.offsetHeight > box.scrollTop + 4) return article.dataset.id || "";
  }
  return "";
}

async function loadInsightLikes(mode: "cache" | "latest" | "more" = "latest") {
  const account = insightAccount.value;
  if (!account) return;
  if (mode === "more" && (!insightHasMore.value || insightBusy.value)) return;
  const accountId = account.account_id;
  const request = ++insightRequest;
  const anchor = mode === "more" ? visibleInsightId(insightListElement()) : "";
  insightBusy.value = true;
  insightLoadingOlder.value = mode === "more";
  try {
    const result = await ruyiPixivLikes(accountId, mode);
    if (request !== insightRequest || insightAccount.value?.account_id !== accountId) return;
    const likes = result.items || [];
    const follows = result.follows || [];
    if (mode === "cache" && !likes.length && !follows.length) {
      insightHasMore.value = !!result.hasMore;
      return;
    }
    if (mode === "cache") {
      insightLikes.value = sortPixivItems(likes);
      insightFollows.value = sortPixivItems(follows);
    } else {
      insightLikes.value = mergePixivItems(insightLikes.value, likes);
      insightFollows.value = mergePixivItems(insightFollows.value, follows);
    }
    insightHasMore.value = !!result.hasMore;
    insightNote.value = result.note || "";
    insightError.value = "";
    await nextTick();
    const box = insightListElement();
    if (!box || request !== insightRequest) return;
    if (mode === "latest") {
      box.scrollTop = 0;
      return;
    }
    if (mode === "more" && anchor) {
      const target = box.querySelector(`[data-id="${CSS.escape(anchor)}"]`);
      if (target instanceof HTMLElement) box.scrollTop = target.offsetTop;
    }
  } catch (error) {
    if (request !== insightRequest || insightAccount.value?.account_id !== accountId) return;
    if (mode !== "cache") insightError.value = errorText(error);
  } finally {
    if (request === insightRequest) {
      insightBusy.value = false;
      insightLoadingOlder.value = false;
    }
  }
}

function onInsightScroll(event: Event) {
  const el = event.target as HTMLElement;
  if (el.scrollHeight - el.scrollTop - el.clientHeight > 140) return;
  void loadInsightLikes("more");
}

async function loadInsightStats() {
  const account = insightAccount.value;
  if (!account) return;
  const accountId = account.account_id;
  const request = ++insightRequest;
  insightBusy.value = true;
  try {
    const result = await ruyiPixivStats(accountId, insightStart.value, insightEnd.value);
    if (request !== insightRequest || insightAccount.value?.account_id !== accountId) return;
    insightDays.value = result.days || [];
    insightFollowers.value = result.followers || 0;
    insightNote.value = result.note || "";
    insightError.value = "";
    await nextTick();
    paintInsightChart();
  } catch (error) {
    if (request !== insightRequest || insightAccount.value?.account_id !== accountId) return;
    insightError.value = errorText(error);
  } finally {
    if (request === insightRequest) insightBusy.value = false;
  }
}

function openInsight(row: TwitterAccount) {
  insightRequest += 1;
  insightAccount.value = row;
  insightTab.value = "likes";
  insightOpen.value = true;
  insightLikes.value = [];
  insightFollows.value = [];
  insightDays.value = [];
  insightFollowers.value = 0;
  insightNote.value = "";
  insightError.value = "";
  insightHasMore.value = false;
  menuId.value = "";
  void (async () => {
    await loadInsightLikes("cache");
    if (insightAccount.value?.account_id === row.account_id && !insightLikes.value.length && !insightFollows.value.length) {
      await loadInsightLikes("latest");
    }
  })();
}

function closeInsight() {
  insightOpen.value = false;
  insightChart?.dispose();
  insightChart = null;
}

async function showInsightTab(tab: "likes" | "follows" | "stats") {
  insightTab.value = tab;
  if (tab === "stats") await loadInsightStats();
}

async function openLikedPost(url: string) {
  const account = insightAccount.value;
  if (!account || !url) return;
  try {
    await ruyiPixivOpenTab(account.account_id, url);
    note.value = "已在这个账号的指纹浏览器里打开帖子";
  } catch (error) {
    insightError.value = errorText(error);
  }
}

watch([insightDays, insightLikes, insightFollows, insightStart, insightEnd], () => {
  void nextTick().then(paintInsightChart);
});

onMounted(async () => {
  document.addEventListener("pointerdown", onQueueOutside);
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
  void pollNurture();
  nurtureTimer = window.setInterval(() => {
    void pollNurture();
  }, 1000);
  liveTimer = window.setInterval(() => {
    const now = Date.now();
    for (const [id, at] of liveAt) {
      if (now - at > 1500) markLive(id, false);
    }
  }, 500);
});

onUnmounted(() => {
  document.removeEventListener("pointerdown", onQueueOutside);
  if (pollTimer) window.clearInterval(pollTimer);
  if (liveTimer) window.clearInterval(liveTimer);
  if (nurtureTimer) window.clearInterval(nurtureTimer);
  insightChart?.dispose();
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
        <div ref="queueHost" class="queue-host">
          <button class="btn queue-trigger" type="button" @click="queueOpen = !queueOpen">
            <i :class="{ on: nurtureActiveCount }" />
            任务队列
            <b v-if="!queueOpen && nurtureActiveCount">{{ nurtureActiveCount > 99 ? "99+" : nurtureActiveCount }}</b>
          </button>
          <div v-if="queueOpen" class="queue-bubble">
            <div class="queue-head">
              <span>任务 ({{ nurtureActiveCount }})</span>
              <button v-if="nurtureActiveCount" class="btn danger tiny" type="button" @click="stopNurture">全部取消</button>
            </div>
            <div class="queue-body">
              <p v-if="!nurtureActiveTasks.length" class="queue-empty">当前没有运行中的任务</p>
              <article v-for="task in nurtureActiveTasks" :key="task.id" class="queue-item">
                <div>
                  <strong>
                    <template v-if="task.status === 'running'">正在</template>
                    {{ task.label || task.accountId }}
                    <em :class="task.status">{{ nurtureStatusText(task.status) }}</em>
                  </strong>
                  <span :class="{ failed: task.status === 'failed' }">{{ task.status === "queued" ? formatRemain(task) : (task.message || formatRemain(task)) }}</span>
                </div>
                <div class="queue-actions">
                  <button class="link-detail" type="button" @click.stop="openTaskDetail(task.id)">详情</button>
                  <button v-if="task.status === 'queued' || task.status === 'running'" class="link-cancel" type="button" @click.stop="cancelNurtureTask(task.id)">取消</button>
                </div>
              </article>
            </div>
          </div>
        </div>
      </div>
      <div class="proxy-choice">
        <span>代理工具</span>
        <button
          v-for="item in proxyChoices"
          :key="item.id"
          type="button"
          :class="{ on: proxyProduct === item.id }"
          :disabled="!item.running || proxySaving"
          :title="item.running ? `使用${item.name}` : `${item.name} 未打开`"
          @click="chooseProxy(item.id)"
        >
          {{ item.name }}{{ item.running ? "" : " · 未打开" }}
        </button>
      </div>
      <p class="hint proxy-hint">{{ proxyHint }}</p>

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
            <button v-if="site === 'pixiv'" class="btn" type="button" @click="openInsight(row)">数据</button>
            <button class="btn primary" type="button" :disabled="!!busyId || (site === 'x' && nurtureLocked(row.account_id))" @click="openBrowser(row)">
              {{ site === "x" && nurtureLocked(row.account_id) ? "养号中" : (row.state === "logged_in" ? "打开" : "登录") }}
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

    <div v-if="detailOpen && detailTask" class="mask" @click.self="detailOpen = false">
      <div class="dialog records">
        <h3>{{ detailTask.label || detailTask.accountId }} 的任务详情</h3>
        <p class="hint">{{ formatRemain(detailTask) }}。{{ detailTask.message }}</p>
        <p class="queue-counts">
          点赞 {{ eventCount(detailTask, "like") }}<template v-if="detailTask.likes"> / {{ detailTask.likes }}</template>
          · 转发 {{ eventCount(detailTask, "repost") }}<template v-if="detailTask.reposts"> / {{ detailTask.reposts }}</template>
          · 关注 {{ eventCount(detailTask, "follow") }}<template v-if="detailTask.follows"> / {{ detailTask.follows }}</template>
          · 共 {{ (detailTask.events || []).length }} 条
        </p>
        <p v-if="!(detailTask.events || []).length" class="empty">还没有记录。点赞、转发或关注之后会出现在这里。</p>
        <ul v-else class="record-list">
          <li v-for="(event, index) in [...(detailTask.events || [])].slice().reverse()" :key="`${event.at}-${event.url}-${index}`">
            <time>{{ formatEventTime(event.at) }}</time>
            <b>{{ actionLabel(event.action) }}</b>
            <div>
              <strong>{{ eventWho(event) }}</strong>
              <span v-if="event.text">{{ event.text }}</span>
            </div>
          </li>
        </ul>
        <div class="dialog-actions">
          <button class="btn" type="button" @click="detailOpen = false">关闭</button>
        </div>
      </div>
    </div>

    <div v-if="insightOpen && insightAccount" class="mask" @click.self="closeInsight">
      <div class="dialog insight">
        <h3>{{ insightAccount.label || insightAccount.username || "Pixiv" }} 的数据</h3>
        <div class="seg">
          <button type="button" :class="{ on: insightTab === 'likes' }" @click="showInsightTab('likes')">点赞记录</button>
          <button type="button" :class="{ on: insightTab === 'follows' }" @click="showInsightTab('follows')">关注记录</button>
          <button type="button" :class="{ on: insightTab === 'stats' }" @click="showInsightTab('stats')">每日数据</button>
        </div>
        <p v-if="insightError" class="hint failed">{{ insightError }}</p>
        <div v-if="insightTab === 'likes'" ref="likeListEl" class="like-list" @scroll="onInsightScroll">
          <p v-if="insightNote" class="hint">{{ insightNote }}</p>
          <p v-if="insightBusy && !insightLikes.length" class="empty">正在读取最新点赞</p>
          <p v-else-if="!insightLikes.length" class="empty">还没有点赞记录。点刷新读取最新通知。</p>
          <article v-for="item in insightLikes" :key="item.id || item.at + item.url" :data-id="item.id">
            <img v-if="item.cover" :src="item.cover" alt="" @error="item.cover = ''" />
            <div v-else class="cover-fallback">图</div>
            <div>
              <strong>{{ item.userName }} <em>{{ item.kind }}</em></strong>
              <span>{{ item.title || item.text || "未命名帖子" }}</span>
              <time>{{ formatPixivTime(item.at) }}</time>
            </div>
            <button v-if="item.url" class="btn" type="button" @click="openLikedPost(item.url)">打开帖子</button>
          </article>
          <button v-if="insightHasMore" class="btn more" type="button" :disabled="insightBusy" @click="loadInsightLikes('more')">
            {{ insightLoadingOlder ? "正在读取更早的记录…" : "更早的记录" }}
          </button>
        </div>
        <div v-else-if="insightTab === 'follows'" ref="followListEl" class="like-list" @scroll="onInsightScroll">
          <div class="range">
            <label>开始<input v-model="insightStart" type="date" /></label>
            <label>结束<input v-model="insightEnd" type="date" /></label>
            <span>这几天新增关注 {{ followTotal }}</span>
          </div>
          <p v-if="insightBusy && !insightFollows.length" class="empty">正在读取最新关注</p>
          <p v-else-if="!rangedFollows.length" class="empty">这个日期里还没有关注记录。点刷新，或把日期范围放宽。</p>
          <article v-for="item in rangedFollows" :key="item.id || item.at + item.userName" :data-id="item.id">
            <img v-if="item.cover" :src="item.cover" alt="" />
            <div v-else class="cover-fallback">人</div>
            <div>
              <strong>{{ item.userName }} <em>{{ item.kind }}</em></strong>
              <span>{{ item.title || item.text }}</span>
              <time>{{ formatPixivTime(item.at) }}</time>
            </div>
            <button v-if="item.url" class="btn" type="button" @click="openLikedPost(item.url)">打开主页</button>
          </article>
          <button v-if="insightHasMore" class="btn more" type="button" :disabled="insightBusy" @click="loadInsightLikes('more')">
            {{ insightLoadingOlder ? "正在读取更早的记录…" : "更早的记录" }}
          </button>
        </div>
        <div v-else class="stats-pane">
          <div class="range">
            <label>开始<input v-model="insightStart" type="date" /></label>
            <label>结束<input v-model="insightEnd" type="date" /></label>
            <button class="btn" type="button" :disabled="insightBusy" @click="loadInsightStats">查询</button>
            <span v-if="insightFollowers">当前粉丝 {{ insightFollowers }}</span>
            <span>这几天新增关注 {{ followTotal }}</span>
          </div>
          <p v-if="insightNote" class="hint">{{ insightNote }}</p>
          <div ref="chartHost" class="chart" />
        </div>
        <div class="dialog-actions">
          <button class="btn" type="button" :disabled="insightBusy" @click="loadInsightLikes('latest')">{{ insightBusy ? "读取中…" : "刷新" }}</button>
          <button class="btn" type="button" @click="closeInsight">关闭</button>
        </div>
      </div>
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
          <button type="button" :class="{ on: nurtureForm.mode === 'plaza' }" @click="nurtureForm.mode = 'plaza'">广场</button>
        </div>
        <div class="checks">
          <label v-for="row in accounts" :key="row.account_id" class="check">
            <input
              type="checkbox"
              :disabled="nurtureLocked(row.account_id)"
              :checked="nurtureIds.includes(row.account_id)"
              @change="toggleNurtureAccount(row.account_id)"
            />
            <span>{{ row.label || row.username || row.account_id }}{{ nurtureLocked(row.account_id) ? "（养号中）" : "" }}</span>
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
          养号时间（分钟）
          <input v-model.number="nurtureForm.minutes" type="number" min="1" max="180" />
        </label>
        <label v-if="nurtureForm.mode === 'interact'">
          每个目标滚动轮数
          <input v-model.number="nurtureForm.rounds" type="number" min="1" max="40" />
        </label>
        <p v-if="nurtureForm.mode === 'plaza'" class="hint">打开 X 首页的「为你推荐」，按下面的次数随机点赞、转发和关注。次数用完后继续浏览，直到养号时间结束。</p>
        <div v-if="nurtureForm.mode === 'interact' || nurtureForm.mode === 'plaza'" class="counts">
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
        <p class="hint">开始后会进入任务队列，按顺序一个一个养。养号中的浏览器不能再打开，完成或取消后才能打开。可以在队列里单独取消，也可以全部取消。</p>
        <div class="dialog-actions">
          <button class="btn" type="button" @click="nurtureOpen = false">关闭</button>
          <button class="btn primary" type="submit">加入队列</button>
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
.toolbar:has(+ .proxy-choice) { border-bottom: 0; }
.orient { margin-left: auto; flex-shrink: 0; }
.proxy-choice { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; padding: 0 16px 8px; }
.proxy-choice span { color: var(--muted); font-size: 13px; }
.proxy-choice button { border: 1px solid var(--bg3); background: transparent; border-radius: 8px; padding: 6px 10px; }
.proxy-choice button.on { background: var(--heading); color: var(--bg0); border-color: var(--heading); }
.proxy-choice button:disabled { opacity: 0.45; }
.proxy-hint { margin: 0; padding: 0 16px 12px; border-bottom: 1px solid var(--bg3); }
.text select { min-width: 0; max-width: 180px; padding: 4px 6px; }
.ip-line { display: flex; align-items: center; gap: 8px; color: var(--heading); font-size: 12px; }
.ip-line button { border: 0; background: transparent; color: var(--heading); padding: 0; text-decoration: underline; }
.tools { flex-wrap: wrap; }
.queue-host { position: relative; z-index: 8; flex-shrink: 0; }
.queue-trigger { position: relative; display: inline-flex; align-items: center; gap: 6px; border-color: #2563eb; color: #60a5fa; }
.queue-trigger:hover { border-color: #3b82f6; color: #93c5fd; background: #172554; }
.queue-trigger i { width: 12px; height: 12px; border-radius: 99px; border: 2px solid currentColor; border-right-color: transparent; opacity: 0.35; }
.queue-trigger i.on { opacity: 1; animation: queue-spin 0.8s linear infinite; }
.queue-trigger b { position: absolute; top: -7px; right: -7px; min-width: 16px; height: 16px; padding: 0 4px; border-radius: 99px; background: #ef4444; color: white; font-size: 11px; line-height: 16px; text-align: center; }
.queue-bubble { position: absolute; top: calc(100% + 10px); right: 0; width: 440px; max-height: min(70vh, 640px); display: flex; flex-direction: column; overflow: hidden; border-radius: 8px; background: #18181b; border: 1px solid #3f3f46; box-shadow: 0 8px 24px rgba(0, 0, 0, 0.45); }
.queue-bubble::before { content: ""; position: absolute; top: -6px; right: 36px; width: 10px; height: 10px; background: #18181b; border-left: 1px solid #3f3f46; border-top: 1px solid #3f3f46; transform: rotate(45deg); }
.queue-head { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 12px 16px; border-bottom: 1px solid #3f3f46; color: #e4e4e7; font-size: 13px; }
.queue-head .tiny { padding: 4px 8px; font-size: 12px; }
.queue-body { max-height: 400px; overflow: auto; padding: 8px 10px 12px; }
.queue-empty { margin: 28px 0; text-align: center; color: #a1a1aa; font-size: 13px; }
.queue-item { display: flex; align-items: center; justify-content: space-between; gap: 12px; padding: 10px 8px; border-bottom: 1px solid #27272a; }
.queue-item:last-child { border-bottom: 0; }
.queue-item div { min-width: 0; display: flex; flex-direction: column; gap: 4px; }
.queue-item strong { display: flex; align-items: center; gap: 6px; min-width: 0; color: #f4f4f5; font-size: 14px; }
.queue-item strong em { flex-shrink: 0; padding: 0 6px; border-radius: 4px; background: #27272a; color: #a1a1aa; font-size: 11px; }
.queue-item strong em.running { background: #172554; color: #93c5fd; }
.queue-item span { color: #a1a1aa; font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.queue-item span.failed { color: #f87171; white-space: normal; }
.queue-actions { display: flex; flex-shrink: 0; align-items: center; gap: 4px; }
.queue-counts { margin: 0; color: var(--muted); font-size: 13px; }
.dialog.records { width: min(640px, 100%); max-height: min(80vh, 720px); }
.record-list { list-style: none; margin: 0; padding: 0; overflow: auto; min-height: 0; }
.record-list li { display: grid; grid-template-columns: 72px 40px minmax(0, 1fr); gap: 8px; align-items: start; padding: 10px 0; border-bottom: 1px solid var(--bg3); }
.record-list time { color: var(--muted); font-size: 12px; line-height: 20px; }
.record-list b { color: var(--heading); font-size: 13px; line-height: 20px; }
.record-list div { min-width: 0; display: flex; flex-direction: column; gap: 2px; }
.record-list strong { color: var(--text); font-size: 13px; }
.record-list span { color: var(--muted); font-size: 12px; white-space: normal; }
.link-detail { border: 0; background: transparent; color: #60a5fa; padding: 4px 6px; }
.link-detail:hover { color: #93c5fd; }
.link-cancel { border: 0; background: transparent; color: #f87171; padding: 4px 6px; }
.link-cancel:hover { color: #fca5a5; }
@keyframes queue-spin { to { transform: rotate(360deg); } }
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
.dialog.insight { width: min(860px, 100%); max-height: min(86vh, 820px); overflow: hidden; }
.hint.failed { color: #f87171; }
.like-list { min-height: 0; max-height: min(62vh, 640px); overflow: auto; display: flex; flex-direction: column; gap: 8px; }
.like-list .btn.more { align-self: center; margin: 8px 0 4px; }
.like-list article { display: grid; grid-template-columns: 72px minmax(0, 1fr) auto; gap: 10px; align-items: center; padding: 8px 0; border-bottom: 1px solid var(--bg3); }
.like-list img, .cover-fallback { width: 72px; height: 72px; object-fit: cover; border-radius: 8px; background: var(--bg2); }
.cover-fallback { display: grid; place-items: center; color: var(--muted); }
.like-list strong { display: flex; gap: 8px; align-items: center; }
.like-list em { color: var(--heading); font-style: normal; font-size: 12px; }
.like-list span, .like-list time { display: block; color: var(--muted); font-size: 12px; }
.stats-pane { min-height: 0; display: flex; flex-direction: column; gap: 8px; }
.range { display: flex; flex-wrap: wrap; align-items: end; gap: 10px; }
.range label { display: flex; flex-direction: column; gap: 4px; color: var(--muted); font-size: 12px; }
.range input { color: var(--text); background: var(--bg0); border: 1px solid var(--bg3); border-radius: 8px; padding: 6px 8px; }
.chart { height: 320px; }
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
