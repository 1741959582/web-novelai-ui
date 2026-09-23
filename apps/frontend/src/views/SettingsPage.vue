<script setup lang="ts">
import { onMounted, ref } from "vue";
import { censorDownload, censorOpenDir, censorStatus, danbooruStatus, downloadDanbooru, openOutputDir, pickOutputDir, type CensorEngineInfo } from "@/api/tauri";
import { useAppStore } from "@/stores/app";
import ClTaggerPanel from "@/components/ClTaggerPanel.vue";
import NaiIcon from "@/components/NaiIcon.vue";

type Tab = "api" | "storage" | "tags" | "tagger" | "censor" | "perf" | "about";

const store = useAppStore();
const tab = ref<Tab>("api");
const tokenDraft = ref("");
const message = ref("");
const tagBusy = ref(false);
const tagLib = ref({ downloaded: false, count: 0 });
const censorEngines = ref<CensorEngineInfo[]>([]);
const censorDir = ref("");
const censorBusy = ref("");

onMounted(async () => {
  try {
    tagLib.value = await danbooruStatus();
  } catch {
    /* ignore */
  }
});

async function loadCensor() {
  try {
    const status = await censorStatus();
    censorEngines.value = status.engines;
    censorDir.value = status.dir;
  } catch (error) {
    message.value = error instanceof Error ? error.message : String(error);
  }
}

async function downloadCensor(id = "") {
  const pending = censorEngines.value.filter((engine) => !engine.present && (!id || engine.id === id));
  if (!pending.length) {
    message.value = "打码模型都已就绪";
    return;
  }
  censorBusy.value = pending[0].id;
  try {
    for (const engine of pending) {
      censorBusy.value = engine.id;
      message.value = `正在下载 ${engine.title}…`;
      const status = await censorDownload(engine.id);
      censorEngines.value = status.engines;
      censorDir.value = status.dir;
    }
    message.value = "打码模型已下载到本机目录";
  } catch (error) {
    message.value = error instanceof Error ? error.message : String(error);
  } finally {
    censorBusy.value = "";
  }
}

