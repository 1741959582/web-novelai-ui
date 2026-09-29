<script setup lang="ts">
import { computed, onUnmounted, ref } from "vue";
import { pickImages, reapiPublishImage, reapiSaveVideo, reapiVideoSubmit, reapiVideoTask, type VideoTask } from "@/api/tauri";
import { useAppStore } from "@/stores/app";

type Mode = "text" | "refs" | "frames";
type Slot = { preview: string; source: string; name: string };

const MODELS = [
  { id: "doubao-seedance-2.0-fast-face", label: "Seedance 2.0 Fast", resolutions: ["480p", "720p"] },
  { id: "doubao-seedance-2.0-face", label: "Seedance 2.0", resolutions: ["480p", "720p", "1080p", "4k"] },
  { id: "doubao-seedance-2.0-eco", label: "Seedance 2.0 Eco", resolutions: ["720p", "1080p", "4k"] },
];
const SIZES = ["16:9", "9:16", "1:1", "4:3", "3:4", "21:9", "adaptive"];

const store = useAppStore();
const mode = ref<Mode>("text");
const model = ref(MODELS[0].id);
const prompt = ref("");
const resolution = ref("720p");
const size = ref("16:9");
const duration = ref(5);
const bitrateMode = ref("default");
const generateAudio = ref(true);
const watermark = ref(false);
const returnLastFrame = ref(false);
const webSearch = ref(false);
const contentFilter = ref(false);
const more = ref(false);
const refs = ref<Slot[]>([]);
const firstFrame = ref<Slot | null>(null);
const lastFrame = ref<Slot | null>(null);
const urlDraft = ref("");
const busy = ref(false);
const note = ref("");
const videoUrl = ref("");
const lastFrameUrl = ref("");
const savedPath = ref("");
let pollTimer = 0;
let generation = 0;

const modelInfo = computed(() => MODELS.find((item) => item.id === model.value) || MODELS[0]);
const resolutions = computed(() => modelInfo.value.resolutions);
const autoDuration = computed(() => model.value !== "doubao-seedance-2.0-fast-face");

function snapResolution() {
  if (!resolutions.value.includes(resolution.value)) resolution.value = resolutions.value.includes("720p") ? "720p" : resolutions.value[0];
  if (!autoDuration.value && duration.value < 0) duration.value = 5;
}

function slotFrom(file: { path: string; dataUrl: string }): Slot {
  const name = file.path.split(/[/\\]/).pop() || "图片";
  return { preview: file.dataUrl || file.path, source: file.path || file.dataUrl, name };
}

async function addRefs() {
  const files = await pickImages();
  const room = 9 - refs.value.length;
  if (room <= 0) return;
  refs.value = [...refs.value, ...files.slice(0, room).map(slotFrom)];
}

function addUrl() {
  const url = urlDraft.value.trim();
  if (!/^https?:\/\//i.test(url)) {
    note.value = "参考图要填 http 或 https 地址";
    return;
  }
  const slot = { preview: url, source: url, name: url };
  if (mode.value === "refs") {
    if (refs.value.length >= 9) return;
    refs.value = [...refs.value, slot];
  } else if (!firstFrame.value) firstFrame.value = slot;
  else lastFrame.value = slot;
  urlDraft.value = "";
}

async function pickFrame(which: "first" | "last") {
  const files = await pickImages();
  if (!files[0]) return;
  const slot = slotFrom(files[0]);
  if (which === "first") firstFrame.value = slot;
  else lastFrame.value = slot;
}

async function publish(slot: Slot) {
  note.value = `正在上传 ${slot.name}`;
  return reapiPublishImage(slot.source);
}

function buildBody(images: { refs: string[]; first?: string; last?: string }) {
  const body: Record<string, unknown> = {
    model: model.value,
    prompt: prompt.value.trim(),
    resolution: resolution.value,
    size: size.value,
    duration: duration.value,
    bitrate_mode: bitrateMode.value,
    generate_audio: generateAudio.value,
    watermark: watermark.value,
    return_last_frame: returnLastFrame.value,
    content_filter: contentFilter.value,
  };
  if (webSearch.value) body.tools = [{ type: "web_search" }];
  if (mode.value === "refs" && images.refs.length) body.image_urls = images.refs;
  if (mode.value === "frames" && images.first) {
    const roles = [{ url: images.first, role: "first_frame" }];
    if (images.last) roles.push({ url: images.last, role: "last_frame" });
    body.image_with_roles = roles;
  }
  return body;
}

function unwrap(task: VideoTask): VideoTask {
  if (task.data && (task.data.id || task.data.status || task.data.task_id)) return task.data;
  return task;
}

