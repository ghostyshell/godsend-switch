# GODsend Switch design system

The same "midnight console" family as GODsend PSP and GODsend 360: near-black
ink, one laser accent spectrum, brass as the warm note, hairline glass
surfaces, monospaced data, staggered entrances, and a hard reduced-motion kill
switch.

Everything lives in `app/src/styles.css`. Tokens are on `:root`; raw hex in
components is banned.

## Tokens

```css
/* surfaces */
--ink-0: #04060c;  --ink-1: #070a12;  --ink-2: #0b0f1a;
--surface: rgba(16, 20, 34, 0.72);
--surface-2: rgba(22, 27, 45, 0.82);
--surface-3: rgba(30, 37, 58, 0.9);

/* lines */
--line: rgba(255, 255, 255, 0.065);
--line-strong: rgba(255, 255, 255, 0.13);
--line-laser: rgba(109, 139, 255, 0.32);

/* type */
--text: #eef2fb;  --text-dim: #a5b0c7;  --text-faint: #7b869f;

/* spectrum: indigo -> cyan identifies the family */
--laser: #6d8bff;  --laser-2: #56e8f5;  --violet: #b79bff;  --brass: #f2c078;
--good: #4ae3a5;  --warn: #ffc46b;  --bad: #ff8484;
--grad-laser: linear-gradient(112deg, #56e8f5, #6d8bff 52%, #8678ff);

/* shape and motion */
--radius-xl: 22px; --radius-lg: 16px; --radius: 12px; --radius-sm: 9px;
--ease: cubic-bezier(0.22, 0.8, 0.24, 1);
--ease-out: cubic-bezier(0.16, 1, 0.3, 1);
--glow-laser: 0 10px 34px -12px rgba(109, 139, 255, 0.55);
```

The app mark is the purple download-loop the user supplied: a fat down arrow
inside an open circular sweep. `scripts` keep `assets/logo-source.png` as the
source of truth; `npx tauri icon` regenerates every platform icon from it.

## Fonts

- Chakra Petch 400-700 for UI copy, via Google Fonts.
- JetBrains Mono 400/500 for identifiers, paths, sizes, versions and
  timestamps. If a number changes on screen it is mono.

## Shell layout

```
topbar      62px (84px on macOS for the overlay title bar), drag region
rail        236px nav, 11 sections, icon plus label, collapses under 1080px
workspace   one scrolling page, content max width 980px
inspector   fixed right column 300px: device card plus job queue, hidden
            under 1240px
statusbar   34px: transport dot, sys-con dot, MCP port, version
```

Sections: Console, Devices, Library, Controller, Game Index, Downloads,
Files, Convert, Homebrew, Updates, Settings.

## Ambient backdrop

Fixed radial gradients, three blurred drifting orbs (cyan, indigo, brass,
`orb-drift 26s`) and a 56px hairline grid at `z-index: -1`. Never raise it and
never put a full-viewport `mask-image` over content; that combination erases
text in WebKit and Chromium.

## Components

- `.btn` ghost, `.btn.primary` with the laser gradient and sheen sweep,
  `.btn.danger`, `.btn.small`.
- `.chip` with a `.dot`. State always carries a word, never colour alone.
- `.seg` segmented control for filters and source switches.
- `.bar` progress track, `.bar.live .fill` for running jobs.
- `.switch` toggle.
- `.card`, `.panel` translucent surfaces with a hairline border and inset
  highlight.
- `.tile` library cell: cover art or monogram, title, mono subtitle, actions.
- `.store-card` homebrew card: name, category tag, description, note, repo,
  version and size, install button.
- `.rows` and `.row` for lists of consoles, downloads, files and updates.
- `.checks` and `.check` for preflight results.
- `.empty` dashed box for empty states.
- `.toast` bottom-centre status pill.

### Controller

The pad is a hand-drawn surface, not an image: `.pad` holds `.pad-shoulders`,
`.pad-main` (d-pad and face cluster positioned absolutely in a 132px square),
`.pad-center` and `.pad-sticks` with an 8-direction `.analog` grid.

`.pad-btn` is a mono label on a glass key. `:active` and `.on` fill it with the
laser gradient and push it 1px down, so a press reads even with the pointer
still held. Buttons use `touch-action: none` so a finger drag does not scroll
the page mid-press.

## Motion

Entrances: `.page` fades and rises 10px (340ms), direct children stagger
10-130ms. Rail items stagger. Durations `--fast 0.16s`, `--base 0.26s`,
`--slow 0.5s`. Only transform and opacity animate. Hard kill switch:

```css
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.001ms !important;
    transition-duration: 0.001ms !important;
  }
  .backdrop .orb { animation: none !important; }
}
```

## Accessibility

- `:focus-visible` rings in `--laser-2` with a 2px offset.
- `aria-current="page"` on the rail, `role="status"` on the title and toasts,
  skip link to `#workspace`, Alt+1..9 section jumps on the rail.
- The pad's buttons are real buttons with `aria-label`s; the keyboard mapping
  is documented on the page.
- Destructive actions always go through `window.confirm` with the exact path.

## Copy

No em dashes and no en dashes in user-visible text. ASCII hyphens, commas or
parentheses only. Numbers and paths in mono.
