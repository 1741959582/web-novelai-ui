import { createRouter, createWebHistory } from "vue-router";

export default createRouter({
  history: createWebHistory(),
  routes: [
    { path: "/", name: "generate", component: () => import("@/views/GeneratePage.vue") },
    { path: "/batch", name: "batch", component: () => import("@/views/BatchPage.vue"), meta: { title: "批量生成" } },
    { path: "/inpaint", name: "inpaint", component: () => import("@/views/GeneratePage.vue") },
    { path: "/postprocess", name: "postprocess", component: () => import("@/views/PlaceholderPage.vue"), meta: { title: "后期", hint: "云端超分、Director Tools（去背景、线稿、上色等）将按原项目接口迁入。" } },
    { path: "/metadata", name: "metadata", component: () => import("@/views/MetadataPage.vue") },
    { path: "/tools", name: "tools", component: () => import("@/views/ToolsPage.vue") },
    { path: "/apng", name: "apng", component: () => import("@/views/ApngPage.vue"), meta: { title: "APNG" } },
    { path: "/reverse", name: "reverse", component: () => import("@/views/ReversePage.vue") },
    { path: "/reference", name: "reference", component: () => import("@/views/ReferencePage.vue"), meta: { title: "参考预设" } },
    { path: "/gallery", name: "gallery", component: () => import("@/views/GalleryPage.vue"), meta: { title: "法典" } },
    { path: "/tavern", name: "tavern", component: () => import("@/views/TavernPage.vue"), meta: { title: "酒馆 AI 生图" } },
    { path: "/records", name: "records", component: () => import("@/views/RecordsPage.vue") },
    { path: "/settings", name: "settings", component: () => import("@/views/SettingsPage.vue") },
  ],
});
