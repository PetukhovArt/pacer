# Mermaid preview in the file tree

## Problem Statement

Architecture diagrams live as mermaid inside markdown. In pacer's file preview (`b`) they show as
fenced source: a wall of `graph TD` and arrow syntax you have to read as code and assemble in your
head. Seeing the actual diagram means leaving pacer for an editor or a browser, which is the thing
pacer exists to avoid.

## Solution

The preview pane renders mermaid as text art. A `.mmd` file previews as a diagram instead of its
source. A markdown file has each fenced mermaid block replaced by the rendered diagram in place,
with the surrounding prose untouched.

An external command does the rendering, `mermaid-ascii` by default, named in a `mermaid_renderer`
setting. When that command is not installed the preview is exactly what it is today, plus one line
above the first block naming what to install.

## User Stories

1. As a developer reading a repo in the tree browser, I want a `.mmd` file to preview as a diagram, so that I can understand the design without opening another program.
2. As a developer reading a README, I want fenced mermaid blocks rendered in place, so that the document reads as a document rather than as source.
3. As a developer, I want the prose around a diagram to keep its syntax highlighting, so that a rendered block does not degrade the rest of the file.
4. As a developer, I want the rendered art left uncoloured, so that the highlighter does not paint box-drawing characters as if they were code.
5. As a developer without the renderer installed, I want the preview to keep showing the mermaid source, so that a missing optional tool never costs me content.
6. As a developer without the renderer installed, I want one line telling me which command to install, so that I can find out the feature exists at all.
7. As a developer who does not want this, I want to empty the `mermaid_renderer` setting, so that pacer never spawns anything.
8. As a developer with a different renderer, I want to name my own command, so that I can use the one that handles my diagram types.
9. As a developer walking a file list, I want repeat visits to a file to cost nothing, so that navigation does not stutter once a diagram has been rendered.
10. As a developer, I want a diagram wider than the pane clipped rather than wrapped, so that the art keeps its shape.
11. As a developer, I want to widen the preview with the existing splitter to read a wide diagram, so that I do not need a new control.
12. As a developer whose diagram has a syntax error, I want the renderer's own message shown in place of the block, so that I can see what is wrong.
13. As a developer, I want a renderer that hangs to be given up on after a short timeout, so that one bad diagram cannot freeze the UI.
14. As a developer editing a diagram, I want the next preview to show the new version, so that the cache never shows me stale art.
15. As a developer, I want every mermaid block in a file rendered, so that the rule is not "the first one only".
16. As a developer, I want fenced blocks that are not mermaid left alone, so that ordinary code blocks keep their highlighting.
17. As a developer, I want directory listings and binary placeholders to behave exactly as before, so that the change is confined to what it touches.
18. As a developer opening a large markdown file, I want the existing preview caps to still apply, so that this cannot make a big file slower than it is today.
19. As a maintainer, I want the mermaid logic in one module behind a small interface, so that a second renderer kind later does not mean touching the tree browser again.
20. As a maintainer, I want the setting in the settings overlay like the others, so that it is discoverable without hand-editing the config.
21. As a developer on Windows, I want the renderer spawned without a console window flashing, so that the preview does not blink black boxes.

## Implementation Decisions

- A new module in the TUI crate holds everything mermaid: block extraction, the subprocess call,
  the cache, the fallback message. Its whole public surface is one function taking the preview
  text, the file's path and the configured command, returning the substituted text plus the line
  ranges that must not be highlighted. A later change of renderer or of extraction rules does not
  reach the caller.
- Extraction is decided by extension. `.mmd` and `.mermaid` mean the whole file is one block. `.md`
  and `.markdown` mean fenced blocks whose info string is exactly `mermaid`. Anything else means no
  blocks and the text comes back unchanged.
- The tree browser's preview loader gains one step between reading the file and building the
  highlighted lines: pass the text through the module, then build the lines as today except that
  lines inside a returned range skip the highlighter and become plain tokens. That plain path
  exists today only for whole-file placeholders, so it generalises from whole-file to a set of
  ranges.
