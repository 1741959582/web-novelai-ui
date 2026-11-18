<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useAppStore } from "@/stores/app";
import NaiIcon from "@/components/NaiIcon.vue";

interface NavItem {
  path: string;
  label: string;
  icon: string;
  key?: string;
}

const groups: { label: string; items: NavItem[] }[] = [
  {
    label: "创作",
    items: [
      { path: "/", label: "生成", icon: "pen", key: "1" },
      { path: "/batch", label: "批量", icon: "layers", key: "2" },
      { path: "/video", label: "视频", icon: "play", key: "3" },
      { path: "/tavern", label: "酒馆", icon: "sparkle", key: "4" },
    ],
  },
  {
    label: "图库",
    items: [
      { path: "/online", label: "在线画廊", icon: "landscape", key: "5" },
      { path: "/gallery", label: "法典", icon: "image", key: "6" },
      { path: "/records", label: "记录", icon: "history" },
    ],
  },
  {
    label: "素材",
    items: [
      { path: "/prompt-blocks", label: "块库", icon: "grid", key: "7" },
      { path: "/vibe-library", label: "Vibe 库", icon: "vibe", key: "8" },
      { path: "/precise-library", label: "精准参考库", icon: "precise", key: "9" },
      { path: "/reference", label: "参考预设", icon: "layers" },
      { path: "/tagmarket", label: "标签超市", icon: "file" },
    ],
  },
  {
    label: "工具",
    items: [
      { path: "/reverse", label: "反推", icon: "bulb" },
      { path: "/metadata", label: "原数据", icon: "file" },
      { path: "/postprocess", label: "后期", icon: "upscale" },
      { path: "/apng", label: "APNG", icon: "film" },
      { path: "/fingerprint", label: "指纹", icon: "eye" },
      { path: "/deepseek", label: "DeepSeek", icon: "diamond" },
      { path: "/tools", label: "工具", icon: "sliders" },
    ],
  },
];

const settingsItem: NavItem = { path: "/settings", label: "设置", icon: "gear", key: "," };
const pages = [...groups.flatMap((group) => group.items), settingsItem];

const route = useRoute();
const router = useRouter();
const store = useAppStore();

const open = ref(false);
const pinned = ref(false);
const accountOpen = ref(false);
const ctrlHeld = ref(false);

const accountName = computed(() => (store.hasToken ? store.account.tierName || "已登录" : "未登录"));
const accountSub = computed(() => {
  const balance = store.account.anlasBalance;
  if (balance == null) return "";
  return `Anlas ${balance.toLocaleString("en-US")}`;
});

function active(path: string) {
  if (path === "/") return route.path === "/" || route.path === "/inpaint";
  return route.path === path;
}

function go(path: string) {
  accountOpen.value = false;
  if (!pinned.value) open.value = false;
  void router.push(path);
}

function togglePin() {
  pinned.value = !pinned.value;
  open.value = !pinned.value;
}

function collapse() {
  accountOpen.value = false;
  if (pinned.value) pinned.value = false;
  else open.value = false;
}

function onKey(event: KeyboardEvent) {
  if (event.key === "Control") ctrlHeld.value = event.type === "keydown";
  if (event.type !== "keydown") return;
  if (event.key === "Escape" && open.value && !pinned.value) {
    open.value = false;
    accountOpen.value = false;
    return;
  }
  if (!event.ctrlKey || event.altKey || event.metaKey || event.shiftKey) return;
  const target = event.target as HTMLElement | null;
  if (target?.closest("input, textarea, select, [contenteditable='true']")) return;
  const item = pages.find((page) => page.key === event.key);
  if (!item) return;
  event.preventDefault();
  go(item.path);
}

function releaseCtrl() {
  ctrlHeld.value = false;
}

onMounted(() => {
  pinned.value = localStorage.getItem("nav-rail-pinned") === "1";
  window.addEventListener("keydown", onKey);
  window.addEventListener("keyup", onKey);
  window.addEventListener("blur", releaseCtrl);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", onKey);
  window.removeEventListener("keyup", onKey);
  window.removeEventListener("blur", releaseCtrl);
});

watch(pinned, (value) => localStorage.setItem("nav-rail-pinned", value ? "1" : "0"));
</script>