function taskIdentity(task: VideoTask) {
  return task.id || task.task_id || "";
}

async function generate() {
  snapResolution();
  const text = prompt.value.trim();
  if (text.length < 3) {
    note.value = "提示词至少 3 个字";
    return;
  }
  if (mode.value === "frames" && !firstFrame.value) {
    note.value = "首尾帧至少要一张首帧";
    return;
  }
  if (store.settings.reapiToken !== "configured") {
    note.value = "先到设置 → API 配置里保存 reAPI Key";
    return;
  }
  const token = ++generation;
  window.clearTimeout(pollTimer);
  busy.value = true;
  videoUrl.value = "";
  lastFrameUrl.value = "";
  savedPath.value = "";
  try {
    const images = {
      refs: mode.value === "refs" ? await Promise.all(refs.value.map(publish)) : [],
      first: mode.value === "frames" && firstFrame.value ? await publish(firstFrame.value) : "",
      last: mode.value === "frames" && lastFrame.value ? await publish(lastFrame.value) : "",
    };
    if (token !== generation) return;
    note.value = "正在提交";
    const created = unwrap(await reapiVideoSubmit(buildBody(images)));
    const id = taskIdentity(created);
    if (!id) throw new Error("没有返回任务编号");
    await watchTask(id, token);
  } catch (error) {
    if (token === generation) note.value = error instanceof Error ? error.message : String(error);
  } finally {
    if (token === generation) busy.value = false;
  }
}

async function watchTask(id: string, token: number) {
  for (let i = 0; i < 180; i += 1) {
    if (token !== generation) return;
    const task = unwrap(await reapiVideoTask(id));
    if (task.status === "completed") {
      videoUrl.value = task.output?.video_urls?.[0] || "";
      lastFrameUrl.value = task.output?.last_frame_url || "";
      note.value = videoUrl.value ? "视频已生成，链接大约保留 30 天" : "任务完成，但没有视频地址";
      return;
    }
    if (task.status === "failed") {
      throw new Error(task.error?.message || "生成失败");
    }
    note.value = `生成中 ${i + 1}，任务 ${id}`;
    await new Promise((resolve) => {
      pollTimer = window.setTimeout(resolve, 4000);
    });
  }
  throw new Error("等太久了，可以稍后用任务号再查");
}

async function saveVideo() {
  if (!videoUrl.value) return;
  busy.value = true;
  try {
    savedPath.value = await reapiSaveVideo(videoUrl.value);
    note.value = `已保存到 ${savedPath.value}`;
  } catch (error) {
    note.value = error instanceof Error ? error.message : String(error);
  } finally {
    busy.value = false;
  }
}

function stop() {
  generation += 1;
  window.clearTimeout(pollTimer);
  busy.value = false;
  note.value = "已停止等待";
}

onUnmounted(() => {
  generation += 1;
  window.clearTimeout(pollTimer);
});
</script>

