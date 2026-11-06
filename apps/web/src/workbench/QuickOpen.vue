<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue';
import { listWorktreeMarkdown, type MarkdownEntry } from '../agents/proto/markdownFiles';
import QuickSwitcher from '../agents/proto/QuickSwitcher.vue';
import { isDesktop } from '../bridge';
import { openCleanup } from '../cleanup/state';
import { t } from '../i18n';
import { useDock } from './dock';
import { quickItems, type WorkNode } from './tree';
import { useWorkTree } from './useWorkTree';

const dock = useDock();
const { all } = useWorkTree();
const { switching, activeWorktree } = dock;
const entries = computed(() => (switching.value ? quickItems(all.value) : []));
const files = shallowRef<MarkdownEntry[]>([]);
const actions = computed(() => (isDesktop ? [{ key: 'cleanup', label: t('cleanup.action') }] : []));

watch(switching, async (on) => {
  files.value = [];
  const worktree = activeWorktree.value;
  if (!on || !worktree) return;
  try {
    const found = await listWorktreeMarkdown(worktree.path);
    if (switching.value) files.value = found;
  } catch {
    files.value = [];
  }
});

function pick(node: WorkNode) {
  switching.value = false;
  dock.openLeaf(node);
}

function pickAction(key: string) {
  switching.value = false;
  if (key === 'cleanup') openCleanup();
}

function pickFile(path: string) {
  switching.value = false;
  dock.openFile(path, { worktreeId: activeWorktree.value?.id });
}
</script>

<template>
  <Transition name="qs">
    <QuickSwitcher v-if="switching" :entries="entries" :files="files" :file-scope="activeWorktree?.branch" :actions="actions" @select="pick" @select-action="pickAction" @select-file="pickFile" @close="switching = false" />
  </Transition>
</template>
