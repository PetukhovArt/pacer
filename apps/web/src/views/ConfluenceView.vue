<script setup lang="ts">
import { computed } from 'vue';
import { BookOpen } from 'lucide-vue-next';
import ConfluencePageView from '../atlassian/ConfluencePageView.vue';
import ConfluenceSearch from '../atlassian/ConfluenceSearch.vue';
import { t } from '../i18n';
import { withViewTransition } from '../motion';
import { view } from '../nav';
import ConnectorPage from './ConnectorPage.vue';

const KINDS = ['confluence'] as const;

const open = computed(() => (view.value.kind === 'confluence' ? view.value.open : undefined));

function select(target: { connector: string; id: string }) {
  void withViewTransition(() => {
    view.value = { kind: 'confluence', open: target };
  });
}
</script>

<template>
  <ConnectorPage
    :kinds="KINDS"
    title="Confluence"
    :icon="BookOpen"
    :empty-text="t('confluence.view.empty')"
    :placeholder="t('confluence.view.placeholder')"
    test-id="confluence-view"
    storage-key="veyra.confluence"
    picker
    :preferred="open?.connector"
    :selected="open"
  >
    <template #list="{ connector }">
      <ConfluenceSearch :key="connector" :connector="connector" @open="select" />
    </template>
    <template #detail="{ connector }">
      <ConfluencePageView v-if="open" :key="`${connector}/${open.id}`" :connector="connector" :id="open.id" @open="select" />
    </template>
  </ConnectorPage>
</template>
