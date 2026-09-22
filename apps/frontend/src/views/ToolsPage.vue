<script setup lang="ts">
import { useRouter } from "vue-router";

const router = useRouter();
const tools = [
  { title: "批量生成", desc: "批量录入主词、角色词和图片配置，排队一张接一张生成。", path: "/batch" },
  { title: "漫画生成器", desc: "逐行 Tag / JSON / CSV 分镜，候选图与主图 ZIP。" },
  { title: "批量重绘", desc: "对一组已有图片做 img2img 队列。" },
  { title: "画风实验室", desc: "随机画师串、目标画风迭代与收藏。" },
  { title: "灵感胶囊 / Tag 补全", desc: "本地词库与在线补全。" },
  { title: "反推", desc: "WD Tagger 走 Hugging Face 空间；若设置里启用了本地 GPU，CL Tagger v2 会在本机跑。", path: "/reverse" },
  { title: "APNG 伪装 / GIF", desc: "封面藏缩略图，真图藏进点开后的动画帧；也能合成 GIF、还原、清元数据和打码。", path: "/apng" },
  { title: "个人法典", desc: "离线查找章节与分类提示词。" },
];
</script>

<template>
  <div class="wrap">
    <h2>工具</h2>
    <p class="hint">这些工具在原 Electron 项目里已经实现。当前桌面端先提供入口，后续把对应 IPC 迁到 Rust。</p>
    <div class="grid">
      <article
        v-for="t in tools"
        :key="t.title"
        class="card"
        :class="{ go: t.path }"
        @click="t.path && router.push(t.path)"
      >
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
.card.go { cursor: pointer; }
.card.go:hover { border-color: var(--heading); }
</style>
