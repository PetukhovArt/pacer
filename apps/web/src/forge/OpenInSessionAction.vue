<script setup lang="ts">
import { computed } from 'vue';
import { LoaderCircle, SquareTerminal } from 'lucide-vue-next';
import { VButton } from '@ui/kit';
import { useBerth } from '../agents/berth';
import { t } from '../i18n';
import { itemId } from './inbox';
import { useOpenInSession, type MrItem } from './openInSession';

const props = withDefaults(defineProps<{ item: MrItem; refs?: readonly string[]; variant?: 'button' | 'icon' }>(), { refs: () => [], variant: 'button' });

const berth = useBerth();
const { open, runs } = useOpenInSession();
const busy = computed(() => runs.value.get(itemId(props.item))?.busy ?? false);
</script>

<template>
  <template v-if="berth">
    <VButton v-if="variant === 'button'" size="small" severity="secondary" :label="t('forge.openSession.action')" :title="t('forge.openSession.title')" :loading="busy" data-testid="cr-open-session" @click="open(item, refs)">
      <template #icon><SquareTerminal :size="14" /></template>
    </VButton>
    <button v-else class="icon" type="button" :title="t('forge.openSession.title')" :aria-label="t('forge.openSession.action')" :disabled="busy" data-testid="mr-open-session" @click.stop="open(item, refs)">
      <LoaderCircle v-if="busy" :size="14" class="spin" />
      <SquareTerminal v-else :size="14" />
    </button>
  </template>
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

.icon:hover {
  color: var(--vr-color-text);
  background: var(--vr-color-nav-hover);
}

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}
</style>
