# web-novelai-ui

中文 NovelAI 图像创作工作台的 **Tauri v2 + Vue 3 + Vite + Rust** 桌面端。

- 架构对齐 `qzl-aicjxt`：根 workspace + `apps/frontend` + `src-tauri`
- 功能对齐 `novelai-image-desktop-main`：文生图 / 图生图、账号积分、历史、原数据、设置

Token 只存在 Rust 侧，前端通过 `invoke` 调用，不会直接把 Bearer 发给网页。

## 目录

```text
web-novelai-ui/
├─ apps/frontend/     # Vue 3 + Vite
├─ src-tauri/         # Tauri v2 Rust 宿主
└─ package.json
```

## 环境

与 [Tauri v2 Windows 前置要求](https://v2.tauri.app/zh-cn/start/prerequisites/) 一致：

1. Visual Studio C++ 桌面开发（MSVC）
2. WebView2
3. `rustup default stable-msvc`
4. Node.js ≥ 18

## 开发

```bash
cd d:\project\web-novelai-ui
npm install
npm run dev
```

仅浏览器调试前端：

```bash
npm run dev:frontend
```

## 打包

```bash
npm run build
```

安装包为 NSIS（见 `src-tauri/tauri.conf.json`）。

## 已实现 / 待迁

| 模块 | 状态 |
| --- | --- |
| 文生图、图生图、模型/采样器/尺寸/Seed | 已实现 |
| Token 验证、Anlas、历史保存 | 已实现 |
| PNG 原数据读取并套用参数 | 已实现 |
| 设置（输出目录、代理、自定义端点） | 已实现 |
| 局部重绘、后期、酒馆、漫画、画廊、参考预设 | 页面骨架，待按原 Electron 逻辑迁入 |
