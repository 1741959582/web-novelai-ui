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
    <template v-else-if="name === 'search'">
      <circle cx="11" cy="11" r="6" />
      <path d="m20 20-3.5-3.5" />
    </template>
    <template v-else-if="name === 'star'">
      <path d="m12 3.6 2.05 4.16 4.59.67-3.32 3.24.78 4.57L12 14.1l-4.1 2.14.78-4.57-3.32-3.24 4.59-.67L12 3.6Z" />
    </template>
    <template v-else-if="name === 'anlas'">
      <g transform="translate(2.2 3) scale(1.95)">
        <path
          fill="currentColor"
          stroke="none"
          fill-rule="evenodd"
          d="M4.48874 0H5.51107L7.14867 1.60183L5.45731 8.99994H4.54284L2.85142 1.60156L4.48874 0ZM7.47144 9L10 5.04294V4.39087L8.65736 3.07756L8.24906 3.18423L6.91946 9H7.47144ZM3.08069 9L1.7543 3.19828L1.33187 3.08792L0 4.39069V5.04271L2.52871 9H3.08069Z"
        />
      </g>
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
    <template v-else-if="name === 'upscale'">
      <path d="M9 4H4v5M15 4h5v5M9 20H4v-5M15 20h5v-5" />
      <path d="M4 4l6 6M20 4l-6 6M4 20l6-6M20 20l-6-6" />
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
    <template v-else-if="name === 'lock'">
      <rect x="6" y="10" width="12" height="9" rx="1.6" />
      <path d="M8.5 10V7.5a3.5 3.5 0 0 1 7 0V10" />
    </template>
    <template v-else-if="name === 'unlock'">
      <rect x="6" y="10" width="12" height="9" rx="1.6" />
      <path d="M8.5 10V7.5a3.5 3.5 0 0 1 6.2-2.2" />
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
    <template v-else-if="name === 'female'">
      <circle cx="12" cy="8" r="3.2" />
      <path d="M12 11.2v8M9.2 16.2h5.6" />
    </template>
    <template v-else-if="name === 'male'">
      <circle cx="10" cy="14" r="3.4" />
      <path d="M12.6 11.4 18 6M14.2 6H18v3.8" />
    </template>
    <template v-else-if="name === 'user'">
      <circle cx="12" cy="9" r="3.1" />
      <path d="M6.2 18.6c1.3-2.5 3.3-3.6 5.8-3.6s4.5 1.1 5.8 3.6" />
    </template>
    <template v-else-if="name === 'pen'">
      <path d="M14.2 4.8 19.2 9.8 8.6 20.4H3.6v-5L14.2 4.8Z" />
      <path d="M12.2 6.8 17.2 11.8" />
    </template>
    <template v-else-if="name === 'file'">
      <path d="M7 3.5h7l4 4V20a1 1 0 0 1-1 1H7a1 1 0 0 1-1-1V4.5a1 1 0 0 1 1-1Z" />
      <path d="M14 3.5V8h4.2M8.5 12.5h7M8.5 16h5" />
    </template>
    <template v-else-if="name === 'chart'">
      <path d="M5 19V5M5 19h14" />
      <path d="M8.5 15.5v-3.2M12 15.5V8.5M15.5 15.5v-5" />
    </template>
    <template v-else-if="name === 'chevrons-right'">
      <path d="M6.5 7.5 11.5 12l-5 4.5M12.5 7.5 17.5 12l-5 4.5" />
    </template>
    <template v-else-if="name === 'chevrons-left'">
      <path d="M17.5 7.5 12.5 12l5 4.5M11.5 7.5 6.5 12l5 4.5" />
    </template>
    <template v-else-if="name === 'gear'">
      <path d="M12 15.2a3.2 3.2 0 1 0 0-6.4 3.2 3.2 0 0 0 0 6.4Z" />
      <path d="M19.4 13.1a1.7 1.7 0 0 0 .34 1.84l.05.06a2 2 0 1 1-2.83 2.83l-.06-.05a1.7 1.7 0 0 0-1.84-.34 1.7 1.7 0 0 0-1.03 1.55V19.2a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.11-1.56 1.7 1.7 0 0 0-1.84.34l-.06.05a2 2 0 1 1-2.83-2.83l.05-.06a1.7 1.7 0 0 0 .34-1.84 1.7 1.7 0 0 0-1.55-1.03H3.8a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.56-1.11 1.7 1.7 0 0 0-.34-1.84l-.05-.06a2 2 0 1 1 2.83-2.83l.06.05a1.7 1.7 0 0 0 1.84.34H10a1.7 1.7 0 0 0 1.03-1.55V3.8a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1.03 1.56 1.7 1.7 0 0 0 1.84-.34l.06-.05a2 2 0 1 1 2.83 2.83l-.05.06a1.7 1.7 0 0 0-.34 1.84V10c.26.7.9 1.2 1.63 1.26h.1a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.63 1.04Z" />
    </template>
    <template v-else-if="name === 'film'">
      <rect x="4" y="5" width="16" height="14" rx="2" />
      <path d="M8 5v14M16 5v14M4 9.2h4M4 14.8h4M16 9.2h4M16 14.8h4" />
    </template>
  </svg>
</template>

<style scoped>
.nai-icon { display: block; flex-shrink: 0; }
</style>
