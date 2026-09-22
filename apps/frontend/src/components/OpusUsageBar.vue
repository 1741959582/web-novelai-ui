<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { isV5 } from "@/types/nai";
import { useAppStore } from "@/stores/app";
import NaiIcon from "@/components/NaiIcon.vue";

/** sharednai5 / official FAQ: ~17 images per 1%, cap ~1700, ~185/day when until=7888. */
const IMAGES_PER_PERCENT = 17;
const DEFAULT_MAX = 1700;
const DEFAULT_DAILY = 185;

const store = useAppStore();
const open = ref(false);
const tick = ref(Date.now());

const visible = computed(() => isV5(store.params.model));
const usage = computed(() => store.account.opusUsage ?? null);
const until = computed(() => usage.value?.timeUntilNextPercent ?? 0);
const maxImages = computed(() => {
  const raw = usage.value?.maxImages ?? 0;
  return raw > 0 ? raw : DEFAULT_MAX;
});
const liveRemaining = computed(() => {
  if (!usage.value) return null;
  if (usage.value.isNegative) return 0;
  const base = usage.value.remainingImages && usage.value.remainingImages > 0
    ? usage.value.remainingImages
    : IMAGES_PER_PERCENT * Math.min(100, Math.max(0, usage.value.percent));
  const updatedAt = store.account.opusUsageUpdatedAt ?? 0;
  if (base >= maxImages.value - 0.01 || until.value <= 0 || updatedAt <= 0) {
    return Math.min(maxImages.value, base);
  }
  const elapsed = Math.max(0, (tick.value - updatedAt) / 1000);
  return Math.min(maxImages.value, base + (elapsed / until.value) * IMAGES_PER_PERCENT);
});
const remainingImages = computed(() => (liveRemaining.value === null ? null : Math.round(liveRemaining.value)));
const percent = computed(() => {
  if (liveRemaining.value === null) return null;
  return Math.round(Math.min(100, Math.max(0, liveRemaining.value / IMAGES_PER_PERCENT)) * 10) / 10;
});
const dailyImages = computed(() => {
  const raw = usage.value?.dailyRefillImages ?? 0;
  if (raw > 0) return Math.round(raw);
  if (until.value > 0) return Math.round(IMAGES_PER_PERCENT * (86_400 / until.value));
  return DEFAULT_DAILY;
});
const fullHours = computed(() => (until.value > 0 ? Math.round((until.value * 100) / 3600) : 220));
const full = computed(() => (remainingImages.value ?? 0) >= maxImages.value - 0.5 && !usage.value?.isNegative);
const recoveryText = computed(() => {
  if (full.value) return "恢复已暂停（能量已满 100%）";
  return `正在随时间自动恢复补充（每日恢复约 ${dailyImages.value} 张）`;
});
const poolImages = computed(() => Math.round(maxImages.value));

let timer: number | undefined;
let clock: number | undefined;
function startPoll() {
  stopPoll();
  if (!visible.value || !store.hasToken) return;
  void store.refreshAccount(true);
  timer = window.setInterval(() => {
    void store.refreshAccount(true);
  }, 30_000);
  clock = window.setInterval(() => {
    tick.value = Date.now();
  }, 5_000);
}
function stopPoll() {
  if (timer) window.clearInterval(timer);
  if (clock) window.clearInterval(clock);
  timer = undefined;
  clock = undefined;
}

watch([visible, () => store.hasToken], startPoll, { immediate: true });
watch(open, (value) => {
  if (value) void store.refreshAccount(true);
});
onMounted(startPoll);
onUnmounted(stopPoll);
</script>

