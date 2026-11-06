<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue';
import Avatar from '../Avatar.vue';
import type { Person } from './api';

const props = defineProps<{ person: Person; small?: boolean }>();

const failed = shallowRef(false);
watch(
  () => props.person.avatar_url,
  () => (failed.value = false),
);

const picture = computed(() => (failed.value ? null : props.person.avatar_url));
const label = computed(() => props.person.name || props.person.username);
</script>

<template>
  <span class="person" :title="label">
    <Avatar :id="person.username || label" :name="label" :small="small" :picture="picture" @error="failed = true" />
  </span>
</template>

<style scoped>
.person {
  display: inline-flex;
  flex: none;
}
</style>
