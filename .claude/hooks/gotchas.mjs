#!/usr/bin/env node
// A Claude Code `UserPromptSubmit` hook: reads the prompt on stdin, works out
// which parts of this project it is about, and prints the matching lines of
// GOTCHAS.md. Whatever a hook prints becomes context for that turn, so the
// traps arrive with the question instead of waiting to be looked up.
//
// Matching runs on three signals. Vocabulary: GLOSSARY.md's terms and the
// `_Avoid_` synonyms listed under each one, so "the socket dir" finds lines
// about the Runtime Dir. Labels: the ALL-CAPS lead-in each gotcha carries,
// which is its own vocabulary — SQLITE STORE and MIGRATION have no glossary
// entry. Code: paths, `snake_case` identifiers and `Type::fn` spellings lifted
// out of the prompt verbatim.
//
// A term that labels half the file (Session, Running) says almost nothing, so
// every hit is damped by how many lines already carry it — the rare term is
// the informative one — and a prompt whose best match stays under MIN_SCORE
// gets nothing at all rather than a plausible-looking irrelevance.
//
// Registered in `.claude/settings.json`. Run it by hand:
//   echo '{"prompt":"the stop gate turns green while subagents run"}' \
//     | node .claude/hooks/gotchas.mjs
//
// It fails silent by construction: any error exits 0 with no output, because a
// broken hook must never be able to block a prompt.

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const MAX_LINES = 12; // gotchas injected per prompt
const MAX_CHARS = 6000; // hard ceiling on everything printed
const MIN_PROMPT = 12; // "yes", "do it", "the second one" carry no nouns
const MIN_TOKEN = 5; // shorter code tokens match everything
// Calibrated against the file: a prompt genuinely about one of these areas
// scores its best line above 1.7, while a chatty prompt that happens to say
// "project" or "running" tops out below 0.9. Under the bar, print nothing —
// injecting a plausible-looking but unrelated trap is worse than silence.
const MIN_SCORE = 1.2;

// The repo root, found from this file rather than from cwd or an env var, so
// the hook works the same whichever directory Claude runs it in.
const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..', '..');

const read = (name) => {
  try {
    return readFileSync(join(ROOT, name), 'utf8');
  } catch {
    return '';
  }
};

/** A whole-word occurrence of `needle`, not a substring of a longer word. */
const escape = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
const contains = (hay, needle) =>
  new RegExp(`(?<![\\w-])${escape(needle)}(?![\\w-])`).test(hay);

/**
 * GLOSSARY.md entries as `term -> [term, ...synonyms]`, all lowercased. A
 * `_Avoid_` line lists the words the glossary steers away from, which is
 * exactly the vocabulary a prompt is likely to use.
 */
function glossary() {
  const terms = new Map();
  let current = null;
  for (const line of read('GLOSSARY.md').split('\n')) {
    const heading = /^\*\*(.+?)\*\*\s*$/.exec(line);
    if (heading) {
      current = heading[1].trim();
      terms.set(current, new Set([current.toLowerCase()]));
      continue;
    }
    const avoid = /^_Avoid_:\s*(.+)$/.exec(line);
    if (avoid && current) {
      // Parentheticals hold their own commas ("the API (the Hook Receiver is
      // the HTTP one, not this)") — drop them before splitting.
      for (const raw of avoid[1].replace(/\([^)]*\)/g, '').split(',')) {
        const alias = raw.trim().toLowerCase();
        if (alias.length >= 3) terms.get(current).add(alias);
      }
    }
  }
  return terms;
}

/** GOTCHAS.md as `{ section, label, text }`, in file order. */
function gotchas() {
  const rows = [];
  let section = '';
  for (const line of read('GOTCHAS.md').split('\n')) {
    if (line.startsWith('## ')) {
      section = line.slice(3).trim();
      continue;
    }
    const m = /^- \*\*(.+?)\*\* — (.*)$/.exec(line);
    // The `⟵ <slug>` trailer points at a write-up that only exists in git
    // history; it is worth its bytes in the file, not in an injected prompt.
    if (m) rows.push({ section, label: m[1].trim(), text: m[2].split('⟵')[0].trim() });
  }
  return rows;
}

/** Terms the prompt uses, scored 2 for the term itself and 1 for a synonym. */
function matchTerms(prompt, terms) {
  const hits = new Map();
  for (const [term, aliases] of terms) {
    for (const alias of aliases) {
      if (alias.length >= 3 && contains(prompt, alias)) {
        hits.set(term, alias === term.toLowerCase() ? 2 : 1);
        break;
      }
    }
  }
  return hits;
}