<template>
  <div class="page">
    <section class="form">
      <div class="modes">
        <button type="button" :class="{ on: mode === 'text' }" @click="mode = 'text'">文生视频</button>
        <button type="button" :class="{ on: mode === 'refs' }" @click="mode = 'refs'">参考图</button>
        <button type="button" :class="{ on: mode === 'frames' }" @click="mode = 'frames'">首尾帧</button>
      </div>
      <textarea v-model="prompt" rows="5" placeholder="描述要生成的视频，至少 3 个字" />
      <div class="row">
        <label>模型
          <select v-model="model" @change="snapResolution">
            <option v-for="item in MODELS" :key="item.id" :value="item.id">{{ item.label }}</option>
          </select>
        </label>
        <label>分辨率
          <select v-model="resolution">
            <option v-for="item in resolutions" :key="item" :value="item">{{ item }}</option>
          </select>
        </label>
        <label>比例
          <select v-model="size">
            <option v-for="item in SIZES" :key="item" :value="item">{{ item }}</option>
          </select>
        </label>
        <label>时长
          <select v-model.number="duration">
            <option v-if="autoDuration" :value="-1">自动</option>
            <option v-for="item in 12" :key="item" :value="item + 3">{{ item + 3 }} 秒</option>
          </select>
        </label>
      </div>
      <div v-if="mode === 'refs'" class="shots">
        <button v-for="(item, index) in refs" :key="item.source + index" type="button" class="shot" @click="refs.splice(index, 1)">
          <img :src="item.preview" alt="" />
          <i>移除</i>
        </button>
        <button v-if="refs.length < 9" type="button" class="shot add" @click="addRefs">加参考图</button>
      </div>
      <div v-else-if="mode === 'frames'" class="shots">
        <button type="button" class="shot" @click="firstFrame ? (firstFrame = null) : pickFrame('first')">
          <img v-if="firstFrame" :src="firstFrame.preview" alt="" />
          <span v-else>首帧</span>
          <i v-if="firstFrame">移除</i>
        </button>
        <button type="button" class="shot" @click="lastFrame ? (lastFrame = null) : pickFrame('last')">
          <img v-if="lastFrame" :src="lastFrame.preview" alt="" />
          <span v-else>尾帧，可空</span>
          <i v-if="lastFrame">移除</i>
        </button>
      </div>
      <div v-if="mode !== 'text'" class="url">
        <input v-model="urlDraft" placeholder="或粘贴图片的 http 地址" @keydown.enter.prevent="addUrl" />
        <button type="button" @click="addUrl">加入</button>
      </div>
      <p v-if="mode !== 'text'" class="hint">本机图片会先传到图床，再用返回的图片地址生成。</p>
      <button type="button" class="more" @click="more = !more">{{ more ? "收起" : "更多" }}</button>
      <div v-if="more" class="extra">
        <label>码率
          <select v-model="bitrateMode">
            <option value="default">default</option>
            <option value="standard">standard</option>
            <option value="high">high</option>
          </select>
        </label>
        <label class="chk"><input v-model="generateAudio" type="checkbox" />生成音频</label>
        <label class="chk"><input v-model="watermark" type="checkbox" />水印</label>
        <label class="chk"><input v-model="returnLastFrame" type="checkbox" />返回尾帧</label>
        <label class="chk"><input v-model="webSearch" type="checkbox" />联网检索</label>
        <label class="chk"><input v-model="contentFilter" type="checkbox" />内容过滤</label>
      </div>
      <p class="hint">内容过滤默认关闭，请求带 content_filter: false，可以生成 NSFW。</p>
      <div class="actions">
        <button type="button" class="primary" :disabled="busy" @click="generate">生成视频</button>
        <button v-if="busy" type="button" @click="stop">停止等待</button>
        <button type="button" :disabled="busy || !videoUrl" @click="saveVideo">保存到本机</button>
      </div>
      <p class="note">{{ note }}</p>
    </section>
    <section class="stage">
      <video v-if="videoUrl" :src="videoUrl" controls autoplay loop />
      <img v-else-if="lastFrameUrl" :src="lastFrameUrl" alt="" />
      <p v-else>生成完成后在这里播放。菜单里打开「视频」。</p>
      <img v-if="videoUrl && lastFrameUrl" class="frame" :src="lastFrameUrl" alt="尾帧" />
      <p v-if="savedPath" class="hint">{{ savedPath }}</p>
    </section>
  </div>
</template>

<style scoped>
.page { height: 100%; display: grid; grid-template-columns: minmax(360px, 460px) 1fr; min-height: 0; }
.form, .stage { min-height: 0; overflow: auto; }
.form { padding: 16px 18px 24px; display: flex; flex-direction: column; gap: 10px; }
.modes, .row, .actions, .extra, .url { display: flex; flex-wrap: wrap; gap: 8px; align-items: center; }
.modes button, .actions button, .more, .url button, .shot.add {
  border: 0;
  border-radius: 8px;
  background: var(--bg3);
  color: #fff;
  padding: 6px 12px;
}
.modes button.on, .actions .primary { background: var(--heading); color: #1b1730; }
textarea, .url input, select {
  width: 100%;
  background: var(--bg2);
  border-radius: 8px;
  padding: 8px 10px;
}
.row label, .extra label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; color: var(--muted); }
.row select, .extra select { width: auto; min-width: 96px; }
.chk { flex-direction: row !important; align-items: center; color: #fff; }
.shots { display: flex; gap: 8px; overflow-x: auto; }
.shot {
  width: 92px;
  height: 92px;
  flex: none;
  position: relative;
  border: 0;
  border-radius: 8px;
  background: var(--bg2);
  overflow: hidden;
  color: var(--muted);
}
.shot img { width: 100%; height: 100%; object-fit: cover; }
.shot i {
  position: absolute;
  right: 4px;
  bottom: 4px;
  font-style: normal;
  font-size: 11px;
  background: rgba(0,0,0,0.6);
  color: #fff;
  border-radius: 99px;
  padding: 1px 6px;
}
.more { align-self: flex-start; background: transparent; color: var(--heading); padding: 0; }
.hint, .note { margin: 0; color: var(--muted); font-size: 12px; }
.stage {
  display: grid;
  place-items: center;
  align-content: center;
  gap: 12px;
  padding: 16px;
  background: #0b0c18;
}
.stage video, .stage > img { max-width: 100%; max-height: calc(100% - 24px); border-radius: 8px; }
.frame { max-height: 120px; }
</style>
