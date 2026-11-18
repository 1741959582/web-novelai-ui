<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import NaiIcon from "@/components/NaiIcon.vue";
import {
  AGENT_BUILTIN_PROMPT,
  AGENT_GLYPHS,
  composeAgentSystem,
  useNaiAgentStore,
  type AgentQuick,
  type CacheWarming,
} from "@/stores/naiAgent";

withDefaults(defineProps<{ part?: "all" | "quick" }>(), { part: "all" });

const store = useNaiAgentStore();
const showFinal = ref(false);
const showBuiltin = ref(false);
const mcpName = ref("");
const mcpJson = ref("{\n  \"command\": \"\",\n  \"args\": []\n}");
const mcpError = ref("");

onMounted(() => {
  void store.load();
});

const shown = computed(() => Math.min(6, store.quick.length));
const finalPrompt = computed(() => composeAgentSystem({
  prefix: store.prefix,
  instruction: store.instruction,
  scene: "（发送时附上当前草稿、角色和参考图）",
  skipPaid: store.skipPaid,
  bypass: store.permission === "bypass",
}));
const order = ["内置提示词前", "内置系统提示词", "全局指令", "当前画面"];

function moveQuick(index: number, dir: -1 | 1) {
  const next = index + dir;
  if (next < 0 || next >= store.quick.length) return;
  const copy = store.quick.slice();
  const [item] = copy.splice(index, 1);
  copy.splice(next, 0, item);
  store.quick = copy;
}

function cycleGlyph(item: AgentQuick) {
  const index = AGENT_GLYPHS.indexOf(item.glyph);
  item.glyph = AGENT_GLYPHS[(index + 1) % AGENT_GLYPHS.length];
}

function addQuick() {
  store.quick.push({
    id: crypto.randomUUID(),
    glyph: "sparkle",
    title: "新快捷信息",
    hint: "",
    prompt: "",
    send: false,
  });
}

function removeQuick(item: AgentQuick) {
  const title = item.title.trim() || "未命名";
  if (!window.confirm(`删除快捷信息「${title}」？`)) return;
  store.quick = store.quick.filter((entry) => entry.id !== item.id);
}

function restoreQuick() {
  if (!window.confirm("恢复成内置的 4 条快捷信息？你新增和改过的会被替换。")) return;
  store.restoreQuick();
}

function addMcp() {
  mcpError.value = "";
  const name = mcpName.value.trim();
  if (!/^[A-Za-z0-9_-]{1,64}$/.test(name) || name.replace(/-/g, "_") === "nai_studio") {
    mcpError.value = "名称只能用字母、数字、下划线和连字符，且不能叫 nai_studio。";
    return;
  }
  if (store.mcpServers.some((item) => item.name === name)) {
    mcpError.value = "已经有同名服务器。";
    return;
  }
  try {
    JSON.parse(mcpJson.value);
  } catch {
    mcpError.value = "MCP JSON 无法解析。";
    return;
  }
  store.mcpServers.push({ id: crypto.randomUUID(), name, json: mcpJson.value, enabled: true });
  mcpName.value = "";
}

function setCache(value: string) {
  const next: CacheWarming = value === "off" || value === "idle" ? value : "streaming";
  store.cacheWarming = next;
}

function setRequests(value: string) {
  const n = Number(value);
  if (!Number.isFinite(n)) return;
  store.maxRequests = Math.max(1, Math.min(200, Math.round(n)));
}
</script>

