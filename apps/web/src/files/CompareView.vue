<script setup lang="ts">
import { onBeforeUnmount, onMounted, useTemplateRef } from 'vue';
import { MergeView } from '@codemirror/merge';
import { EditorState } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import { t } from '../i18n';
import { baseExtensions } from './editor';

const props = defineProps<{ disk: string; mine: string }>();

const host = useTemplateRef<HTMLElement>('host');
let merge: MergeView | undefined;

onMounted(() => {
  const parent = host.value;
  if (!parent) return;
  const extensions = [...baseExtensions, EditorState.readOnly.of(true), EditorView.editable.of(false)];
  merge = new MergeView({
    a: { doc: props.disk, extensions },
    b: { doc: props.mine, extensions },
    parent,
    highlightChanges: true,
    gutter: true,
  });
});

onBeforeUnmount(() => merge?.destroy());
</script>

<template>
  <div class="compare" data-testid="markdown-compare">
    <div class="heads">
      <span>{{ t('files.editor.compare.disk') }}</span>
      <span>{{ t('files.editor.compare.mine') }}</span>
    </div>
    <div ref="host" class="host" />
  </div>
</template>

<style scoped>
.compare {
  display: flex;
  flex-direction: column;
  min-height: 0;
  background: var(--vr-color-surface);
}

.heads {
  display: grid;
  flex: none;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
  border-bottom: 1px solid var(--vr-color-border);
}

.heads span {
  padding: var(--vr-space-1) var(--vr-space-3);
}

.host {
  flex: 1;
  min-height: 0;
  overflow: auto;
}

.host :deep(.cm-mergeView) {
  height: auto;
}

.host :deep(.cm-mergeViewEditors) {
  align-items: flex-start;
}

.host :deep(.cm-mergeViewEditor) {
  min-width: 0;
}

.host :deep(.cm-scroller) {
  overflow: visible;
}

.host :deep(.cm-editor) {
  height: auto;
}

.host :deep(.cm-changedLine),
.host :deep(.cm-insertedLine),
.host :deep(.cm-inlineChangedLine) {
  background: color-mix(in srgb, var(--vr-hl-add) 14%, transparent);
}

.host :deep(.cm-deletedLine),
.host :deep(.cm-deletedChunk),
.host :deep(.cm-deletedChunk .cm-line) {
  background: color-mix(in srgb, var(--vr-hl-del) 14%, transparent);
}

.host :deep(.cm-changedText),
.host :deep(.cm-insertedLine .cm-changedText) {
  background: color-mix(in srgb, var(--vr-hl-add) 32%, transparent);
}

.host :deep(.cm-deletedLine .cm-changedText),
.host :deep(.cm-deletedChunk .cm-changedText) {
  background: color-mix(in srgb, var(--vr-hl-del) 32%, transparent);
}

.host :deep(.cm-changedLineGutter),
.host :deep(.cm-insertedLineGutter) {
  background: var(--vr-hl-add);
}

.host :deep(.cm-deletedLineGutter) {
  background: var(--vr-hl-del);
}

.host :deep(.cm-collapsedLines) {
  color: var(--vr-color-text-muted);
  background: var(--vr-color-surface-muted);
}
</style>
