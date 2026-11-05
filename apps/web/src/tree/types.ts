import type { Component } from 'vue';

export type TreeNode<T = unknown> = {
  id: string;
  label: string;
  kind: string;
  icon?: Component;
  iconOpen?: Component;
  children?: TreeNode<T>[];
  detail?: string;
  count?: number | string;
  countTitle?: string;
  defaultExpanded?: boolean;
  custom?: boolean;
  inert?: boolean;
  title?: string;
  data?: T;
};

export type TreeRow<T = unknown> = {
  id: string;
  depth: number;
  parent: string | null;
  node: TreeNode<T>;
  group: boolean;
  expanded: boolean;
};
