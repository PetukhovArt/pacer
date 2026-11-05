<script setup lang="ts">
import { computed } from 'vue';
import { Ticket } from 'lucide-vue-next';
import JiraIssues from '../atlassian/JiraIssues.vue';
import JiraIssueView from '../atlassian/JiraIssueView.vue';
import { view } from '../nav';
import ConnectorPage from './ConnectorPage.vue';

const KINDS = ['jira'] as const;

const open = computed(() => (view.value.kind === 'jira' ? view.value.open : undefined));

function select(target: { connector: string; key: string }) {
  view.value = { kind: 'jira', open: target };
}
</script>

<template>
  <ConnectorPage
    :kinds="KINDS"
    title="Jira"
    :icon="Ticket"
    empty-text="Подключите Jira, чтобы видеть здесь задачи"
    placeholder="Выберите задачу слева"
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
