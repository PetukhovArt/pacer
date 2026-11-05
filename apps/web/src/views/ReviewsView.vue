<script setup lang="ts">
import { computed } from 'vue';
import { GitPullRequest } from 'lucide-vue-next';
import ChangeRequestView from '../forge/ChangeRequestView.vue';
import ForgeInbox from '../forge/ForgeInbox.vue';
import { openReview, view } from '../nav';
import ConnectorPage from './ConnectorPage.vue';

const KINDS = ['github', 'gitlab'] as const;

const open = computed(() => (view.value.kind === 'reviews' ? view.value.open : undefined));
</script>

<template>
  <ConnectorPage
    :kinds="KINDS"
    title="Ревью"
    :icon="GitPullRequest"
    empty-text="Подключите GitHub или GitLab, чтобы видеть здесь MR и PR"
    placeholder="Выберите MR/PR слева"
    test-id="reviews-view"
    storage-key="veyra.reviews"
    :preferred="open?.connector"
    :selected="open"
  >
    <template #list>
      <ForgeInbox @open="openReview" />
    </template>
    <template #detail>
      <ChangeRequestView v-if="open" :key="`${open.connector}/${open.project}/${open.number}`" v-bind="open" />
    </template>
  </ConnectorPage>
</template>