/** Paths, snake_case names and `Type::fn` spellings, taken verbatim. */
function codeTokens(prompt) {
  const rx =
    /\b(?:crates\/)?[\w-]+(?:\/[\w-]+)+\.\w+\b|\b[a-z][a-z0-9]*(?:_[a-z0-9]+)+\b|\b\w+::\w+\b/g;
  return new Set((prompt.match(rx) || []).filter((t) => t.length >= MIN_TOKEN));
}

function main() {
  let prompt = '';
  try {
    const raw = readFileSync(0, 'utf8');
    prompt = (JSON.parse(raw).prompt ?? '').trim();
  } catch {
    return;
  }
  // A slash command is an instruction to Claude, not a question about the code.
  if (prompt.length < MIN_PROMPT || prompt.startsWith('/')) return;

  const lower = prompt.toLowerCase();
  const hits = matchTerms(lower, glossary());
  const tokens = codeTokens(prompt);

  const lines = gotchas().map((row) => ({
    row,
    label: row.label.toLowerCase(),
    body: row.text.toLowerCase(),
  }));
  // The labels are their own vocabulary — not every one has a glossary entry
  // (SQLITE STORE, MIGRATION do not), so a prompt naming one directly has to
  // count as a signal in its own right.
  const labelled = new Set(lines.filter((l) => contains(lower, l.label)).map((l) => l.label));
  if (lines.length === 0 || (hits.size === 0 && tokens.size === 0 && labelled.size === 0)) return;

  // Everything here is damped by how common it is. A label owning eleven lines
  // (SHARED CHECKOUT) localizes less than one owning three (STOP GATE), and a
  // term the file says on every other line (Session, Running) barely localizes
  // at all — without this, any prompt containing "running" drags in the lot.
  const rare = (n) => 1 / Math.log2(2 + n);
  const labelDf = new Map();
  for (const { row } of lines) labelDf.set(row.label, (labelDf.get(row.label) || 0) + 1);
  const termWeight = new Map();
  for (const [term, weight] of hits) {
    const t = term.toLowerCase();
    const df = lines.filter((l) => contains(l.label, t) || contains(l.body, t)).length;
    termWeight.set(t, weight * rare(df));
  }

  const scored = [];
  for (const { row, label, body } of lines) {
    const damp = rare(labelDf.get(row.label));
    let score = 0;

    // The line's own label named in the prompt is the strongest signal there
    // is: the user asked about this exact thing. Most of the weight is flat
    // rather than damped, so a broad label (RELEASE owns nineteen lines) still
    // clears the bar when it is named outright — MAX_LINES caps the volume.
    if (labelled.has(label)) score += 1.5 + 2.5 * damp;

    for (const [term, weight] of termWeight) {
      if (contains(label, term)) score += 3 * weight * damp;
      else if (contains(body, term)) score += 1.5 * weight;
    }
    for (const token of tokens) {
      if (row.text.includes(token) || row.label.includes(token)) score += 2;
    }
    if (score > 0) scored.push({ score, row });
  }
  if (scored.length === 0) return;
  scored.sort((a, b) => b.score - a.score);

  if (scored[0].score < MIN_SCORE) return;
  // Rank alone would always find twelve lines to print, however thin the tail.
  // Cut relative to the best match too: a prompt with one sharp hit gets one
  // line, and a broad one still gets its whole cluster.
  const floor = Math.max(MIN_SCORE, scored[0].score * 0.35);
  const keep = scored.filter((s) => s.score >= floor).slice(0, MAX_LINES);
  const order = [...new Set(keep.map((s) => s.row.section))];
  const subject = [...new Map([...hits.keys(), ...tokens, ...labelled].map((s) => [s.toLowerCase(), s])).values()]
    .sort()
    .join(', ');

  const out = [
    `[pacer gotchas] Traps already paid for that touch this prompt (${subject}).`,
    `Full list, and the task each came from, in GOTCHAS.md.`,
  ];
  for (const section of order) {
    out.push(`\n## ${section}`);
    for (const { row } of keep) {
      if (row.section === section) out.push(`- **${row.label}** — ${row.text}`);
    }
  }
  process.stdout.write(`${out.join('\n').slice(0, MAX_CHARS)}\n`);
}

try {
  main();
} catch {
  // Never block a prompt.
}
process.exit(0);
