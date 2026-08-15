# Figma design system

## files

- [Shared UI — Foundations & Components](https://www.figma.com/design/OjBY1RR6vVYmSSxLcSYF4Y)
- [notchtap — Product Design System](https://www.figma.com/design/J4mguJpJEDHDHWbxrhmCMn)

Both files live in Chetan Jain's Starter team drafts.

The Starter plan permits local components, styles, and variables.
It blocks team library publication and cross-file library reuse.

## authority

Use this source order:

1. `src/` defines shipped notchtap surfaces and behaviour.
2. `../shared-ui/design/tokens.css` defines shared tokens.
3. `src/notchtap-tokens.css` defines notchtap-only CSS tokens.
4. `src/components/ui/` and `src/settings/controls/` define local components.
5. `prototype/*.html` supplies study references only.

Figma is downstream documentation and assembly.
Do not add a Figma token without a code source.
Do not treat Figma as motion or lifecycle authority.

## current contents

The Shared UI file contains 43 Dark-mode variables.
It includes 33 colours, one radius, three font stacks, and six motion values.

The Shared UI file contains five component sets:

- Button: 18 variants.
- Input: four states.
- Switch: eight variants.
- Card: two sizes.
- Badge: six variants.

The notchtap file contains 44 local variables.
It includes identity colours, the settings radius and type scales, and motion values.

The notchtap file contains three component sets:

- Segmented: six variants.
- MetaChip: ten variants.
- ActionStatus: four states.

The overlay page contains four reference frames:

- bare
- idle rail
- showing
- expanded

These frames are study references.
They are not reusable overlay components.

## synchronisation

1. Change shared tokens in `../shared-ui/design/tokens.css`.
2. Run `npm run tokens:figma` in `../shared-ui`.
3. Update `../shared-ui/design/figma.tokens.json` with the command output.
4. Run `npm run verify:tokens` and `npm run verify:figma`.
5. Refresh the Shared UI Figma variables from the generated file.
6. Change notchtap extensions in their code source.
7. Run the notchtap web tests and snapshot guard.
8. Refresh the affected Figma variables or reference frames.
9. Inspect screenshots before any library update.

## publication

Upgrade the team to a paid Figma plan before publication.
Move both files from drafts into one team project.
Publish Shared UI first.
Enable Shared UI in the notchtap file.
Replace shared-value paints in notchtap with Shared UI variable bindings.
Publish notchtap second.

The seven radius labels on the notchtap Foundations & Settings page were fixed 2026-08-08.
Layer names and text now read `sm · 6` through `4xl · 26`, matching the renamed variables.
No known pre-publication cleanup remains.

## verification log

2026-08-08 audit, via desktop-app variable export (MCP quota exhausted):

- Shared UI variables: 43 of 43 match `figma.tokens.json` exactly (33 colours, radius, 3 fonts, 6 motion). `media-mint` correctly absent.
- notchtap variables: 44 of 44 match code (`src/notchtap-tokens.css`, `src/lib/sourceColors.ts`, `src/settings/base.css`, `src/animationTiming.ts`, `src/styles.css`). `media-mint` present at `#B6F5E5`.
- `figma.tokens.json` sourceSha256 matches current `design/tokens.css`.
- The seven stale radius labels were the only drift; fixed same day after the plan upgrade (verified by script read-back and screenshot).
- Shared UI Foundations page radius panel checked clean (`radius/base 10` only).

The team upgraded to Figma Professional on 2026-08-08 (200 MCP tool calls per day, Full seat).
On Starter the MCP allowed six tool calls per month; `whoami` is exempt.
The desktop app's collection right-click "Export modes" gives a JSON export for auditing without MCP calls.
