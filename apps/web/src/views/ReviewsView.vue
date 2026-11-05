<script setup lang="ts">
import { computed } from 'vue';
import { GitPullRequest } from 'lucide-vue-next';
import ChangeRequestView from '../forge/ChangeRequestView.vue';
import ForgeInbox from '../forge/ForgeInbox.vue';
import { t } from '../i18n';
import { withViewTransition } from '../motion';
import { openReview, view } from '../nav';
import ConnectorPage from './ConnectorPage.vue';

const KINDS = ['github', 'gitlab'] as const;

const open = computed(() => (view.value.kind === 'reviews' ? view.value.open : undefined));
</script>

<template>
  <ConnectorPage
    :kinds="KINDS"
    :title="t('reviews.title')"
    :icon="GitPullRequest"
    :empty-text="t('reviews.empty')"
    :placeholder="t('reviews.placeholder')"
    test-id="reviews-view"
    storage-key="veyra.reviews"
    :preferred="open?.connector"
    :selected="open"
  >
    <template #list>
      <ForgeInbox :selected="open" @open="(ref) => withViewTransition(() => openReview(ref))" />
    </template>
    <template #detail>
      <ChangeRequestView v-if="open" :key="`${open.connector}/${open.project}/${open.number}`" v-bind="open" />
    </template>
  </ConnectorPage>
</template>
