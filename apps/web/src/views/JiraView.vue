<script setup lang="ts">
import { computed } from 'vue';
import { Ticket } from 'lucide-vue-next';
import JiraIssues from '../atlassian/JiraIssues.vue';
import JiraIssueView from '../atlassian/JiraIssueView.vue';
import { t } from '../i18n';
import { withViewTransition } from '../motion';
import { view } from '../nav';
import ConnectorPage from './ConnectorPage.vue';

const KINDS = ['jira'] as const;

const open = computed(() => (view.value.kind === 'jira' ? view.value.open : undefined));

function select(target: { connector: string; key: string }) {
  void withViewTransition(() => {
    view.value = { kind: 'jira', open: target };
  });
}
</script>

<template>
  <ConnectorPage
    :kinds="KINDS"
    title="Jira"
    :icon="Ticket"
    :empty-text="t('jira.view.empty')"
    :placeholder="t('jira.view.placeholder')"
    test-id="jira-view"
    storage-key="veyra.jira"
    picker
    :preferred="open?.connector"
    :selected="open"
  >
    <template #list="{ connector }">
      <JiraIssues :key="connector" :connector="connector" @open="select" />
    </template>
    <template #detail="{ connector }">
      <JiraIssueView v-if="open" :key="`${connector}/${open.key}`" :connector="connector" :issue-key="open.key" @open="select" />
    </template>
  </ConnectorPage>
</template>