<template>
  <div class="settings">
    <template v-if="part === 'all'">
      <header class="page-head">
        <h2>NAI Agent</h2>
        <p>侧栏助手的模型、指令、快捷信息和工具。对话存在本机，钥匙用 DeepSeek 页的同一把。</p>
      </header>

      <section>
        <h3>模型</h3>
        <div class="row">
          <div>
            <b>默认模型</b>
            <span>思考强度在聊天框里按任务调整</span>
          </div>
          <select v-model="store.modelId">
            <option value="">未选择模型</option>
            <option v-for="id in store.models" :key="id" :value="id">{{ id }}</option>
          </select>
        </div>
        <label class="row">
          <div>
            <b>Enter 直接发送</b>
            <span>关掉后用 Ctrl+Enter 发送，Enter 换行</span>
          </div>
          <input v-model="store.enterToSend" type="checkbox" />
        </label>
        <label class="row">
          <div>
            <b>单次任务最多请求</b>
            <span>防止上下文无限变长。一次发送最多带上这么多次往来。</span>
          </div>
          <span class="stepper">
            <input :value="store.maxRequests" type="number" min="1" max="200" @change="setRequests(($event.target as HTMLInputElement).value)" />
            次
          </span>
        </label>
        <label class="row">
          <div>
            <b>付费也无需确认</b>
            <span>仅完全权限生效。免费的提示词写入本来就会直接执行；打开后，系统提示改为不再等待付费确认。网页版不会替你按生成按钮扣 Anlas。</span>
          </div>
          <input v-model="store.skipPaid" type="checkbox" />
        </label>
        <label class="row">
          <div>
            <b>下一句建议</b>
            <span>回复里如果带了下一句，会显示在输入框上方，按 Tab 填入。不额外请求模型。</span>
          </div>
          <input v-model="store.nextSuggestion" type="checkbox" />
        </label>
        <div class="row">
          <div>
            <b>提示缓存保温</b>
            <span>通过轻量请求延长提示缓存，减少后续请求重新计算上下文的开销；是否命中缓存以服务商为准。{{ store.cacheWarming === "idle" ? "空闲时也保温（最多 30 分钟）会产生额外模型费用。" : "" }}网页版只记下这个选择，不会在后台自己发请求。</span>
          </div>
          <select :value="store.cacheWarming" @change="setCache(($event.target as HTMLSelectElement).value)">
            <option value="off">关闭</option>
            <option value="streaming">只在 Agent 运行时</option>
            <option value="idle">空闲时也保温</option>
          </select>
        </div>
      </section>

      <section>
        <h3>指令</h3>
        <p class="sub">顺序固定。改动从下一条消息开始生效。</p>
        <div class="order">
          <template v-for="(label, index) in order" :key="label">
            <span :class="{ on: index === 2 }">{{ label }}</span>
            <i v-if="index < order.length - 1">›</i>
          </template>
        </div>
        <label class="block">
          <b>内置提示词前</b>
          <span>在内置系统提示词之前加入的固定内容，例如身份或输出格式。</span>
          <textarea v-model="store.prefix" rows="4" placeholder="还没写" />
        </label>
        <div class="block">
          <div class="line">
            <b>内置系统提示词</b>
            <button type="button" @click="showBuiltin = !showBuiltin">{{ showBuiltin ? "收起" : "查看" }}</button>
          </div>
          <pre v-if="showBuiltin">{{ AGENT_BUILTIN_PROMPT }}</pre>
        </div>
        <label class="block">
          <b>全局指令</b>
          <span>相当于系统级 AGENTS.md，对之后的每次请求生效。</span>
          <textarea v-model="store.instruction" rows="6" placeholder="还没写" />
        </label>
        <div class="block">
          <div class="line">
            <b>查看最终提示词</b>
            <button type="button" @click="showFinal = !showFinal">{{ showFinal ? "收起" : "查看" }}</button>
          </div>
          <pre v-if="showFinal">{{ finalPrompt }}</pre>
        </div>
      </section>
    </template>

    <section>
      <h3>快捷信息</h3>
      <p class="sub">侧栏欢迎页的快捷卡片。点卡片把内容填进输入框，也可以设成直接发送。欢迎页显示 {{ shown }} / {{ store.quick.length }}。</p>
      <p v-if="!store.quick.length" class="sub">还没有快捷信息，欢迎页不显示这一块。</p>
      <article v-for="(item, index) in store.quick" :key="item.id" class="quick">
        <div class="quick-top">
          <button type="button" class="icon" :title="'图标'" @click="cycleGlyph(item)">
            <NaiIcon :name="item.glyph" :size="15" />
          </button>
          <input v-model="item.title" placeholder="标题" />
          <span v-if="item.send" class="badge send">直接发送</span>
          <span v-if="index >= 6" class="badge">不显示</span>
          <button type="button" title="上移" :disabled="index === 0" @click="moveQuick(index, -1)">↑</button>
          <button type="button" title="下移" :disabled="index === store.quick.length - 1" @click="moveQuick(index, 1)">↓</button>
          <button type="button" class="danger" title="删除" @click="removeQuick(item)">删除</button>
        </div>
        <input v-model="item.hint" placeholder="说明（可不填）" />
        <textarea v-model="item.prompt" rows="3" placeholder="发给 Agent 的内容" />
        <label class="check">
          <input v-model="item.send" type="checkbox" />
          点击后直接发送（不勾选则填进输入框）
        </label>
      </article>
      <div class="actions">
        <button type="button" @click="addQuick">新增快捷信息</button>
        <button type="button" @click="restoreQuick">恢复默认</button>
      </div>
    </section>

    <template v-if="part === 'all'">
      <section>
        <h3>Skill</h3>
        <p class="sub">平时只把名称和一句话描述放进提示词。内置项保持开启。</p>
        <article class="row">
          <div>
            <b>nai-studio</b>
            <span>读取当前草稿、角色和参考图。</span>
          </div>
          <em>已加载</em>
        </article>
        <article class="row">
          <div>
            <b>nai-prompt</b>
            <span>Danbooru 标签和权重写法。</span>
          </div>
          <em>已加载</em>
        </article>
      </section>

      <section>
        <h3>扩展</h3>
        <p class="sub">外部扩展能运行本地代码。网页版只启用内置的 Studio 交互，不加载本机扩展文件。</p>
        <article class="row">
          <div>
            <b>Studio 交互</b>
            <span>读取当前草稿，并把选中的标签写进主提示词或指定角色。</span>
          </div>
          <em>内置 · 必需</em>
        </article>
      </section>

      <section>
        <h3>MCP 服务器</h3>
        <p class="sub">内置工具组已经接上。额外服务器会保存下来；网页版没有本地进程，所以它们保持尚未连接，不会把工具塞进对话。</p>
        <article class="row">
          <div>
            <b>NAI Studio · 内置工具组</b>
            <span>提示词、角色、参考图。生图仍由生成按钮执行，先报价再等你确认。</span>
          </div>
          <em>已连接 · 3 个工具</em>
        </article>
        <article v-for="server in store.mcpServers" :key="server.id" class="row">
          <div>
            <b>{{ server.name }}</b>
            <span>尚未连接</span>
          </div>
          <label class="check">
            <input v-model="server.enabled" type="checkbox" />
            启用
          </label>
          <button type="button" class="danger" @click="store.mcpServers = store.mcpServers.filter((item) => item.id !== server.id)">删除</button>
        </article>
        <label class="block">
          <b>添加 MCP 服务器</b>
          <input v-model="mcpName" placeholder="名称，例如 my-tools" />
          <textarea v-model="mcpJson" rows="5" />
        </label>
        <p v-if="mcpError" class="err">{{ mcpError }}</p>
        <button type="button" @click="addMcp">添加</button>
      </section>

      <section>
        <h3>工具</h3>
        <p class="sub">3 个。读取草稿、读取附带的参考图、把勾选的标签写入主提示词或某个 Character。生图类操作始终先说明费用，等你自己按生成。</p>
      </section>

      <section>
        <h3>数据</h3>
        <p class="sub">对话、指令和快捷信息保存在本机酒馆文件 nai_agent。API Key 不写进这个文件。</p>
      </section>
    </template>
  </div>
