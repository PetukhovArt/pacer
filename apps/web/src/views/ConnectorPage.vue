<script setup lang="ts">
import { computed, onMounted, shallowRef, watch, type Component } from 'vue';
import { VButton, VSelect } from '@ui/kit';
import { useConnectors, type ConnectorKind } from '../connectors/api';
import { t } from '../i18n';
import { openSettings } from '../nav';
import SidePane from '../SidePane.vue';

const props = defineProps<{
  kinds: readonly ConnectorKind[];
  title: string;
  icon: Component;
  emptyText: string;
  testId: string;
  storageKey: string;
  preferred?: string;
  picker?: boolean;
  selected?: { connector: string };
  placeholder: string;
}>();

defineSlots<{
  list(props: { connector: string }): unknown;
  detail(props: { connector: string }): unknown;
}>();

const { connectors, loading, list } = useConnectors();

const settled = shallowRef(connectors.value.length > 0);
watch(loading, (busy) => {
  if (!busy) settled.value = true;
});
onMounted(async () => {
  if (settled.value) return;
  await list();
  if (!loading.value) settled.value = true;
});

const candidates = computed(() => connectors.value.filter((c) => props.kinds.includes(c.kind)));
const lastKey = computed(() => `${props.storageKey}.connector`);
const picked = shallowRef(props.picker ? localStorage.getItem(lastKey.value) : null);

const current = computed(() => {
  const id = props.preferred && candidates.value.some((c) => c.id === props.preferred) ? props.preferred : picked.value;
  return candidates.value.find((c) => c.id === id) ?? candidates.value[0] ?? null;
});

watch(
  () => props.preferred,
  (id) => {
    if (id) pick(id);
  },
  { immediate: true },
);

function pick(id: string) {
  picked.value = id;
  if (props.picker) localStorage.setItem(lastKey.value, id);
}

const options = computed(() => candidates.value.map((c) => ({ id: c.id, label: c.label })));
const selectedId = computed({
  get: () => current.value?.id ?? null,
  set: (id: string | null) => {
    if (id) pick(id);
  },
});
</script>

<template>
  <section class="page" :data-testid="testId" :aria-label="title">
    <header class="header">
      <h1>
        <component :is="icon" :size="18" />
        {{ title }}
      </h1>
      <VSelect
        v-if="picker && candidates.length > 1"
        v-model="selectedId"
        class="picker"
        :options="options"
        option-label="label"
        option-value="id"
        :aria-label="t('reviews.connectorPage.connection')"
        :data-testid="`${testId}-connector`"
      />
    </header>

    <p v-if="!settled" class="state" data-testid="connector-page-loading">{{ t('common.loading') }}</p>
    <div v-else-if="!current" class="state empty" data-testid="connector-required">
      <component :is="icon" :size="32" />
      <p>{{ emptyText }}</p>
      <VButton :label="t('reviews.connectorPage.connect')" size="small" data-testid="connector-required-connect" @click="openSettings('connectors')" />
    </div>
    <div v-else class="split">
      <SidePane :storage-key="`${storageKey}.width`" :default-width="380" :label="t('reviews.connectorPage.listWidth')" :handle-testid="`${testId}-resize-handle`">
        <slot name="list" :connector="current.id" />
      </SidePane>
      <div class="detail vt-detail">
        <slot v-if="selected && (!picker || selected.connector === current.id)" name="detail" :connector="current.id" />
        <p v-else class="placeholder" data-testid="connector-page-placeholder">{{ placeholder }}</p>
      </div>
    </div>
  </section>
</template>

<style scoped>
.page {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  color: var(--vr-color-text);
}

.header {
  display: flex;
  flex: none;
  align-items: center;
  gap: var(--vr-space-3);
  padding: var(--vr-space-3) var(--vr-space-6);
  border-bottom: 1px solid var(--vr-color-border);
}

h1 {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  margin: 0 var(--vr-space-4) 0 0;
  font-size: var(--vr-font-size-lg);
  font-weight: 600;
}

.picker {
  min-width: 14rem;
}

.state {
  margin: var(--vr-space-6);
  color: var(--vr-color-text-muted);
}

.state.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--vr-space-3);
  margin: auto;
  text-align: center;
}

.state.empty p {
  margin: 0;
}

.split {
  display: flex;
  flex: 1;
  min-height: 0;
}

.placeholder {
  margin: auto;
  color: var(--vr-color-text-muted);
}

.detail {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
}
</style>
