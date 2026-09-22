<script setup lang="ts">
defineProps<{ name: string; size?: number }>();
</script>

<template>
  <svg
    class="nai-icon"
    :width="size ?? 18"
    :height="size ?? 18"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.7"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <template v-if="name === 'logo'">
      <path d="M7 20c2-6 4.2-11 10-16" />
      <path d="M14.2 5.2 17.8 8.8" />
      <path d="M8 18h4" />
      <path d="M9.2 14.5c.8 1.6 2.2 2.8 4.2 3.2" />
    </template>
    <template v-else-if="name === 'anlas'">
      <path d="M12 3 13.8 8.2 19 9.2 15.2 13 16.4 18.5 12 15.8 7.6 18.5 8.8 13 5 9.2 10.2 8.2 12 3Z" fill="currentColor" stroke="none" />
    </template>
    <template v-else-if="name === 'plus'">
      <rect x="3.5" y="3.5" width="17" height="17" rx="3" />
      <path d="M12 8v8M8 12h8" />
    </template>
    <template v-else-if="name === 'menu'">
      <path d="M5 7h14M5 12h14M5 17h14" />
    </template>
    <template v-else-if="name === 'close'">
      <path d="M6 6l12 12M18 6 6 18" />
    </template>
    <template v-else-if="name === 'pin'">
      <path d="M12 21s7-6.2 7-11a7 7 0 1 0-14 0c0 4.8 7 11 7 11Z" />
      <circle cx="12" cy="10" r="2.2" />
    </template>
    <template v-else-if="name === 'up'">
      <path d="M6 14l6-6 6 6" />
    </template>
    <template v-else-if="name === 'down'">
      <path d="M6 10l6 6 6-6" />
    </template>
    <template v-else-if="name === 'check'">
      <path d="M5 12.5 9.5 17 19 7.5" />
    </template>
    <template v-else-if="name === 'trash'">
      <path d="M5 7h14M9 7V5h6v2M8 7l.8 12h6.4L16 7" />
    </template>
    <template v-else-if="name === 'drag'">
      <path d="M9 7h.01M15 7h.01M9 12h.01M15 12h.01M9 17h.01M15 17h.01" stroke-width="3" />
    </template>
    <template v-else-if="name === 'grid'">
      <rect x="4" y="4" width="7" height="7" rx="1" />
      <rect x="13" y="4" width="7" height="7" rx="1" />
      <rect x="4" y="13" width="7" height="7" rx="1" />
      <rect x="13" y="13" width="7" height="7" rx="1" />
    </template>
    <template v-else-if="name === 'vibe'">
      <path d="M4 12c4-6 12-6 16 0" />
      <path d="M7 12c2.5-3.5 7.5-3.5 10 0" />
      <circle cx="12" cy="12" r="2" />
    </template>
    <template v-else-if="name === 'precise'">
      <circle cx="12" cy="12" r="3.2" />
      <path d="M12 4v2.5M12 17.5V20M4 12h2.5M17.5 12H20" />
    </template>
    <template v-else-if="name === 'i2i' || name === 'image'">
      <rect x="3.5" y="5" width="17" height="14" rx="2" />
      <circle cx="9" cy="10" r="1.6" />
      <path d="M4 16.5 9 12l4 3.2 2.2-1.7 4.8 3.5" />
    </template>
    <template v-else-if="name === 'crop'">
      <path d="M7 3v12a2 2 0 0 0 2 2h12" />
      <path d="M17 21V9a2 2 0 0 0-2-2H3" />
    </template>
    <template v-else-if="name === 'clip'">
      <path d="M16.5 7.5 9 15a3 3 0 0 0 4.2 4.2L20 12.5a5 5 0 0 0-7-7L6.2 12.2a2.4 2.4 0 0 0 3.4 3.4L16 9.2" />
    </template>
    <template v-else-if="name === 'dice'">
      <rect x="4" y="4" width="16" height="16" rx="3" />
      <circle cx="9" cy="9" r="1" fill="currentColor" stroke="none" />
      <circle cx="15" cy="9" r="1" fill="currentColor" stroke="none" />
      <circle cx="9" cy="15" r="1" fill="currentColor" stroke="none" />
      <circle cx="15" cy="15" r="1" fill="currentColor" stroke="none" />
      <circle cx="12" cy="12" r="1" fill="currentColor" stroke="none" />
    </template>
    <template v-else-if="name === 'reset'">
      <path d="M4 12a8 8 0 1 0 2.2-5.5" />
      <path d="M4 4v5h5" />
    </template>
    <template v-else-if="name === 'play'">
      <path d="M8 5.5v13L19 12 8 5.5Z" fill="currentColor" stroke="none" />
    </template>
    <template v-else-if="name === 'sparkle'">
      <path d="M12 3l1.4 6.1L20 10.5l-6.1 1.4L12 18l-1.4-6.1L4 10.5l6.6-1.4L12 3Z" fill="currentColor" stroke="none" />
    </template>
    <template v-else-if="name === 'diamond'">
      <path d="M12 3 16 12 12 21 8 12 12 3Z" fill="currentColor" stroke="none" />
    </template>
    <template v-else-if="name === 'landscape'">
      <rect x="3" y="7" width="18" height="10" rx="1.5" />
    </template>
    <template v-else-if="name === 'portrait'">
      <rect x="7" y="3" width="10" height="18" rx="1.5" />
    </template>
    <template v-else-if="name === 'square'">
      <rect x="5" y="5" width="14" height="14" rx="1.5" />
    </template>
    <template v-else-if="name === 'history'">
      <circle cx="12" cy="12" r="8" />
      <path d="M12 8v5l3 2" />
    </template>
    <template v-else-if="name === 'x'">
      <path d="M7 7l10 10M17 7 7 17" />
    </template>
    <template v-else-if="name === 'brush'">
      <path d="M14.5 4.5 19.5 9.5 10 19H5v-5L14.5 4.5Z" />
      <path d="M13 6.5 17.5 11" />
    </template>
    <template v-else-if="name === 'eraser'">
      <path d="M15.5 5.5 4.8 16.2a2 2 0 0 0 0 2.8l.2.2h6.2L20 10.4 15.5 5.5Z" />
      <path d="M8 19.2h11" />
    </template>
    <template v-else-if="name === 'upload'">
      <rect x="4" y="4" width="16" height="16" rx="3" />
      <path d="M12 16V9" />
      <path d="M8.5 12 12 8.5 15.5 12" />
    </template>
    <template v-else-if="name === 'dots'">
      <circle cx="6" cy="12" r="1.4" fill="currentColor" stroke="none" />
      <circle cx="12" cy="12" r="1.4" fill="currentColor" stroke="none" />
      <circle cx="18" cy="12" r="1.4" fill="currentColor" stroke="none" />
    </template>
    <template v-else-if="name === 'folder'">
      <path d="M4 7.5A1.5 1.5 0 0 1 5.5 6H9l1.6 1.8H18.5A1.5 1.5 0 0 1 20 9.3v7.2A1.5 1.5 0 0 1 18.5 18h-13A1.5 1.5 0 0 1 4 16.5v-9Z" />
    </template>
    <template v-else-if="name === 'copy'">
      <rect x="8" y="8" width="11" height="11" rx="1.6" />
      <path d="M6 15.5V6.2A1.2 1.2 0 0 1 7.2 5H15" />
    </template>
    <template v-else-if="name === 'download'">
      <path d="M12 4v11" />
      <path d="M7.5 11.5 12 16l4.5-4.5" />
      <path d="M5 19h14" />
    </template>
    <template v-else-if="name === 'arrow'">
      <path d="M5 12h12" />
      <path d="M13 7l5 5-5 5" />
    </template>
    <template v-else-if="name === 'undo'">
      <path d="M8 8H4v4" />
      <path d="M4.6 12A8 8 0 1 0 6 7.2" />
    </template>
    <template v-else-if="name === 'redo'">
      <path d="M16 8h4v4" />
      <path d="M19.4 12A8 8 0 1 1 18 7.2" />
    </template>
    <template v-else-if="name === 'eye'">
      <path d="M3 12s3.4-6 9-6 9 6 9 6-3.4 6-9 6-9-6-9-6Z" />
      <circle cx="12" cy="12" r="2.2" />
    </template>
    <template v-else-if="name === 'invert'">
      <circle cx="12" cy="12" r="8" />
      <path d="M12 4v16" />
      <path d="M12 4a8 8 0 0 1 0 16" fill="currentColor" />
    </template>
    <template v-else-if="name === 'help'">
      <circle cx="12" cy="12" r="8" />
      <path d="M9.6 9.4a2.4 2.4 0 1 1 3.3 2.2c-.8.4-1.1 1-.1 2" />
      <circle cx="12" cy="16.6" r=".8" fill="currentColor" stroke="none" />
    </template>
    <template v-else-if="name === 'thumbtack'">
      <path d="M15 4.5 19.5 9l-2 1-3.2 5.2-2.3-2.3L7 16.2 4.8 14l3.3-5.1L7 6.8 8 4.8 15 4.5Z" />
      <path d="M10.2 14.6 7 20" />
    </template>
    <template v-else-if="name === 'layers'">
      <path d="M12 4 4 8.5 12 13l8-4.5L12 4Z" />
      <path d="M4 12.2 12 16.8 20 12.2" />
      <path d="M4 16 12 20.6 20 16" />
    </template>
    <template v-else-if="name === 'sliders'">
      <path d="M4 7h16M4 17h16" />
      <circle cx="9" cy="7" r="2" />
      <circle cx="15" cy="17" r="2" />
    </template>
    <template v-else-if="name === 'globe'">
      <circle cx="12" cy="12" r="8" />
      <path d="M4 12h16" />
      <path d="M12 4c2.4 2.4 3.6 5.2 3.6 8s-1.2 5.6-3.6 8c-2.4-2.4-3.6-5.2-3.6-8S9.6 6.4 12 4Z" />
    </template>
    <template v-else-if="name === 'bulb'">
      <path d="M9 14.2A5 5 0 1 1 15 14.2C13.8 15.4 13 16.4 13 18h-2c0-1.6-.8-2.6-2-3.8Z" />
      <path d="M10 20h4" />
    </template>
  </svg>
</template>

<style scoped>
.nai-icon { display: block; flex-shrink: 0; }
</style>
