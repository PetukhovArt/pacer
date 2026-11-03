<script setup lang="ts">
import { computed } from 'vue';

const props = defineProps<{ id: string; name: string; agent?: boolean; small?: boolean }>();

const initials = computed(() =>
  props.name
    .split(/[^\p{L}\p{N}]+/u)
    .filter(Boolean)
    .slice(0, 2)
    .map((part) => part[0]!.toUpperCase())
    .join(''),
);

const tone = computed(() => ([...props.id].reduce((sum, char) => sum + char.charCodeAt(0), 0) % 7) + 1);
</script>

<template>
  <span class="avatar" :class="{ agent, small }" :data-tone="tone" aria-hidden="true">{{ initials }}</span>
</template>

<style scoped>
.avatar {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: var(--vr-size-avatar);
  height: var(--vr-size-avatar);
  border-radius: 50%;
  font-size: var(--vr-font-size-xs);
  font-weight: 600;
  line-height: 1;
  color: var(--vr-palette-ink-0);
  user-select: none;
}

.avatar.agent {
  border-radius: 30%;
}

.avatar.small {
  width: var(--vr-size-avatar-sm);
  height: var(--vr-size-avatar-sm);
  font-size: 9px;
}

.avatar[data-tone='1'] { background: var(--vr-palette-avatar-1); }
.avatar[data-tone='2'] { background: var(--vr-palette-avatar-2); }
.avatar[data-tone='3'] { background: var(--vr-palette-avatar-3); color: var(--vr-palette-ink-900); }
.avatar[data-tone='4'] { background: var(--vr-palette-avatar-4); }
.avatar[data-tone='5'] { background: var(--vr-palette-avatar-5); color: var(--vr-palette-ink-900); }
.avatar[data-tone='6'] { background: var(--vr-palette-avatar-6); }
.avatar[data-tone='7'] { background: var(--vr-palette-avatar-7); }
</style>