<template>
  <div v-if="visible" class="wrap">
    <div class="compact">
      <div class="track" role="progressbar" :aria-valuenow="percent ?? 0" aria-valuemin="0" aria-valuemax="100">
        <i :style="{ width: `${percent ?? 0}%` }" />
      </div>
      <div class="line">
        <span>{{ percent === null ? "--" : `${percent}%` }} of Opus Generations remaining{{ remainingImages === null ? "" : ` (~${remainingImages})` }}</span>
        <button type="button" class="more" @click="open = true">More Info</button>
      </div>
    </div>

    <div v-if="open" class="back" @click.self="open = false">
      <section class="panel" role="dialog" aria-labelledby="opus-title">
        <header>
          <h2 id="opus-title"><NaiIcon name="sparkle" :size="16" /> Opus Generation Usage Limit</h2>
          <button class="x" type="button" aria-label="关闭" @click="open = false"><NaiIcon name="close" :size="16" /></button>
        </header>
        <p class="lead">
          您的 Opus 会员权包含在标准分辨率（≤ 1,048,576 像素）和 28 步以内的免费 NovelAI Diffusion V5 绘图生成。该额度有限并随时间自动恢复补充。当额度耗尽时，您仍可通过消耗 Anlas 点数继续生成图片。
        </p>

        <div class="stats">
          <strong>{{ percent === null ? "--" : `${percent}%` }} 剩余（约 {{ remainingImages ?? "--" }} 张图片）</strong>
          <span>{{ recoveryText }}</span>
        </div>
        <div class="track" role="progressbar" :aria-valuenow="percent ?? 0" aria-valuemin="0" aria-valuemax="100">
          <i :style="{ width: `${percent ?? 0}%` }" />
        </div>

        <div class="cols">
          <div>
            <h3>套餐能量池分配</h3>
            <p>独享 / 管理员</p>
            <small>总能量池平均分为 1 份（满额约 {{ poolImages }} 张）。</small>
          </div>
          <div>
            <h3>每日恢复额度</h3>
            <p>{{ dailyImages === null ? "—" : `+约 ${dailyImages} 张/天` }}</p>
            <small>{{ fullHours === null ? "恢复速度以官网同步为准" : `约 ${fullHours} 小时完全回满` }}</small>
          </div>
        </div>
        <p class="note">
          能量池从 0% 完全回满周期约为 {{ fullHours ?? 220 }} 小时（您个人份额每日恢复约 {{ dailyImages ?? 185 }} 张）。
        </p>
        <button class="close" type="button" @click="open = false">关闭</button>
      </section>
    </div>
  </div>
</template>

<style scoped>
.compact { display: grid; gap: 6px; padding: 2px 2px 0; }
.compact > .track { height: 6px; margin: 0; }
.line {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  color: rgba(255,255,255,0.72);
  font-size: 12px;
}
.more {
  background: none;
  border: 0;
  color: rgba(255,255,255,0.78);
  padding: 0;
  text-decoration: underline;
  text-underline-offset: 2px;
}
.back {
  position: fixed;
  inset: 0;
  z-index: 50;
  background: rgba(10, 12, 28, 0.62);
  display: grid;
  place-items: center;
}
.panel {
  width: min(560px, calc(100vw - 32px));
  background: #16182c;
  border: 1px solid #2a2d48;
  border-radius: 16px;
  padding: 22px 24px 20px;
  color: #fff;
  box-shadow: 0 24px 60px rgba(0,0,0,0.45);
}
.panel header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 12px; }
.panel h2 {
  margin: 0;
  display: inline-flex;
  align-items: center;
  gap: 8px;
  font-size: 18px;
}
.lead { margin: 0 0 18px; color: rgba(255,255,255,0.78); font-size: 13px; line-height: 1.55; }
.stats {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  font-size: 13px;
}
.stats span { color: rgba(255,255,255,0.62); }
.track {
  height: 6px;
  margin: 10px 0 18px;
  overflow: hidden;
  border-radius: 99px;
  background: #2a2d48;
}
.track i {
  display: block;
  height: 100%;
  background: #f4f1dc;
}
.cols {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 18px;
  margin-bottom: 14px;
}
.cols h3 { margin: 0 0 6px; font-size: 12px; color: rgba(255,255,255,0.55); font-weight: 600; }
.cols p { margin: 0 0 4px; font-size: 14px; }
.cols small { color: rgba(255,255,255,0.55); font-size: 12px; line-height: 1.4; }
.note { margin: 0 0 16px; color: rgba(255,255,255,0.5); font-size: 12px; line-height: 1.45; }
.x {
  background: none;
  border: 0;
  color: rgba(255,255,255,0.7);
  padding: 4px;
}
.close {
  display: block;
  margin-left: auto;
  background: #f4f1dc;
  color: #1b1d32;
  border: 0;
  border-radius: 999px;
  padding: 8px 18px;
  font-weight: 700;
}
</style>