- The renderer is a program plus arguments taken verbatim from the setting. The block's source goes
  to its stdin, the art comes back on stdout. Feeding stdin rather than a path keeps the contract
  renderer-agnostic and needs no temporary files. A non-zero exit puts the renderer's stderr in
  place of the block.
- The call carries a short timeout. A renderer that has not answered is abandoned and the block
  stays as source.
- Results are memoised by a hash of the block's source, not by file path, so the same diagram in two
  files renders once and an edit invalidates on its own. The cache is bounded and private to the
  module.
- Whether the renderer exists is probed once per process, on the first block met, and remembered. A
  missing renderer costs one failed spawn for the session, not one per file.
- With the renderer missing, the text comes back unchanged except for one line above the first block
  naming the command to install. An empty setting inserts nothing.
- The setting is a string field on the TUI config beside `editor`, defaulting to the default
  renderer's name, with an accessor resolving the empty case to off, and a row in the settings
  overlay.
- Art is never wrapped. Lines wider than the pane are clipped; the existing splitter is how a wide
  diagram gets read.
- The existing byte and line caps apply before extraction, so the feature inherits them.
- Spawning follows whatever the codebase already does to keep console windows hidden on Windows.

## Testing Decisions

A good test here asserts what the preview shows, not how the module got there: the substituted text,
which ranges came back plain, and what happens with no renderer. Nothing asserts on the cache, on
spawn counts, or on internal struct shapes.

Three tests, in the new module's own test section, matching the in-file convention the tree browser
already uses.

1. Extraction against markdown. Fails when someone changes the fence scanner and it starts matching
   a differently labelled fence, misses a second block, or eats an indented one. That edit is
   routine and its breakage is silent and wide: it mangles previews of ordinary markdown that has
   nothing to do with mermaid. Risk category: edge case with high error cost.
2. Substitution and plain ranges together. Fails when someone changes the splice offsets and the
   ranges stop lining up with the art, so the highlighter colours box-drawing characters or the art
   shifts a line. Risk category: contract between the module and the preview loader, the one thing
   the two agree on.
3. The missing renderer. Fails when someone changes the fallback and an absent command blanks the
   preview or drops the block instead of leaving the source. Risk category: regression with high
   cost, since the preview would silently lose content.

The subprocess is exercised through a stub program, the way the editor overlays already are; the
codebase carries a stub binary for exactly this. Prior art for asserting on preview output is the
tree browser's own tests, which build a browser over a temporary directory and assert on the preview
fields.

No test for the cache. No concrete edit to it produces a visible failure, and a miss is invisible by
design.

## Out of Scope

- Mermaid in pull request bodies. Separate rendering path, no shared code, a follow-up.
- Image rendering through sixel, kitty or iTerm2. Needs a browser-backed renderer and a different
  draw path, and it contradicts the single-binary install.
- A plugin system. This is deliberately a plain feature; the extension point comes out only when a
  second and third renderer make its shape visible.
- Width-aware rendering, where the pane's width reaches the renderer.
- Asynchronous rendering. It stays available if a measurement says the synchronous call is felt.
- Any change to the diff viewer, the file finder or find-in-files.

## Further Notes

Rendering happens on the thread handling the keypress, so a first render is felt as latency in the
file tree. That was accepted knowing the number is unmeasured. The first thing to do once it works
is measure it on a real markdown file on Windows, where process spawn is slowest. If it is felt, the
fix is the asynchronous path or an explicit key, both sitting on top of the same module.

This repository has no mermaid blocks, so the feature has to be tried against a file written for the
purpose or another checkout.

Renderers worth knowing: mermaid-ascii (Go, flowcharts and sequence diagrams), beautiful-mermaid
(JavaScript, more diagram types), mermaid-ascii-diagrams (Python), mermaid2term. Cursor's CLI already
renders mermaid inline, which is prior art for the interaction.