<template>
  <div class="shell">
    <nav v-if="!pinned" class="rail" aria-label="页面">
      <button class="avatar" type="button" :title="accountName" @click="open = true; accountOpen = true">
        <NaiIcon name="user" :size="18" />
      </button>
      <div class="rail-scroll">
        <template v-for="(group, index) in groups" :key="group.label">
          <span v-if="index > 0" class="rule" />
          <button
            v-for="item in group.items"
            :key="item.path"
            type="button"
            class="plate"
            :class="{ on: active(item.path) }"
            :title="item.key ? `${item.label}  Ctrl+${item.key}` : item.label"
            @click="go(item.path)"
          >
            <span v-if="active(item.path)" class="bar" />
            <span class="face"><NaiIcon :name="item.icon" :size="18" /></span>
            <em v-if="ctrlHeld && item.key">{{ item.key }}</em>
          </button>
        </template>
      </div>
      <button class="plate muted" type="button" title="展开" @click="open = true">
        <span class="face"><NaiIcon name="chevrons-right" :size="17" /></span>
      </button>
      <button
        class="plate muted"
        type="button"
        :class="{ on: active(settingsItem.path) }"
        title="设置  Ctrl+,"
        @click="go(settingsItem.path)"
      >
        <span v-if="active(settingsItem.path)" class="bar" />
        <span class="face"><NaiIcon name="gear" :size="18" /></span>
        <em v-if="ctrlHeld">,</em>
      </button>
    </nav>

    <Transition name="nav">
      <aside v-if="pinned || open" class="panel" :class="{ overlay: !pinned }" aria-label="页面">
        <button class="who" type="button" @click="accountOpen = !accountOpen">
          <span class="avatar sm"><NaiIcon name="user" :size="16" /></span>
          <span class="who-copy">
            <b>{{ accountName }}</b>
            <small v-if="accountSub">{{ accountSub }}</small>
          </span>
          <NaiIcon name="down" :size="14" />
        </button>
        <div v-if="accountOpen" class="account-pop" @click.stop>
          <p>{{ accountName }}</p>
          <p v-if="accountSub" class="sub">{{ accountSub }}</p>
          <button type="button" @click="store.refreshAccount(); accountOpen = false">刷新积分</button>
          <button type="button" @click="go('/settings')">打开设置</button>
        </div>
        <div class="hairline" />
        <div class="panel-scroll">
          <section v-for="group in groups" :key="group.label">
            <h3>{{ group.label }}</h3>
            <button
              v-for="item in group.items"
              :key="item.path"
              type="button"
              class="row"
              :class="{ on: active(item.path) }"
              @click="go(item.path)"
            >
              <NaiIcon :name="item.icon" :size="18" />
              <span>{{ item.label }}</span>
              <kbd v-if="item.key"><i>Ctrl</i><i>{{ item.key }}</i></kbd>
            </button>
          </section>
        </div>
        <button type="button" class="row quiet" @click="togglePin">
          <NaiIcon name="thumbtack" :size="16" />
          <span>钉住展开</span>
          <span class="switch" :class="{ on: pinned }" aria-hidden="true"><i /></span>
        </button>
        <button type="button" class="row quiet" @click="collapse">
          <NaiIcon name="chevrons-left" :size="16" />
          <span>收起</span>
          <kbd v-if="!pinned"><i>Esc</i></kbd>
        </button>
        <button type="button" class="row" :class="{ on: active(settingsItem.path) }" @click="go(settingsItem.path)">
          <NaiIcon name="gear" :size="17" />
          <span>设置</span>
          <kbd><i>Ctrl</i><i>,</i></kbd>
        </button>
      </aside>
    </Transition>
    <div v-if="!pinned && open" class="scrim" @click="open = false; accountOpen = false" />

    <div class="work">
      <header class="top">
        <button class="logo" type="button" title="生成" @click="go('/')">
          <NaiIcon name="logo" :size="22" />
        </button>
        <button class="anlas" type="button" title="刷新 Anlas" @click="store.refreshAccount()">
          Anlas:
          <NaiIcon name="anlas" :size="14" />
          {{ store.account.anlasBalance ?? "—" }}
        </button>
        <span class="grow" />
        <span class="status">{{ store.status }}</span>
      </header>

      <div
        v-if="store.updateInfo?.available && store.updateDismissed !== store.updateInfo.latest"
        class="update-bar"
      >
        <span>发现新版本 {{ store.updateInfo.latest }}（当前 {{ store.updateInfo.current }}）</span>
        <span v-if="store.updateBusy" class="upd-msg">{{ store.updateMsg || `下载中 ${store.updatePct}%` }}</span>
        <button type="button" class="upd-go" :disabled="store.updateBusy" @click="store.applyAppUpdate()">
          {{ store.updateBusy ? "更新中…" : "立即更新" }}
        </button>
        <button type="button" @click="store.openUpdatePage()">打开发行页</button>
        <button type="button" :disabled="store.updateBusy" @click="store.dismissUpdate()">稍后</button>
      </div>

      <main class="main">
        <slot />
      </main>
    </div>
  </div>
