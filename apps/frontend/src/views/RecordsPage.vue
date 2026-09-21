<script setup lang="ts">
import { useAppStore } from "@/stores/app";
import { openOutputDir } from "@/api/tauri";

const store = useAppStore();
</script>

<template>
  <div class="wrap">
    <div class="head">
      <h2>记录</h2>
      <button class="btn" type="button" @click="openOutputDir()">打开输出目录</button>
    </div>
    <div v-if="!store.history.length" class="hint">还没有生成记录。</div>
    <div class="grid">
      <article v-for="item in store.history" :key="item.id" class="card item">
        <div class="title">{{ item.kind }} · {{ item.width }}×{{ item.height }} · seed {{ item.seed }}</div>
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
.wrap { padding: 20px; }
.head { display: flex; justify-content: space-between; align-items: center; }
.grid { display: grid; gap: 12px; }
.item .prompt { white-space: pre-wrap; font-size: 13px; }
.title { font-weight: 600; margin-bottom: 6px; }
</style>
