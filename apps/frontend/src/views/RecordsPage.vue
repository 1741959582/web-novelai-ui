<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import { useAppStore } from "@/stores/app";
import { openOutputDir } from "@/api/tauri";

const store = useAppStore();
const router = useRouter();
const picked = ref<string[]>([]);

function toggle(id: string) {
  picked.value = picked.value.includes(id)
    ? picked.value.filter((item) => item !== id)
    : [...picked.value, id];
}

function sendEnhance() {
  const ids = store.history.filter((item) => picked.value.includes(item.id)).map((item) => item.id);
  if (!ids.length) {
    store.status = "先勾选要增强的记录。";
    return;
  }
  sessionStorage.setItem("nai-dlssnr-ids", JSON.stringify(ids.slice(0, 24)));
  void router.push("/enhance");
}
</script>

<template>
  <div class="wrap">
    <div class="head">
      <h2>记录</h2>
      <div class="row">
        <button class="btn" type="button" :disabled="!picked.length" @click="sendEnhance">DLSSNR 增强</button>
        <button class="btn" type="button" @click="openOutputDir()">打开输出目录</button>
      </div>
    </div>
    <div v-if="!store.history.length" class="hint">还没有生成记录。</div>
    <div class="grid">
      <article v-for="item in store.history" :key="item.id" class="card item" :class="{ on: picked.includes(item.id) }">
        <label class="pick">
          <input type="checkbox" :checked="picked.includes(item.id)" @change="toggle(item.id)" />
          <span class="title">{{ item.kind }} · {{ item.width }}×{{ item.height }} · seed {{ item.seed }}</span>
        </label>
        <p class="prompt">{{ item.prompt }}</p>
        <p class="hint">{{ item.path }}</p>
        <div class="row">
          <button class="btn" type="button" @click="store.applyHistory(item)">套用参数</button>
          <button class="btn" type="button" @click="store.showHistory(item)">预览</button>
          <button class="btn danger" type="button" @click="store.removeHistory(item.id)">删除</button>
        </div>
      </article>
    </div>
  </div>
</template>

<style scoped>
.wrap { padding: 20px; overflow: auto; }
.head { display: flex; justify-content: space-between; align-items: center; gap: 12px; }
.grid { display: grid; gap: 12px; }
.item .prompt { white-space: pre-wrap; font-size: 13px; }
.item.on { outline: 1px solid var(--heading); }
.pick { display: flex; align-items: center; gap: 8px; margin-bottom: 6px; }
.title { font-weight: 600; }
</style>
