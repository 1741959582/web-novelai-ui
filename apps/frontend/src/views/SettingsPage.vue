<script setup lang="ts">
import { ref } from "vue";
import { openOutputDir, pickOutputDir } from "@/api/tauri";
import { useAppStore } from "@/stores/app";

const store = useAppStore();
const tokenDraft = ref("");
const message = ref("");

async function verify() {
  const res = await store.verifyToken(tokenDraft.value);
  message.value = res.message;
  tokenDraft.value = "";
}

async function chooseDir() {
  const dir = await pickOutputDir();
  if (dir) {
    store.settings.outputDir = dir;
    await store.saveSettings();
  }
}

async function save() {
  await store.saveSettings();
  message.value = "设置已保存";
}
</script>

<template>
  <div class="wrap">
    <div class="card">
      <h2>API 配置</h2>
      <p class="hint">填写 NovelAI Persistent API Token。Token 只保存在本机 Rust 侧，不会出现在页面请求里。</p>
      <div class="field">
        <label>Token</label>
        <input v-model="tokenDraft" type="password" :placeholder="store.hasToken ? '已配置，输入新 Token 可覆盖' : 'pst-...'" />
      </div>
      <button class="btn primary" type="button" @click="verify">验证 Token / 刷新积分</button>
      <p class="hint">当前：{{ store.account.tierName }} · Anlas {{ store.account.anlasBalance ?? "—" }} · 到期 {{ store.account.expiresAt || "—" }}</p>
      <p class="hint">会话记录保存在本机数据目录，不依赖浏览器缓存。窗口以 WebView 无痕模式运行，站点 Cookie 不会跨启动残留。</p>
    </div>

    <div class="card">
      <h2>存储与网络</h2>
      <div class="field">
        <label>输出目录</label>
        <div class="row">
          <input v-model="store.settings.outputDir" placeholder="空则使用「图片/Langbai NovelAI」" />
          <button class="btn" type="button" @click="chooseDir">选择…</button>
          <button class="btn" type="button" @click="openOutputDir()">打开</button>
        </div>
      </div>
      <div class="field">
        <label>Image API</label>
        <input v-model="store.settings.imageBaseUrl" />
      </div>
      <div class="field">
        <label>HTTP / SOCKS 代理（可选）</label>
        <input v-model="store.settings.proxyUrl" placeholder="http://127.0.0.1:7890 或 socks5://127.0.0.1:10808" />
      </div>
      <label class="check">
        <input v-model="store.settings.streamPreviewEnabled" type="checkbox" />
        流式预览（逐步显示生成过程，失败时自动回退 ZIP）
      </label>
      <label class="check">
        <input v-model="store.settings.allowCustomEndpoint" type="checkbox" />
        允许把 Token 发到非官方端点
      </label>
      <label class="check">
        <input v-model="store.settings.allowCustomEndpointFallback" type="checkbox" />
        自定义端点 401/403 时回退官方
      </label>
      <div class="actions">
        <button class="btn primary" type="button" @click="save">保存设置</button>
      </div>
    </div>
    <p v-if="message">{{ message }}</p>
  </div>
</template>

<style scoped>
.wrap { padding: 20px; display: grid; gap: 16px; max-width: 780px; }
.check { display: flex; gap: 8px; align-items: center; margin: 8px 0; }
.actions { margin-top: 12px; }
</style>
