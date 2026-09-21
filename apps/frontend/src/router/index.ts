import { createRouter, createWebHistory } from "vue-router";

export default createRouter({
  history: createWebHistory(),
  routes: [
    { path: "/", name: "generate", component: () => import("@/views/GeneratePage.vue") },
    { path: "/inpaint", name: "inpaint", component: () => import("@/views/PlaceholderPage.vue"), meta: { title: "局部重绘", hint: "在蒙版画布标记区域，通过 NovelAI infill 修改局部。正在从 Electron 版迁入。" } },
    { path: "/postprocess", name: "postprocess", component: () => import("@/views/PlaceholderPage.vue"), meta: { title: "后期", hint: "云端超分、Director Tools（去背景、线稿、上色等）将按原项目接口迁入。" } },
    { path: "/metadata", name: "metadata", component: () => import("@/views/MetadataPage.vue") },
    { path: "/tools", name: "tools", component: () => import("@/views/ToolsPage.vue") },
    { path: "/reference", name: "reference", component: () => import("@/views/PlaceholderPage.vue"), meta: { title: "参考预设", hint: "精准参考、氛围迁移与在线角色目录将迁入此页。" } },
    { path: "/gallery", name: "gallery", component: () => import("@/views/PlaceholderPage.vue"), meta: { title: "在线画廊", hint: "作品搜索、排行与参数套用将迁入此页。" } },
    { path: "/tavern", name: "tavern", component: () => import("@/views/PlaceholderPage.vue"), meta: { title: "酒馆 AI 生图", hint: "对话构思后再确认生图 / 全自动生图，角色卡与世界书将迁入。" } },
    { path: "/records", name: "records", component: () => import("@/views/RecordsPage.vue") },
    { path: "/settings", name: "settings", component: () => import("@/views/SettingsPage.vue") },
  ],
});
