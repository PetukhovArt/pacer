<script setup lang="ts">
import { computed, shallowRef } from 'vue';
import { VButton, VDialog, VMessage, VSelect } from '@ui/kit';
import { t } from '../../i18n';
import { useBerth } from '../berth';
import { useAction } from '../useAction';
import { moveAgent } from './orphans';
import { useWorktreeOptions } from './worktrees';

const props = defineProps<{ agent: string }>();
const emit = defineEmits<{ moved: []; close: [] }>();

const berth = useBerth();
const current = computed(() => berth?.entities.value.agents.get(props.agent)?.worktree_id);
const project = computed(() => (current.value ? berth?.entities.value.worktrees.get(current.value)?.project_id : undefined));
const options = useWorktreeOptions(() => project.value, () => current.value);

const picked = shallowRef<string | null>(null);
const { busy, error, run } = useAction();

const target = computed(() => options.value.find((option) => option.id === picked.value)?.id ?? options.value[0]?.id);

function pick(worktree: unknown) {
  if (typeof worktree === 'string') picked.value = worktree;
}

async function submit() {
  const worktree = target.value;
  if (!worktree) return;
  if (await run(() => moveAgent(props.agent, worktree))) emit('moved');
}
</script>

<template>
  <VDialog v-if="berth" :visible="true" modal dismissable-mask :header="t('agents.move.title')" :style="{ width: '28rem', maxWidth: '95vw' }" data-testid="move-agent-dialog" @update:visible="emit('close')">
    <p v-if="!options.length" class="state" role="status">{{ t('agents.move.none') }}</p>
    <form v-else class="form" @submit.prevent="submit">
      <VSelect
        :model-value="target"
        :options="options"
        option-label="label"
        option-value="id"
        :aria-label="t('agents.move.target')"
        data-testid="move-agent-worktree"
        @update:model-value="pick"
      />
      <VMessage v-if="error" severity="error" size="small" data-testid="move-agent-error">{{ error }}</VMessage>
      <div class="actions">
        <VButton type="submit" :label="t('agents.move.submit')" size="small" data-testid="move-agent-submit" :loading="busy" :disabled="!target || busy" />
      </div>
    </form>
  </VDialog>
</template>

<style scoped>
.state {
  margin: 0;
  color: var(--vr-color-text-muted);
}

.form {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
}

.actions {
  display: flex;
  justify-content: flex-end;
}
</style>
