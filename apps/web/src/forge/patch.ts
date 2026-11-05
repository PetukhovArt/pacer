export type PatchLine = {
  index: number;
  kind: 'add' | 'del' | 'ctx' | 'hunk' | 'meta';
  oldLine: number | null;
  newLine: number | null;
  text: string;
};

export type SplitRow = { left: PatchLine | null; right: PatchLine | null; full: PatchLine | null };

const HUNK = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/;

export function parsePatch(patch: string): PatchLine[] {
  if (!patch) return [];
  const lines: PatchLine[] = [];
  let oldNo = 0;
  let newNo = 0;
  let inHunk = false;
  const push = (kind: PatchLine['kind'], oldLine: number | null, newLine: number | null, text: string) => lines.push({ index: lines.length, kind, oldLine, newLine, text });
  const source = patch.split('\n');
  if (source.at(-1) === '') source.pop();
  for (const raw of source) {
    const line = raw.endsWith('\r') ? raw.slice(0, -1) : raw;
    const hunk = HUNK.exec(line);
    if (hunk) {
      oldNo = Number(hunk[1]);
      newNo = Number(hunk[2]);
      inHunk = true;
      push('hunk', null, null, line);
      continue;
    }
    if (!inHunk) continue;
    const mark = line[0];
    if (mark === '+') push('add', null, newNo++, line.slice(1));
    else if (mark === '-') push('del', oldNo++, null, line.slice(1));
    else if (mark === '\\') push('meta', null, null, line);
    else push('ctx', oldNo++, newNo++, mark === ' ' ? line.slice(1) : line);
  }
  return lines;
}

export function splitRows(lines: PatchLine[]): SplitRow[] {
  const rows: SplitRow[] = [];
  let i = 0;
  while (i < lines.length) {
    const line = lines[i]!;
    if (line.kind === 'hunk' || line.kind === 'meta') {
      rows.push({ left: null, right: null, full: line });
      i++;
    } else if (line.kind === 'ctx') {
      rows.push({ left: line, right: line, full: null });
      i++;
    } else {
      const dels: PatchLine[] = [];
      const adds: PatchLine[] = [];
      while (i < lines.length && lines[i]!.kind === 'del') dels.push(lines[i++]!);
      while (i < lines.length && lines[i]!.kind === 'add') adds.push(lines[i++]!);
      const height = Math.max(dels.length, adds.length);
      for (let k = 0; k < height; k++) rows.push({ left: dels[k] ?? null, right: adds[k] ?? null, full: null });
    }
  }
  return rows;
}

export function sideTexts(lines: PatchLine[]): { oldCode: string; newCode: string; oldIndex: Map<number, number>; newIndex: Map<number, number> } {
  const oldLines: string[] = [];
  const newLines: string[] = [];
  const oldIndex = new Map<number, number>();
  const newIndex = new Map<number, number>();
  for (const line of lines) {
    if (line.kind === 'ctx' || line.kind === 'del') {
      oldIndex.set(line.index, oldLines.length);
      oldLines.push(line.text);
    }
    if (line.kind === 'ctx' || line.kind === 'add') {
      newIndex.set(line.index, newLines.length);
      newLines.push(line.text);
    }
  }
  return { oldCode: oldLines.join('\n'), newCode: newLines.join('\n'), oldIndex, newIndex };
}
