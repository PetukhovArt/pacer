import { computed, shallowRef, watch } from 'vue';
import { useBerth } from '../agents/berth';
import { hasBackend, invoke, isDesktop } from '../bridge';
import { useConnectors } from '../connectors/api';
import { t } from '../i18n';
import { connectorBase, lookupRemotes, remotes } from '../workbench/remotes';

export type ArchitectureTarget = { connector: string; project: string; number: number; target_branch: string };
export type ArchitectureParams = { connector: string; project: string; number: number; repo: string; target: string };
export type Visualizer = { key: string; url: string; head: string; base: string };
export type Progress = { phase: string; lines: string[] };
export type Availability = { status: 'hidden' } | { status: 'checking' } | { status: 'disabled'; reason: string } | { status: 'ready'; params: ArchitectureParams };

type Probe = { config: boolean; linter: string | null };

const probes = shallowRef<ReadonlyMap<string, Probe | 'failed'>>(new Map());
const asked = new Set<string>();

const architectureProbe = (repo: string, target: string) => invoke<Probe>('stratum_probe', { repo, target });
export const architectureStart = (params: ArchitectureParams) => invoke<Visualizer>('stratum_visualize', { ...params });
export const architectureProgress = (connector: string, project: string, number: number) => invoke<Progress>('stratum_progress', { connector, project, number });
export const architectureRelease = (key: string) => invoke<void>('stratum_release', { key });

const probeKey = (repo: string, target: string) => `${repo}\n${target}`;

function ensureProbe(repo: string, target: string) {
  const key = probeKey(repo, target);
  if (asked.has(key)) return;
  asked.add(key);
  void architectureProbe(repo, target)
    .then((probe) => (probes.value = new Map([...probes.value, [key, probe]])))
    .catch(() => (probes.value = new Map([...probes.value, [key, 'failed']])));
}

export const architecturePanelId = (ref: { connector: string; project: string; number: number }) => `arch:${ref.connector}/${ref.project}/${ref.number}`;

export function useArchitecture(target: () => ArchitectureTarget | undefined) {
  const berth = useBerth();
  const { connectors } = useConnectors();

  const projects = computed(() => [...(berth?.entities.value.projects.values() ?? [])]);
  watch(
    () => projects.value.map((project) => project.repo_path),
    (repos) => lookupRemotes(repos),
    { immediate: true },
  );

  const clone = computed(() => {
    const item = target();
    const connector = item ? connectors.value.find((candidate) => candidate.id === item.connector) : undefined;
    const base = connector ? connectorBase(connector.base_url) : null;
    if (!item || !base) return null;
    const wanted = [base.path, item.project].filter(Boolean).join('/').toLowerCase();
    return projects.value.find((project) => {
      const remote = remotes.value.get(project.repo_path);
      return remote?.host === base.host && remote.path.toLowerCase() === wanted;
    })?.repo_path ?? null;
  });

  watch(
    [clone, () => target()?.target_branch],
    ([repo, branch]) => {
      if (repo && branch && isDesktop) ensureProbe(repo, branch);
    },
    { immediate: true },
  );

  const availability = computed<Availability>(() => {
    const item = target();
    if (!item || !hasBackend) return { status: 'hidden' };
    if (!isDesktop) return { status: 'disabled', reason: t('forge.stratum.remote') };
    const repo = clone.value;
    if (!repo) return { status: 'disabled', reason: t('forge.stratum.noClone') };
    const probe = probes.value.get(probeKey(repo, item.target_branch));
    if (!probe) return { status: 'checking' };
    if (probe === 'failed' || !probe.config) return { status: 'hidden' };
    if (!probe.linter) return { status: 'disabled', reason: t('forge.stratum.noLinter') };
    return { status: 'ready', params: { connector: item.connector, project: item.project, number: item.number, repo, target: item.target_branch } };
  });

  return { availability };
}
