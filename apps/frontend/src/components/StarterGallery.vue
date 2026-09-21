<script setup lang="ts">
import { useAppStore } from "@/stores/app";

const store = useAppStore();

const samples = [
  { id: "a", title: "Forest path", src: "/starter/a.png", prompt: "1girl, walking a forest path with a large white wolf, flowers, sunlight, cinematic lighting" },
  { id: "b", title: "Lily pond", src: "/starter/b.png", prompt: "a frog sitting on a log in a lily pond, city skyline in the distance, watercolor, dusk" },
  { id: "c", title: "Garden gazebo", src: "/starter/c.png", prompt: "1girl, sitting in a rose garden gazebo, maid outfit, afternoon light, detailed flowers" },
  { id: "d", title: "Fox shrine", src: "/starter/d.png", prompt: "fox girl, forest shrine stairs, lanterns, soft sunlight, looking at viewer" },
  { id: "e", title: "Bar night", src: "/starter/e.png", prompt: "1girl, short blue hair, sitting at a bar, neon bottles, side glance, night" },
  { id: "f", title: "Sleepy cat", src: "/starter/f.png", prompt: "a fluffy cat portrait, warm indoor light, close-up, detailed fur" },
];

function usePrompt(prompt: string) {
  if (!store.characters.length) store.addCharacter();
  const first = store.characters[0];
  if (first) store.updateCharacter(first.id, { prompt });
  store.status = "已复制示例提示词";
}
</script>

<template>
  <div class="started">
    <div class="hero">
      <svg class="spark sl" viewBox="0 0 64 64" aria-hidden="true">
        <path d="M32 2l4.2 18.6L54 26l-17 6.2L32 50l-5-17.8L10 26l16.8-5.4L32 2Z" fill="#F5F3C2" />
      </svg>
      <div class="copy">
        <h1>Get Started</h1>
        <p>Get inspiration from our quick start gallery!</p>
        <small>Click an image to copy the prompt.</small>
      </div>
      <svg class="spark sr" viewBox="0 0 64 64" aria-hidden="true">
        <path d="M32 4l3.4 14.8L50 22.6l-13.6 5L32 42l-4.4-14.4L14 22.6l13.6-3.8L32 4Z" fill="#F5F3C2" />
      </svg>
      <span class="dia d1" />
      <span class="dia d2" />
    </div>
    <div class="masonry">
      <button
        v-for="s in samples"
        :key="s.id"
        class="tile"
        type="button"
        @click="usePrompt(s.prompt)"
      >
        <img :src="s.src" :alt="s.title" />
      </button>
    </div>
  </div>
</template>

<style scoped>
.started {
  width: min(820px, 92%);
  margin: 0 auto;
  padding: 36px 12px 48px;
}
.hero {
  position: relative;
  text-align: center;
  margin-bottom: 28px;
}
.copy h1 {
  font-family: Eczar, serif;
  color: #f5f3c2;
  font-size: 34px;
  font-weight: 600;
  letter-spacing: 0.5px;
  margin: 0 0 6px;
}
.copy p, .copy small {
  color: rgba(255, 255, 255, 0.72);
  margin: 0;
  font-size: 14px;
}
.copy small { display: block; margin-top: 10px; }
.spark {
  position: absolute;
  top: -8px;
  width: 28px;
  height: 28px;
  opacity: 0.95;
}
.sl { left: calc(50% - 210px); }
.sr { right: calc(50% - 210px); top: 6px; width: 20px; height: 20px; }
.dia {
  position: absolute;
  width: 10px;
  height: 10px;
  background: #f5f3c2;
  opacity: 0.18;
  transform: rotate(45deg);
}
.d1 { left: 18%; top: 18px; }
.d2 { right: 16%; top: 40px; width: 14px; height: 14px; }
.masonry {
  columns: 3;
  column-gap: 14px;
}
.tile {
  width: 100%;
  break-inside: avoid;
  margin: 0 0 14px;
  padding: 0;
  border: 0;
  border-radius: 6px;
  overflow: hidden;
  background: #13152c;
  display: block;
}
.tile img {
  display: block;
  width: 100%;
  height: auto;
}
@media (max-width: 900px) {
  .masonry { columns: 2; }
  .sl, .sr { display: none; }
}
</style>
