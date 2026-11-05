export type ParseFailure = { message: string; line: number | null; column: number | null };
type ParseResult = { ok: true; object: Record<string, unknown> } | { ok: false; error: ParseFailure };

function locate(text: string, position: number): { line: number; column: number } {
  const before = text.slice(0, position).split('\n');
  return { line: before.length, column: (before.at(-1)?.length ?? 0) + 1 };
}

function failure(text: string, error: unknown): ParseFailure {
  const raw = error instanceof Error ? error.message : String(error);
  const message = raw.replace(/\s*in JSON at position \d+(?: \(line \d+ column \d+\))?/, '');
  const explicit = raw.match(/\(line (\d+) column (\d+)\)/);
  if (explicit) return { message, line: Number(explicit[1]), column: Number(explicit[2]) };
  const position = raw.match(/position (\d+)/);
  if (position) return { message, ...locate(text, Number(position[1])) };
  if (/end of JSON input/.test(raw)) return { message, ...locate(text, text.length) };
  return { message, line: null, column: null };
}

export function parseSettings(text: string): ParseResult {
  if (!text.trim()) return { ok: true, object: {} };
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch (e) {
    return { ok: false, error: failure(text, e) };
  }
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return { ok: false, error: { message: 'Корень файла должен быть объектом {…}', line: 1, column: 1 } };
  }
  return { ok: true, object: value as Record<string, unknown> };
}

export function describeFailure(error: ParseFailure): string {
  const where = error.line === null ? '' : `строка ${error.line}, столбец ${error.column}: `;
  return `${where}${error.message}`;
}
