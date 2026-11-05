<script setup lang="ts">
import { useTemplateRef } from 'vue';
import { VButton, VPopover } from '@ui/kit';
import { t } from '../../i18n';

defineProps<{ testid: string; label: string; removeLabel: string; extraLabel?: string; extraTestid?: string }>();
const emit = defineEmits<{ rename: []; remove: []; extra: [] }>();

const popover = useTemplateRef<InstanceType<typeof VPopover>>('popover');

function choose(action: 'rename' | 'remove' | 'extra') {
  popover.value?.hide();
  if (action === 'rename') emit('rename');
  else if (action === 'remove') emit('remove');
  else emit('extra');
}
</script>

<template>
  <VButton label="⋯" size="small" text severity="secondary" :aria-label="label" aria-haspopup="menu" :data-testid="testid" @click="popover?.toggle($event)" />
  <VPopover ref="popover">
    <div class="menu" role="menu">
      <button class="item" type="button" role="menuitem" data-testid="rename" @click="choose('rename')">{{ t('common.rename') }}</button>
      <button v-if="extraLabel" class="item" type="button" role="menuitem" :data-testid="extraTestid" @click="choose('extra')">{{ extraLabel }}</button>
      <button class="item danger" type="button" role="menuitem" data-testid="remove" @click="choose('remove')">{{ removeLabel }}</button>
    </div>
  </VPopover>
</template>

<style scoped>
.menu {
  display: flex;
  flex-direction: column;
  min-width: 160px;
}

.item {
  padding: var(--vr-space-2) var(--vr-space-3);
  font: inherit;
  text-align: left;
  color: var(--vr-color-text);
  background: none;
  border: 0;
  border-radius: var(--vr-radius-sm);
}

.item:hover,
.item:focus-visible {
  background: var(--vr-color-nav-hover);
}

.danger {
  color: var(--vr-color-danger);
}
</style>
