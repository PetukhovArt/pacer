import { cssToken } from './theme';

let loading: Promise<typeof import('mermaid').default> | null = null;
let sequence = 0;

function load() {
  loading ??= import('mermaid').then((module) => module.default);
  return loading;
}

function themeVariables(dark: boolean) {
  const text = cssToken('color-text');
  const surface = cssToken('color-surface');
  const muted = cssToken('color-surface-muted');
  const raised = cssToken('color-surface-raised');
  const line = cssToken('color-text-muted');
  const border = cssToken('color-border');
  return {
    darkMode: dark,
    background: surface,
    fontFamily: cssToken('font-family'),
    primaryColor: muted,
    primaryTextColor: text,
    primaryBorderColor: line,
    secondaryColor: raised,
    secondaryTextColor: text,
    secondaryBorderColor: line,
    tertiaryColor: surface,
    tertiaryTextColor: text,
    tertiaryBorderColor: border,
    lineColor: line,
    textColor: text,
    mainBkg: muted,
    nodeBorder: line,
    nodeTextColor: text,
    clusterBkg: surface,
    clusterBorder: border,
    titleColor: text,
    edgeLabelBackground: surface,
    noteBkgColor: raised,
    noteTextColor: text,
    noteBorderColor: line,
    actorBkg: muted,
    actorBorder: line,
    actorTextColor: text,
    actorLineColor: line,
    signalColor: text,
    signalTextColor: text,
    labelBoxBkgColor: muted,
    labelBoxBorderColor: line,
    labelTextColor: text,
    loopTextColor: text,
    activationBkgColor: raised,
    activationBorderColor: line,
    sequenceNumberColor: surface,
  };
}

export async function renderDiagram(source: string, dark: boolean): Promise<string> {
  const mermaid = await load();
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: 'strict',
    suppressErrorRendering: true,
    theme: 'base',
    fontFamily: cssToken('font-family'),
    themeVariables: themeVariables(dark),
  });
  const id = `rc-mermaid-${++sequence}`;
  try {
    const { svg } = await mermaid.render(id, source);
    return svg;
  } finally {
    document.getElementById(`d${id}`)?.remove();
    document.getElementById(id)?.remove();
  }
}
