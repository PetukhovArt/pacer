import { tr } from '../settings/translate';

type ContextValue = boolean | string;
export type Context = Readonly<Record<string, ContextValue>>;
export type Condition = (context: Context) => boolean;

const TOKEN = /\s*(&&|\|\||==|!=|!|\(|\)|'[^']*'|"[^"]*"|[A-Za-z_][\w.-]*)/y;

function tokenize(source: string): string[] {
  const tokens: string[] = [];
  TOKEN.lastIndex = 0;
  while (TOKEN.lastIndex < source.length) {
    if (!source.slice(TOKEN.lastIndex).trim()) break;
    const match = TOKEN.exec(source);
    if (!match?.[1]) throw new Error(tr('commands.when.unexpectedChar', { source, position: TOKEN.lastIndex }));
    tokens.push(match[1]);
  }
  return tokens;
}

const isQuoted = (token: string) => token.startsWith("'") || token.startsWith('"');
const isWord = (token: string) => /^[A-Za-z_]/.test(token);

export function parseCondition(source: string): Condition {
  const tokens = tokenize(source);
  let position = 0;

  const peek = () => tokens[position];
  const take = () => tokens[position++];

  function parseOr(): Condition {
    let left = parseAnd();
    while (peek() === '||') {
      take();
      const previous = left;
      const right = parseAnd();
      left = (context) => previous(context) || right(context);
    }
    return left;
  }

  function parseAnd(): Condition {
    let left = parseUnary();
    while (peek() === '&&') {
      take();
      const previous = left;
      const right = parseUnary();
      left = (context) => previous(context) && right(context);
    }
    return left;
  }

  function parseUnary(): Condition {
    if (peek() === '!') {
      take();
      const inner = parseUnary();
      return (context) => !inner(context);
    }
    return parsePrimary();
  }

  function parsePrimary(): Condition {
    const token = take();
    if (token === undefined) throw new Error(tr('commands.when.unterminated', { source }));
    if (token === '(') {
      const inner = parseOr();
      if (take() !== ')') throw new Error(tr('commands.when.missingParen', { source }));
      return inner;
    }
    if (!isWord(token)) throw new Error(tr('commands.when.expectedName', { token, source }));
    const operator = peek();
    if (operator !== '==' && operator !== '!=') return (context) => Boolean(context[token]);
    take();
    const operand = take();
    if (operand === undefined || (!isQuoted(operand) && !isWord(operand))) throw new Error(tr('commands.when.expectedValue', { operator, source }));
    const expected = isQuoted(operand) ? operand.slice(1, -1) : operand;
    const equal = operator === '==';
    return (context) => (String(context[token] ?? '') === expected) === equal;
  }

  const condition = parseOr();
  if (position < tokens.length) throw new Error(tr('commands.when.extra', { token: tokens[position] ?? '', source }));
  return condition;
}