</template>

<style scoped>
.settings { max-width: 760px; display: flex; flex-direction: column; gap: 22px; color: #fff; }
.page-head h2, h3 { margin: 0; font-weight: 650; }
.page-head h2 { font-size: 20px; }
.page-head p, .sub, .row span, .block span { margin: 0; color: rgba(255, 255, 255, 0.55); font-size: 12px; line-height: 1.55; }
h3 { font-size: 14px; margin-bottom: 8px; }
section { display: flex; flex-direction: column; gap: 10px; }
.row, .quick, .block {
  padding: 12px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  background: #1a1e38;
}
.row { display: flex; align-items: center; gap: 12px; }
.row > div, .block { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
.row > div { flex: 1; }
.row b, .block b, .quick-top input { font-size: 13px; }
.row em { flex: none; color: rgba(245, 243, 194, 0.85); font-size: 11px; font-style: normal; }
select, input[type="number"], .block input, .quick input, textarea {
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 8px;
  background: #12142a;
  color: #fff;
  padding: 7px 8px;
  font-size: 12px;
}
select { max-width: 230px; }
textarea { width: 100%; resize: vertical; }
.stepper { display: inline-flex; align-items: center; gap: 6px; color: rgba(255, 255, 255, 0.55); font-size: 12px; }
.stepper input { width: 72px; }
.order { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
.order span, .badge {
  padding: 3px 8px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.06);
  color: rgba(255, 255, 255, 0.62);
  font-size: 11px;
}
.order span.on, .badge.send { background: rgba(245, 243, 194, 0.16); color: #f5f3c2; }
.order i { color: rgba(255, 255, 255, 0.35); font-style: normal; }
.line { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
pre {
  margin: 0;
  white-space: pre-wrap;
  color: rgba(255, 255, 255, 0.72);
  font-size: 12px;
  line-height: 1.5;
}
.quick { display: flex; flex-direction: column; gap: 8px; }
.quick-top { display: flex; align-items: center; gap: 6px; }
.quick-top input { flex: 1; }
.icon, .quick-top button, .actions button, .line button, .row > button, section > button {
  border: 0;
  border-radius: 8px;
  background: #12142a;
  color: #fff;
  padding: 6px 8px;
}
.icon { width: 32px; height: 32px; display: grid; place-items: center; color: #9ec4ff; }
button:disabled { opacity: 0.35; }
.danger { color: #ffb4b4 !important; }
.check { display: flex; align-items: center; gap: 8px; color: rgba(255, 255, 255, 0.72); font-size: 12px; }
.actions { display: flex; gap: 8px; }
.err { margin: 0; color: #ffb4b4; font-size: 12px; }
</style>