</template>

<style scoped>
.shell {
  height: 100%;
  display: flex;
  background: var(--bg0);
  position: relative;
  overflow: hidden;
}
.rail {
  width: 48px;
  flex: none;
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 6px 0 8px;
  background: #12141f;
  border-right: 1px solid rgba(255, 255, 255, 0.08);
  z-index: 2;
}
.rail-scroll {
  flex: 1;
  min-height: 0;
  width: 100%;
  overflow: auto;
  display: flex;
  flex-direction: column;
  align-items: center;
}
.rail-scroll::-webkit-scrollbar { width: 0; }
.avatar {
  width: 36px;
  height: 36px;
  margin-bottom: 6px;
  border: 0;
  border-radius: 50%;
  display: grid;
  place-items: center;
  background: #2a3148;
  color: rgba(255, 255, 255, 0.82);
}
.avatar.sm { width: 32px; height: 32px; margin: 0; flex: none; }
.plate {
  position: relative;
  width: 47px;
  height: 40px;
  border: 0;
  background: transparent;
  color: rgba(255, 255, 255, 0.72);
  display: grid;
  place-items: center;
  font-weight: 500;
}
.plate.muted { color: rgba(255, 255, 255, 0.48); }
.face {
  width: 36px;
  height: 36px;
  display: grid;
  place-items: center;
  border-radius: 9px;
}
.plate.on .face { background: rgba(96, 165, 250, 0.16); color: #8eb7ff; }
.plate:hover:not(.on) .face { background: rgba(255, 255, 255, 0.06); color: #fff; }
.plate em {
  position: absolute;
  right: 4px;
  bottom: 2px;
  min-width: 12px;
  height: 12px;
  padding: 0 2px;
  border-radius: 4px;
  background: #60a5fa;
  color: #0e0f21;
  font-style: normal;
  font-size: 9px;
  font-weight: 700;
  line-height: 12px;
}
.bar {
  position: absolute;
  left: 0;
  top: 12px;
  width: 3px;
  height: 16px;
  border-radius: 2px;
  background: #60a5fa;
}
.rule {
  width: 16px;
  height: 1.5px;
  margin: 8px 0;
  border-radius: 1px;
  background: rgba(255, 255, 255, 0.28);
}
.panel {
  width: 248px;
  flex: none;
  display: flex;
  flex-direction: column;
  padding: 10px 8px 8px;
  background: #12141f;
  border-right: 1px solid rgba(255, 255, 255, 0.08);
  position: relative;
  z-index: 3;
}
.panel.overlay {
  position: absolute;
  left: 0;
  top: 0;
  bottom: 0;
  z-index: 41;
  box-shadow: 10px 0 30px rgba(0, 0, 0, 0.5);
}
.nav-enter-active, .nav-leave-active { transition: transform 0.18s ease; }
.nav-enter-from, .nav-leave-to { transform: translateX(-100%); }
.scrim {
  position: absolute;
  left: 48px;
  top: 0;
  right: 0;
  bottom: 0;
  z-index: 40;
  background: rgba(0, 0, 0, 0.35);
}
.who {
  height: 40px;
  width: 100%;
  border: 0;
  background: transparent;
  border-radius: 9px;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 6px 0 4px;
  text-align: left;
  font-weight: 500;
  color: rgba(255, 255, 255, 0.45);
}
.who:hover { background: rgba(255, 255, 255, 0.05); }
.who-copy { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.who-copy b {
  color: #fff;
  font-size: 13px;
  font-weight: 700;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.who-copy small { color: var(--muted); font-size: 11px; }
.account-pop {
  position: absolute;
  top: 54px;
  left: 12px;
  right: 12px;
  z-index: 5;
  padding: 8px;
  border-radius: 10px;
  background: #1b2033;
  border: 1px solid rgba(255, 255, 255, 0.08);
  box-shadow: 0 12px 28px rgba(0, 0, 0, 0.35);
}
.account-pop p { margin: 0 4px 6px; font-size: 13px; font-weight: 700; }
.account-pop .sub { color: var(--muted); font-size: 12px; font-weight: 500; }
.account-pop button {
  width: 100%;
  border: 0;
  background: transparent;
  color: #fff;
  text-align: left;
  border-radius: 7px;
  padding: 7px 8px;
  font-weight: 500;
}
.account-pop button:hover { background: rgba(255, 255, 255, 0.06); }
.hairline { height: 1px; margin: 6px 6px 2px; background: rgba(255, 255, 255, 0.08); }
.panel-scroll { flex: 1; min-height: 0; overflow: auto; }
.panel-scroll::-webkit-scrollbar { width: 0; }
.panel h3 {
  margin: 0;
  padding: 10px 10px 4px;
  color: rgba(255, 255, 255, 0.38);
  font-family: inherit;
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.3px;
}
.row {
  position: relative;
  width: 100%;
  height: 36px;
  margin-top: 1px;
  border: 0;
  background: transparent;
  border-radius: 8px;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0 8px 0 10px;
  color: rgba(255, 255, 255, 0.78);
  font-size: 13px;
  font-weight: 400;
  text-align: left;
}
.row :deep(.nai-icon) { flex: none; color: rgba(255, 255, 255, 0.7); }
.row > span { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.row > .switch { flex: none; }
.row:hover { background: rgba(255, 255, 255, 0.05); }
.row.on { background: rgba(96, 165, 250, 0.14); color: #8eb7ff; font-weight: 600; }
.row.on :deep(.nai-icon) { color: #8eb7ff; }
.row.on::before {
  content: "";
  position: absolute;
  left: 0;
  top: 10px;
  width: 3px;
  height: 16px;
  border-radius: 2px;
  background: #60a5fa;
}
.row.quiet { color: rgba(255, 255, 255, 0.5); }
.row.quiet :deep(.nai-icon) { color: rgba(255, 255, 255, 0.5); }
kbd { display: inline-flex; gap: 3px; flex: none; }
kbd i {
  font-style: normal;
  font-weight: 500;
  font-size: 11px;
  line-height: 16px;
  color: rgba(255, 255, 255, 0.55);
  border: 1px solid rgba(255, 255, 255, 0.14);
  border-radius: 4px;
  padding: 0 4px;
  background: rgba(255, 255, 255, 0.03);
}
.switch {
  width: 32px;
  height: 18px;
  flex: none;
  border-radius: 99px;
  background: #34384f;
  position: relative;
}
.switch i {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: #fff;
  transition: left 0.15s ease;
}
.switch.on { background: #3b82f6; }
.switch.on i { left: 16px; }
.work { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.top {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 14px 8px 16px;
  background: var(--bg0);
}
.logo {
  width: 34px;
  height: 34px;
  display: grid;
  place-items: center;
  background: transparent;
  border: 0;
  color: var(--heading);
}
.anlas {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  background: transparent;
  border: 0;
  color: #fff;
  font-size: 14px;
  padding: 0 4px;
  font-weight: 600;
}
.anlas :deep(.nai-icon) { color: var(--heading); }
.grow { flex: 1; }
.status { color: var(--muted); font-size: 12px; max-width: 42%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.update-bar {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 16px;
  background: #2a2d4c;
  color: #fff;
  font-size: 13px;
}
.update-bar .upd-msg { color: var(--heading); }
.update-bar button {
  border: 0;
  border-radius: 6px;
  background: #3d4270;
  color: #fff;
  padding: 6px 10px;
}
.update-bar .upd-go { background: #f5f3c2; color: #0e0f21; font-weight: 700; }
.main { flex: 1; min-height: 0; overflow: hidden; position: relative; }
.main > * { height: 100%; min-height: 0; }
</style>
