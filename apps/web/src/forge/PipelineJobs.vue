<script setup lang="ts">
import { computed } from 'vue';
import { ExternalLink } from 'lucide-vue-next';
import { openUrl, type Pipeline, type PipelineJob } from '../connectors/api';
import { t } from '../i18n';
import { formatDuration, jobTone } from './format';
import PipelineIcon from './PipelineIcon.vue';

const props = defineProps<{ pipeline: Pipeline }>();

const stages = computed(() => {
  const groups = new Map<string, PipelineJob[]>();
  for (const job of props.pipeline.jobs) {
    const list = groups.get(job.stage);
    if (list) list.push(job);
    else groups.set(job.stage, [job]);
  }
  return [...groups.entries()].map(([name, jobs]) => ({ name, jobs }));
});
</script>

<template>
  <div class="jobs" data-testid="pipeline-jobs">
    <p v-if="!pipeline.jobs.length" class="empty">{{ t('forge.pipelineJobs.empty') }}</p>
    <section v-for="stage in stages" :key="stage.name" class="stage">
      <h4>{{ stage.name }}</h4>
      <a v-for="job in stage.jobs" :key="job.id" class="job" :href="job.url" :title="job.status" data-testid="pipeline-job" @click.prevent="openUrl(job.url)">
        <PipelineIcon :status="jobTone(job.status)" />
        <span class="name">{{ job.name }}</span>
        <span class="status">{{ job.status }}</span>
        <span class="duration">{{ formatDuration(job.duration_s) }}</span>
        <ExternalLink :size="12" class="open" />
      </a>
    </section>
  </div>
</template>

<style scoped>
.jobs {
  display: flex;
  flex-direction: column;
  gap: var(--vr-space-3);
}

.empty {
  margin: 0;
  color: var(--vr-color-text-muted);
}

h4 {
  margin: 0 0 var(--vr-space-1);
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
  font-weight: 600;
  text-transform: uppercase;
}

.job {
  display: flex;
  align-items: center;
  gap: var(--vr-space-2);
  min-height: var(--vr-size-row);
  padding: 0 var(--vr-space-2);
  color: var(--vr-color-text);
  font-size: var(--vr-font-size-sm);
  text-decoration: none;
  border-radius: var(--vr-radius-sm);
  cursor: pointer;
}

.job:hover {
  background: var(--vr-color-nav-hover);
}

.name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  text-overflow: ellipsis;
}

.status,
.duration {
  color: var(--vr-color-text-muted);
  font-size: var(--vr-font-size-xs);
}

.open {
  color: var(--vr-color-text-muted);
  opacity: 0;
}

.job:hover .open {
  opacity: 1;
}
</style>
