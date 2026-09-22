<script setup lang="ts">
import { ref } from "vue";
import { useAppStore } from "@/stores/app";
import ClTaggerPanel from "@/components/ClTaggerPanel.vue";

const store = useAppStore();
const step = ref(0);
const tokenDraft = ref("");
const message = ref("");
const busy = ref(false);

async function verify() {
  if (!tokenDraft.value.trim()) {
    message.value = "可以先跳过，之后在设置里填写 NovelAI Token。";
    return;
  }
  busy.value = true;
  try {
    const res = await store.verifyToken(tokenDraft.value);
    message.value = res.message;
    if (res.valid) tokenDraft.value = "";
  } finally {
    busy.value = false;
  }
}

async function finish() {
  busy.value = true;
  try {
    store.settings.hasOnboarded = true;
    await store.saveSettings();
  } catch (e) {
    store.settings.hasOnboarded = false;
    message.value = e instanceof Error ? e.message : String(e);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <div class="backdrop">
    <section class="panel">
      <p class="step">首次安装 {{ step + 1 }} / 3</p>
      <template v-if="step === 0">
        <h1>欢迎使用</h1>
        <p class="hint">Token 只保存在本机 Rust 侧。这一步可以跳过，之后到设置里再填。</p>
        <div class="field">
          <label>NovelAI Persistent API Token</label>
          <input v-model="tokenDraft" type="password" placeholder="pst-..." />
        </div>
        <div class="acts">
          <button class="btn" type="button" :disabled="busy" @click="verify">验证</button>
          <button class="btn primary" type="button" @click="step = 1">下一步</button>
        </div>
        <p v-if="message" class="note">{{ message }}</p>
      </template>
      <template v-else-if="step === 1">
        <h1>本地打标</h1>
        <ClTaggerPanel />
        <div class="acts">
          <button class="btn" type="button" @click="step = 0">上一步</button>
          <button class="btn primary" type="button" @click="step = 2">下一步</button>
        </div>
      </template>
      <template v-else>
        <h1>可以开始了</h1>
        <p class="hint">
          反推页可选 WD Tagger 或 CL Tagger v2。若已启用本地模型，CL 会走本机显卡，不再占用 Hugging Face 空间的免费 GPU 额度。
        </p>
        <p v-if="message" class="note">{{ message }}</p>
        <div class="acts">
          <button class="btn" type="button" @click="step = 1">上一步</button>
          <button class="btn primary" type="button" :disabled="busy" @click="finish">进入应用</button>
        </div>
      </template>
    </section>
  </div>
</template>

<style scoped>
.backdrop {
  position: fixed;
  inset: 0;
  z-index: 50;
  background: rgba(14, 15, 33, 0.82);
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 24px;
}
.panel {
  width: min(560px, calc(100vw - 32px));
  max-height: calc(100vh - 48px);
  overflow: auto;
  background: #191b31;
  border: 1px solid #22253f;
  border-radius: 12px;
  padding: 22px 20px 18px;
  box-shadow: 0 18px 50px rgba(0, 0, 0, 0.45);
  display: grid;
  gap: 10px;
}
.step {
  margin: 0;
  color: #9aa3c7;
  font-size: 12px;
  letter-spacing: 0.4px;
}
h1 { margin: 0; font-size: 26px; }
.acts { display: flex; gap: 8px; justify-content: flex-end; margin-top: 8px; }
.note { color: var(--heading); font-size: 13px; margin: 0; }
</style>
