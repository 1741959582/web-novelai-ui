import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { fileURLToPath, URL } from "node:url";

const host = "127.0.0.1";

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  clearScreen: false,
  server: {
    host,
    port: 1420,
    strictPort: true,
    hmr: {
      protocol: "ws",
      host,
      port: 1421,
      clientPort: 1421,
    },
    watch: {
      ignored: ["**/src-tauri/**"],
    },
    proxy: {
      "/aitag": {
        target: "https://aitag.win",
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/aitag/, ""),
        headers: { Referer: "https://aitag.win/" },
      },
      "/pixiv": {
        target: "https://i.pximg.net",
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/pixiv/, ""),
        headers: { Referer: "https://www.pixiv.net/" },
      },
      "/aiimg": {
        target: "https://ai-img.10118899.xyz",
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/aiimg/, ""),
        headers: { Referer: "https://aitag.win/" },
      },
      "/danbooru": {
        target: "https://danbooru.donmai.us",
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/danbooru/, ""),
        headers: {
          Accept: "application/json",
          "User-Agent": "Mozilla/5.0",
        },
      },
      "/safebooru": {
        target: "https://safebooru.org",
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/safebooru/, ""),
        headers: {
          "User-Agent": "Mozilla/5.0",
          Referer: "https://safebooru.org/",
        },
      },
      "/gelbooru": {
        target: "https://gelbooru.com",
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/gelbooru/, ""),
        headers: {
          "User-Agent": "Mozilla/5.0",
          Referer: "https://gelbooru.com/",
        },
      },
      "/gelimg": {
        target: "https://img4.gelbooru.com",
        changeOrigin: true,
        router: (req) => {
          const host = req.url?.match(/^\/gelimg\/(img\d+)\.gelbooru\.com/)?.[1];
          return host ? `https://${host}.gelbooru.com` : "https://img4.gelbooru.com";
        },
        rewrite: (path) => path.replace(/^\/gelimg\/img\d+\.gelbooru\.com/, ""),
        headers: { Referer: "https://gelbooru.com/" },
      },
    },
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: ["es2021", "chrome105", "safari14"],
    minify: !process.env.TAURI_ENV_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
});
