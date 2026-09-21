<script setup lang="ts">
import { useRoute } from "vue-router";
const route = useRoute();
</script>

<template>
  <div class="wrap">
    <div class="card">
      <h2>{{ route.meta.title || "功能" }}</h2>
      <p class="hint">{{ route.meta.hint }}</p>
      <p class="hint">生成、原数据、记录和设置已经接到 Tauri。其余模块会按 `novelai-image-desktop-main` 的 Electron IPC 逐项迁到 `src-tauri`。</p>
    </div>
  </div>
</template>

<style scoped>
.wrap { padding: 32px; max-width: 720px; }
</style>
