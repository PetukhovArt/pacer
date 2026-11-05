<script setup lang="ts">
import { computed, nextTick, shallowRef, useTemplateRef } from 'vue';
import { VButton, VInputText } from '@ui/kit';
import { t } from '../../i18n';
import { usePacer, worktreesOfProject } from '../pacer';
import ActionMenu from './ActionMenu.vue';
import OrphansDialog from './OrphansDialog.vue';
import { useAction } from '../useAction';
import { addProject, removeProject, renameProject } from './workspaces';

const MISSING_FOLDER = /не существует|не найден|not exist|no such|not found/i;

const props = defineProps<{ projectId?: string; workspaceId?: string }>();
const emit = defineEmits<{ resumed: [agent: string]; create: [worktree: string] }>();

type Mode = 'idle' | 'adding' | 'renaming' | 'removing';

const pacer = usePacer();
const mode = shallowRef<Mode>('idle');
const path = shallowRef('');
const name = shallowRef('');
const refusedPath = shallowRef<string | null>(null);
const orphansOpen = shallowRef(false);
const { error, busy, run, clear } = useAction();
const pathInput = useTemplateRef<{ $el: HTMLInputElement }>('pathInput');
const nameInput = useTemplateRef<{ $el: HTMLInputElement }>('nameInput');

const current = computed(() => (props.projectId ? pacer?.entities.value.projects.get(props.projectId) : undefined));
const mainWorktree = computed(() => {
  const entities = pacer?.entities.value;
  if (!entities || !props.projectId) return undefined;
  const worktrees = worktreesOfProject(entities, props.projectId);
  return worktrees.find((worktree) => worktree.is_main) ?? worktrees[0];
});

function createInMain() {
  const worktree = mainWorktree.value;
  if (worktree) emit('create', worktree.id);
}

const trimmedPath = computed(() => path.value.trim());
const canCreateMissing = computed(() => error.value !== null && refusedPath.value === trimmedPath.value && MISSING_FOLDER.test(error.value));

async function openAdd() {
  clear();
  path.value = '';
  name.value = '';
  refusedPath.value = null;
  mode.value = 'adding';
  await nextTick();
  pathInput.value?.$el.focus();
}

async function openRename() {
  clear();
  name.value = current.value?.name ?? '';
  mode.value = 'renaming';
  await nextTick();
  nameInput.value?.$el.focus();
}

function openRemove() {
  clear();
  mode.value = 'removing';
}

function cancel() {
  clear();
  mode.value = 'idle';
}

async function submitAdd(createMissing: boolean) {
  const target = trimmedPath.value;
  const workspace = props.workspaceId;
  if (!target || !workspace || busy.value) return;
  refusedPath.value = target;
  const ok = await run(() => addProject(workspace, { path: target, name: name.value.trim() || null, createMissing }));
  if (ok) mode.value = 'idle';
}

async function submitRename() {
  const id = props.projectId;
  if (!id || busy.value) return;
  const ok = await run(() => renameProject(id, name.value.trim()));
  if (ok) mode.value = 'idle';
}

async function confirmRemove() {
  const id = props.projectId;
  if (!id) return;
  await run(() => removeProject(id));
  mode.value = 'idle';
}
</script>

<template>
  <div v-if="pacer" class="actions">
    <ActionMenu
      v-if="projectId"
      testid="project-menu"
      :label="t('agents.project.actions')"
      :remove-label="t('agents.project.removeFromPacer')"
      :extra-label="t('agents.project.oldConversations')"
      extra-testid="project-menu-orphans"
      :create-label="mainWorktree ? t('agents.project.newSession') : undefined"
      @create="createInMain"
      @rename="openRename"
      @remove="openRemove"
      @extra="orphansOpen = true"
    />
    <VButton v-else :label="t('agents.project.add')" size="small" text severity="secondary" data-testid="project-add" @click="openAdd" />
    <form v-if="mode === 'adding'" class="inline" @submit.prevent="submitAdd(false)">
      <VInputText ref="pathInput" v-model="path" size="small" :placeholder="t('agents.project.path')" :aria-label="t('agents.project.pathAria')" data-testid="project-path-input" @keydown.esc="cancel" />
      <VInputText v-model="name" size="small" :placeholder="t('agents.project.namePlaceholder')" :aria-label="t('agents.project.nameAria')" data-testid="project-name-input" @keydown.esc="cancel" />
      <VButton type="submit" :label="t('agents.project.addSubmit')" size="small" :disabled="!trimmedPath" :loading="busy" data-testid="project-add-submit" />
      <VButton v-if="canCreateMissing" :label="t('agents.project.createFolder')" size="small" severity="secondary" :disabled="busy" data-testid="project-create-missing" @click="submitAdd(true)" />
      <VButton :label="t('common.cancel')" size="small" text severity="secondary" @click="cancel" />
    </form>
    <form v-else-if="mode === 'renaming'" class="inline" @submit.prevent="submitRename">
      <VInputText ref="nameInput" v-model="name" size="small" :placeholder="t('agents.project.folderName')" :aria-label="t('agents.project.nameAria')" data-testid="project-name-input" @keydown.esc="cancel" />
      <span class="hint">{{ t('agents.project.renameHint') }}</span>
      <VButton type="submit" :label="t('common.save')" size="small" :loading="busy" data-testid="project-rename-submit" />
      <VButton :label="t('common.cancel')" size="small" text severity="secondary" @click="cancel" />
    </form>
    <div v-else-if="mode === 'removing'" class="inline" role="alertdialog" :aria-label="t('agents.project.confirm')">
      <span class="hint">{{ t('agents.project.removeQuestion', { name: current?.name ?? '' }) }}</span>
      <VButton :label="t('common.remove')" size="small" severity="danger" :loading="busy" data-testid="confirm-remove" @click="confirmRemove" />
      <VButton :label="t('common.cancel')" size="small" text severity="secondary" @click="cancel" />
    </div>
    <p v-if="error" class="error" role="alert" data-testid="project-error">{{ error }}</p>
    <OrphansDialog v-if="orphansOpen && projectId" :project="projectId" @resumed="emit('resumed', $event)" @close="orphansOpen = false" />
  </div>
</template>

<style scoped>
.actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: flex-end;
  gap: var(--vr-space-1);
}

.actions:has(.inline) {
  flex: 1 0 100%;
}

.inline {
  display: flex;
  flex: 1 0 100%;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--vr-space-1);
}

.inline :deep(input) {
  flex: 1 0 100%;
  min-width: 0;
}

.hint {
  flex: 1 0 100%;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-text-muted);
}

.error {
  flex: 1 0 100%;
  margin: 0;
  font-size: var(--vr-font-size-xs);
  color: var(--vr-color-danger);
}
</style>
