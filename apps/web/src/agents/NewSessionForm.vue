<script setup lang="ts">
import { computed, shallowRef } from 'vue';
import { VButton, VDialog, VInputText, VMessage, VSelect, VTextarea } from '@ui/kit';
import { t } from '../i18n';
import { useSettings } from '../settings/store';
import { createAgent,createTerminal, entitySessionRef, type AgentKind, type SessionRef, type WorktreeNode } from './berth';
import { useAction } from './useAction';

const props = defineProps<{ worktree: WorktreeNode }>();
const emit = defineEmits<{ close: []; created: [session: SessionRef] }>();

type Kind = AgentKind | 'terminal';
const KINDS = computed<{ label: string; value: Kind }[]>(() => [
  { label: 'Claude', value: 'claude' },
  { label: 'Codex', value: 'codex' },
  { label: 'Cursor', value: 'cursor' },
  { label: t('agents.newSession.terminal'), value: 'terminal' },
]);

function generatedName(kind: Kind): string {
  const taken = new Set(props.worktree.sessions.map((session) => session.name));
  let n = 1;
  while (taken.has(`${kind}-${n}`)) n++;
  return `${kind}-${n}`;
}

const { values } = useSettings();
const initialKind = KINDS.value.find((option) => option.value === values.value['sessions.default_kind'])?.value ?? 'claude';
const kind = shallowRef<Kind>(initialKind);
const name = shallowRef(generatedName(initialKind));
const model = shallowRef(values.value['sessions.default_model']);
const effort = shallowRef(values.value['sessions.default_effort']);
const prompt = shallowRef('');
const action = useAction();

const defaultName = computed(() => generatedName(kind.value));

function pickKind(value: unknown) {
  const next = KINDS.value.find((option) => option.value === value);
  if (!next) return;
  if (name.value.trim() === defaultName.value || !name.value.trim()) name.value = generatedName(next.value);
  kind.value = next.value;
}

const optional = (text: string) => text.trim() || null;

async function submit() {
  const chosen = kind.value;
  const finalName = name.value.trim() || defaultName.value;
  await action.run(async () => {
    const id =
      chosen === 'terminal'
        ? await createTerminal(props.worktree.id, finalName)
        : await createAgent({
            worktree: props.worktree.id,
            name: finalName,
            kind: chosen,
            model: optional(model.value),
            effort: optional(effort.value),
            autoTitle: finalName === defaultName.value,
            startingPrompt: optional(prompt.value),
          });
    const created = entitySessionRef(id);
    if (created) emit('created', created);
    else emit('close');
  });
}

function onVisible(visible: boolean) {
  if (!visible) emit('close');
}
</script>

<template>
  <VDialog :visible="true" modal :header="t('agents.newSession.title', { branch: worktree.branch })" :style="{ width: '420px' }" :draggable="false" @update:visible="onVisible">
    <form class="form" data-testid="new-session-form" @submit.prevent="submit">
      <label class="field">
        <span>{{ t('agents.newSession.type') }}</span>
        <VSelect :model-value="kind" :options="KINDS" option-label="label" option-value="value" size="small" :aria-label="t('agents.newSession.typeAria')" data-testid="new-session-kind" @update:model-value="pickKind" />
      </label>
      <label class="field">
        <span>{{ t('agents.newSession.name') }}</span>
        <VInputText v-model="name" size="small" :placeholder="defaultName" data-testid="new-session-name" />
      </label>
      <template v-if="kind !== 'terminal'">
        <div class="pair">
          <label class="field">
            <span>{{ t('agents.newSession.model') }}</span>
            <VInputText v-model="model" size="small" :placeholder="t('agents.newSession.byDefault')" data-testid="new-session-model" />
          </label>
          <label class="field">
            <span>{{ t('agents.newSession.effort') }}</span>
            <VInputText v-model="effort" size="small" :placeholder="t('agents.newSession.byDefault')" data-testid="new-session-effort" />
          </label>
        </div>
        <label class="field">
          <span>{{ t('agents.newSession.prompt') }}</span>
          <VTextarea v-model="prompt" rows="4" auto-resize :placeholder="t('agents.newSession.optional')" data-testid="new-session-prompt" />
        </label>
      </template>
      <VMessage v-if="action.error.value" severity="error" size="small" data-testid="new-session-error">{{ action.error.value }}</VMessage>
      <div class="actions">
        <VButton :label="t('common.cancel')" size="small" text data-testid="new-session-cancel" @click="emit('close')" />
        <VButton type="submit" :label="t('common.create')" size="small" :loading="action.busy.value" data-testid="new-session-submit" />
      </div>
    </form>
  </VDialog>
</template>

<style scoped>
.form {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
}

.field {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: var(--vr-space-1);
  min-width: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.pair {
  display: flex;
  gap: var(--vr-space-3);
}

.actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--vr-space-2);
}
</style>
