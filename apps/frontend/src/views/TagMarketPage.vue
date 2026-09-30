<script setup lang="ts">
const url = "https://tags.novelai.dev/?scheme=light";
</script>

<template>
  <div class="market">
    <iframe :src="url" title="Danbooru 标签超市" referrerpolicy="no-referrer-when-downgrade" />
  </div>
</template>

<style scoped>
.market {
  height: 100%;
  min-height: 0;
  background: #0e0f21;
}
iframe {
  display: block;
  width: 100%;
  height: 100%;
  border: 0;
  background: #fff;
  color-scheme: light;
}
</style>