async function downloadTags() {
  if (tagBusy.value) return;
  tagBusy.value = true;
  message.value = "正在下载中文标签库（约 7MB，每个标签均含中文）…";
  try {
    tagLib.value = await downloadDanbooru();
    message.value = `已下载中文标签库（${tagLib.value.count} 条，均含中文）。`;
  } catch (e) {
    message.value = e instanceof Error ? e.message : String(e);
  } finally {
    tagBusy.value = false;
  }
}

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
  <div class="page">
    <nav class="nav">
      <button type="button" :class="{ on: tab === 'api' }" @click="tab = 'api'">
        <NaiIcon name="globe" :size="16" />
        <span>API 配置</span>
      </button>
      <button type="button" :class="{ on: tab === 'storage' }" @click="tab = 'storage'">
        <NaiIcon name="folder" :size="16" />
        <span>存储与网络</span>
      </button>
      <button type="button" :class="{ on: tab === 'tags' }" @click="tab = 'tags'">
        <NaiIcon name="bulb" :size="16" />
        <span>提示词补全</span>
      </button>
      <button type="button" :class="{ on: tab === 'tagger' }" @click="tab = 'tagger'">
        <NaiIcon name="diamond" :size="16" />
        <span>本地打标</span>
      </button>
      <button type="button" :class="{ on: tab === 'censor' }" @click="tab = 'censor'; loadCensor()">
        <NaiIcon name="folder" :size="16" />
        <span>打码模型</span>
      </button>
      <button type="button" :class="{ on: tab === 'perf' }" @click="tab = 'perf'">
        <NaiIcon name="sparkle" :size="16" />
        <span>性能</span>
      </button>
      <button type="button" :class="{ on: tab === 'about' }" @click="tab = 'about'">
        <NaiIcon name="globe" :size="16" />
        <span>关于与更新</span>
      </button>
    </nav>

    <section class="content">
      <div v-if="tab === 'api'" class="card">
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

      <div v-else-if="tab === 'storage'" class="card">
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

      <div v-else-if="tab === 'tags'" class="card">
        <h2>提示词补全</h2>
        <p class="hint">来源 DanbooruSearchOnline（GPL-3.0，已固定版本），单独下载、不打包进程序。下载后可用中文或英文补全并显示热度。</p>
        <div class="field">
          <label>标签库状态</label>
          <input
            readOnly
            :value="tagLib.downloaded ? `已下载（${tagLib.count} 条，均含中文）` : '未下载（补全将使用内置精简词库）'"
          />
        </div>
        <div class="actions">
          <button class="btn primary" type="button" :disabled="tagBusy" @click="downloadTags">
            {{ tagBusy ? "下载中…" : tagLib.downloaded ? "重新下载" : "下载标签库" }}
          </button>
        </div>
      </div>

      <div v-else-if="tab === 'tagger'" class="card">
        <h2>本地打标</h2>
        <ClTaggerPanel />
      </div>

      <div v-else-if="tab === 'censor'" class="card">
        <h2>打码模型</h2>
        <p class="hint">和安装包一样放在 GitHub Release 上，不打包进程序。下载后保存到本机打码目录，打马赛克页会直接使用。</p>
        <div v-for="engine in censorEngines" :key="engine.id" class="model">
          <b>{{ engine.title }}</b>
          <span>{{ engine.present ? "已就绪" : "未下载" }}</span>
          <button class="btn" type="button" :disabled="!!censorBusy || engine.present" @click="downloadCensor(engine.id)">
            {{ censorBusy === engine.id ? "下载中…" : "下载" }}
          </button>
        </div>
        <p v-if="!censorEngines.length" class="hint">还没有读到模型列表。</p>
        <div class="actions">
          <button class="btn primary" type="button" :disabled="!!censorBusy" @click="downloadCensor()">
            {{ censorBusy ? "下载中…" : "下载未就绪的模型" }}
          </button>
          <button class="btn" type="button" @click="censorOpenDir()">打开模型目录</button>
          <button class="btn" type="button" @click="loadCensor()">刷新</button>
        </div>
        <p class="hint">{{ censorDir }}</p>
      </div>

      <div v-else-if="tab === 'perf'" class="card">
        <h2>性能</h2>
        <label class="check">
          <input v-model="store.settings.streamPreviewEnabled" type="checkbox" @change="store.saveSettings()" />
          流式预览（逐步显示生成过程，失败时自动回退 ZIP）
        </label>
        <p class="hint">也可在生成画布右上角开关流式预览。</p>
      </div>

      <div v-else class="card">
        <h2>关于与更新</h2>
        <p class="hint">当前版本 {{ store.updateInfo?.current || store.appVersion || "—" }}</p>
        <p class="hint">
          GitHub 最新版
          {{ store.updateInfo ? (store.updateInfo.available ? store.updateInfo.latest : "已是最新") : "尚未检查" }}
        </p>
        <p class="hint">启动时会读取 GitHub Release。有新包时顶栏会出现更新条，下载安装包后自动打开安装程序。</p>
        <p v-if="store.updateBusy" class="hint">{{ store.updateMsg || `下载中 ${store.updatePct}%` }}</p>
        <div class="actions">
          <button class="btn" type="button" :disabled="store.updateBusy" @click="store.checkForAppUpdate()">检查更新</button>
          <button
            class="btn primary"
            type="button"
            :disabled="store.updateBusy || !store.updateInfo?.available"
            @click="store.applyAppUpdate()"
          >
            {{ store.updateBusy ? "更新中…" : "立即更新" }}
          </button>
          <button class="btn" type="button" @click="store.openUpdatePage()">打开发行页</button>
        </div>
      </div>

      <p v-if="message" class="note">{{ message }}</p>
    </section>
  </div>
</template>

<style scoped>
.page {
  height: 100%;
  min-height: 0;
  display: grid;
  grid-template-columns: 200px minmax(0, 1fr);
  background: var(--bg0);
}
.nav {
  height: 100%;
  overflow: auto;
  padding: 14px 10px;
  border-right: 1px solid var(--bg3);
  display: flex;
  flex-direction: column;
  gap: 4px;
}
.nav button {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 40px;
  padding: 0 12px;
  border: 0;
  border-radius: 8px;
  background: transparent;
  color: rgba(255,255,255,0.72);
  text-align: left;
  font-size: 13px;
}
.nav button.on,
.nav button:hover {
  background: var(--bg2);
  color: var(--heading);
}
.content {
  min-width: 0;
  height: 100%;
  overflow: auto;
  padding: 20px 24px 32px;
}
.card {
  max-width: 720px;
  display: grid;
  gap: 10px;
}
.check { display: flex; gap: 8px; align-items: center; margin: 8px 0; }
.actions { margin-top: 8px; display: flex; gap: 8px; flex-wrap: wrap; }
.model {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto auto;
  gap: 10px;
  align-items: center;
  padding: 8px 0;
  border-bottom: 1px solid var(--bg3);
}
.model b { font-weight: 600; }
.model span { color: rgba(255,255,255,0.55); font-size: 12px; }
.note { color: var(--heading); font-size: 13px; margin-top: 16px; }
</style>
