const RANKS: Record<string, number> = {
  blocker: 0,
  highest: 0,
  critical: 1,
  high: 1,
  major: 2,
  medium: 2,
  minor: 3,
  low: 3,
  trivial: 4,
  lowest: 4,
};

function priorityRank(priority: string | null | undefined): number {
  return priority ? (RANKS[priority.toLowerCase()] ?? 5) : 6;
}

export function priorityLevel(priority: string | null | undefined): string {
  return ['blocker', 'high', 'medium', 'low', 'trivial', 'other', 'none'][priorityRank(priority)]!;
}
