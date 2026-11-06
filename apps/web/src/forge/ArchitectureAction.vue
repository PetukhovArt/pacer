<script setup lang="ts">
import { computed } from 'vue';
import { Network } from 'lucide-vue-next';
import { VButton } from '@ui/kit';
import { t } from '../i18n';
import { useDock } from '../workbench/dock';
import { numberLabel } from './format';
import { useArchitecture } from './stratum';

type Item = { connector: string; project: string; number: number; target_branch: string; url: string };

const props = withDefaults(defineProps<{ item: Item; variant?: 'button' | 'icon' }>(), { variant: 'button' });

const dock = useDock();
const { availability } = useArchitecture(() => props.item);
const shown = computed(() => (props.variant === 'icon' ? availability.value.status === 'ready' : availability.value.status !== 'hidden'));
const reason = computed(() => {
  const state = availability.value;
  if (state.status === 'disabled') return state.reason;
  if (state.status === 'checking') return t('forge.stratum.checking');
  return t('forge.stratum.title');
});

function open() {
  const state = availability.value;
  if (state.status === 'ready') dock.openArchitecture(state.params, numberLabel(props.item));
}
</script>

<template>
  <template v-if="shown && variant === 'button'">
    <VButton size="small" severity="secondary" :label="t('forge.stratum.action')" :disabled="availability.status !== 'ready'" :title="reason" data-testid="cr-architecture" @click="open">
      <template #icon><Network :size="14" /></template>
    </VButton>
    <span v-if="availability.status === 'disabled'" class="why" :title="reason" data-testid="cr-architecture-why">{{ reason }}</span>
  </template>
  <button v-else-if="shown" class="icon" type="button" :title="t('forge.stratum.title')" :aria-label="t('forge.stratum.action')" data-testid="mr-architecture" @click.stop="open"><Network :size="14" /></button>
</template>

<style scoped>
.icon {
  display: inline-flex;
  flex: none;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  padding: 0;
  color: var(--vr-color-text-muted);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
}

.why {
  flex: 0 1 auto;
  min-width: 0;
  max-width: 240px;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.icon:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}
</style>
