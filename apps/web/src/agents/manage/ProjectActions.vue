<script setup lang="ts">
import { computed, nextTick, shallowRef, useTemplateRef } from 'vue';
import { VButton, VInputText } from '@ui/kit';
import { usePacer } from '../pacer';
import ActionMenu from './ActionMenu.vue';
import OrphansDialog from './OrphansDialog.vue';
import { useAction } from '../useAction';
import { addProject, removeProject, renameProject } from './workspaces';

const MISSING_FOLDER = /не существует|не найден|not exist|no such|not found/i;

const props = defineProps<{ projectId?: string; workspaceId?: string }>();
const emit = defineEmits<{ resumed: [agent: string] }>();

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
      label="Действия проекта"
      remove-label="Убрать из pacer"
      extra-label="Старые разговоры"
      extra-testid="project-menu-orphans"
      @rename="openRename"
      @remove="openRemove"
      @extra="orphansOpen = true"
    />
    <VButton v-else label="Добавить проект" size="small" text severity="secondary" data-testid="project-add" @click="openAdd" />
    <form v-if="mode === 'adding'" class="inline" @submit.prevent="submitAdd(false)">
      <VInputText ref="pathInput" v-model="path" size="small" placeholder="Путь к папке" aria-label="Путь к папке проекта" data-testid="project-path-input" @keydown.esc="cancel" />
      <VInputText v-model="name" size="small" placeholder="Название (необязательно)" aria-label="Название проекта" data-testid="project-name-input" @keydown.esc="cancel" />
      <VButton type="submit" label="Добавить" size="small" :disabled="!trimmedPath" :loading="busy" data-testid="project-add-submit" />
      <VButton v-if="canCreateMissing" label="Создать папку" size="small" severity="secondary" :disabled="busy" data-testid="project-create-missing" @click="submitAdd(true)" />
      <VButton label="Отмена" size="small" text severity="secondary" @click="cancel" />
    </form>
    <form v-else-if="mode === 'renaming'" class="inline" @submit.prevent="submitRename">
      <VInputText ref="nameInput" v-model="name" size="small" placeholder="Имя папки" aria-label="Название проекта" data-testid="project-name-input" @keydown.esc="cancel" />
      <span class="hint">Пустое название вернёт имя папки</span>
      <VButton type="submit" label="Сохранить" size="small" :loading="busy" data-testid="project-rename-submit" />
      <VButton label="Отмена" size="small" text severity="secondary" @click="cancel" />
    </form>
    <div v-else-if="mode === 'removing'" class="inline" role="alertdialog" aria-label="Подтверждение">
      <span class="hint">Убрать «{{ current?.name }}» из pacer? Папка на диске не будет затронута.</span>
      <VButton label="Убрать" size="small" severity="danger" :loading="busy" data-testid="confirm-remove" @click="confirmRemove" />
      <VButton label="Отмена" size="small" text severity="secondary" @click="cancel" />
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
