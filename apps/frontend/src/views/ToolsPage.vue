<script setup lang="ts">
const tools = [
  { title: "漫画生成器", desc: "逐行 Tag / JSON / CSV 分镜，候选图与主图 ZIP。" },
  { title: "批量重绘", desc: "对一组已有图片做 img2img 队列。" },
  { title: "画风实验室", desc: "随机画师串、目标画风迭代与收藏。" },
  { title: "灵感胶囊 / Tag 补全", desc: "本地词库与在线补全。" },
  { title: "反推 / 转换", desc: "视觉模型反推、文本模型整理提示词。" },
  { title: "个人法典", desc: "离线查找章节与分类提示词。" },
];
</script>

<template>
  <div class="wrap">
    <h2>工具</h2>
    <p class="hint">这些工具在原 Electron 项目里已经实现。当前桌面端先提供入口，后续把对应 IPC 迁到 Rust。</p>
    <div class="grid">
      <article v-for="t in tools" :key="t.title" class="card">
        <h3>{{ t.title }}</h3>
        <p class="hint">{{ t.desc }}</p>
      </article>
    </div>
  </div>
</template>

<style scoped>
.wrap { padding: 20px; }
.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(240px, 1fr)); gap: 12px; }
h3 { margin: 0 0 8px; }
</style>
