<script setup lang="ts">
import { RouterLink, useRoute, useRouter } from "vue-router";
import { ref } from "vue";
import { TABS } from "@/types/nai";
import { useAppStore } from "@/stores/app";
import NaiIcon from "@/components/NaiIcon.vue";

const route = useRoute();
const router = useRouter();
const store = useAppStore();
const menuOpen = ref(false);

function go(path: string) {
  menuOpen.value = false;
  void router.push(path);
}
</script>

<template>
  <div class="shell">
    <header class="top">
      <button class="logo" type="button" title="Generate" @click="go('/')">
        <NaiIcon name="logo" :size="22" />
      </button>
      <button class="anlas" type="button" title="Refresh Anlas" @click="store.refreshAccount()">
        Anlas:
        <NaiIcon name="anlas" :size="14" />
        {{ store.account.anlasBalance ?? "—" }}
      </button>
      <button class="icon-sq" type="button" title="Settings" @click="go('/settings')">
        <NaiIcon name="plus" :size="16" />
      </button>
      <button class="icon-sq" type="button" title="Menu" @click="menuOpen = !menuOpen">
        <NaiIcon name="menu" :size="18" />
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

    <div v-if="menuOpen" class="menu-back" @click="menuOpen = false">
      <nav class="menu" @click.stop>
        <RouterLink
          v-for="tab in TABS"
          :key="tab.id"
          :to="tab.path"
          class="menu-item"
          :class="{ on: route.path === tab.path }"
          @click="menuOpen = false"
        >
          {{ tab.label }}
        </RouterLink>
      </nav>
    </div>

    <main class="main">
      <slot />
    </main>
  </div>
</template>

<style scoped>
.shell { height: 100%; display: flex; flex-direction: column; background: var(--bg0); }
.top {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 10px 14px 8px 16px;
  background: var(--bg0);
}
.logo, .icon-sq {
  width: 34px;
  height: 34px;
  display: grid;
  place-items: center;
  background: transparent;
  border: 1px solid var(--bg3);
  border-radius: 6px;
  color: #fff;
}
.logo { border-color: transparent; color: var(--heading); }
.anlas {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  background: transparent;
  border: 0;
  color: #fff;
  font-size: 14px;
  padding: 0 4px;
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
.menu-back {
  position: fixed;
  inset: 0;
  z-index: 30;
  background: rgba(14, 15, 33, 0.45);
}
.menu {
  position: absolute;
  top: 52px;
  left: 86px;
  min-width: 200px;
  background: var(--bg2);
  border: 1px solid var(--bg3);
  border-radius: 8px;
  padding: 8px;
  display: grid;
  gap: 2px;
}
.menu-item {
  padding: 8px 10px;
  border-radius: 4px;
  color: #fff;
}
.menu-item.on, .menu-item:hover { background: var(--bg3); color: var(--heading); }
</style>
