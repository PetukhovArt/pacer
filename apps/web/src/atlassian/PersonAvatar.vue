<script setup lang="ts">
import { ref, watch } from 'vue';
import Avatar from '../Avatar.vue';
import type { Person } from '../connectors/api';
import { avatarSrc } from './util';

const props = defineProps<{ connector: string; person: Person; small?: boolean }>();
const picture = ref<string | null>(null);

watch(
  () => [props.connector, props.person.avatar_url] as const,
  async ([connector, url]) => {
    picture.value = null;
    if (!url) return;
    const src = await avatarSrc(connector, url);
    if (props.person.avatar_url === url) picture.value = src;
  },
  { immediate: true },
);
</script>

<template>
  <Avatar :id="person.username || person.name" :name="person.name" :picture="picture" :small="small" :title="person.name" />
</template>
