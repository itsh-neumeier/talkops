# Web UI guidelines

This document describes the shared building blocks and layout rules of the
TalkOps web UI (`web/`). Use these rules for new pages and for changes to
existing pages. The aim is one design language and correct layout on phones,
tablets and desktops.

## Theme and native widgets

- The `dark` class on `<html>` selects the dark theme.
  `web/static/theme-init.js` sets the class before the first paint.
  The file loads with `<script src>` because the Content Security Policy
  (`web/vite.config.ts`) blocks inline scripts without a hash.
  Do not add inline scripts to `web/src/app.html`.
- `web/src/app.css` sets `color-scheme` from the theme class. Native
  scrollbars and form controls thus follow the theme of the app, not the
  theme of the operating system.
- All scroll areas get thin scrollbars in theme colors. Do not add
  scrollbar styles to single components.

## Page structure

- Start each page with `PageHeader.svelte`. It shows the back link, the
  title (`title` or a `heading` snippet), the actions and a short intro.
  Actions move below the title on narrow screens.
- Put content in cards (`.card`). Put related form fields in one card with
  an `h2` title.
- Use `Tabs.svelte` for tabs. Use `variant="line"` for page sections and
  `variant="pills"` for a small switch in a card. The component supplies the
  ARIA roles and the arrow-key navigation.
- The settings page selects its section with `?section=<id>`. A section with
  sub-tabs adds `&tab=<id>`, for example
  `/settings?section=system&tab=diagnostics`. Old `/settings#<id>` links
  still work. Give each section one permission area. Put a large section
  into sub-tabs instead of one long page.
- Use `Modal.svelte` for dialogs. Dialog panels use the `.dialog-panel`
  class. The close button is an icon button with the label from
  `common.close`.
- `web/src/routes/+error.svelte` shows unknown addresses and unexpected
  errors inside the normal layout.

## Responsive layout

- Use container queries (`@lg:`, `@xl:`, `@3xl:`), not viewport
  breakpoints, for grids in page content. The content width changes with the
  side menus, so the viewport width is not a reliable value.
  `<main>`, the settings content column and `.dialog-panel` are containers.
- Give each grid a base column count, for example
  `grid grid-cols-1 gap-3 @lg:grid-cols-2`. A grid without `grid-cols-1`
  grows with long words and makes the page scroll sideways.
- Recommended steps: two columns from `@lg` (32rem), three columns from
  `@xl` (36rem), four columns from `@2xl` (42rem).
- The settings menu (`SettingsNav.svelte`) is a column at a viewport width
  of 1024 px and more. Below this width it is a horizontal strip above the
  content. The strip scrolls the active entry into view.
- The height of the top bar is the CSS variable `--header-h`. Use this
  variable for sticky and fixed positions. Use `dvh` units, not `vh`.

## Tables

- Put each table in an element with the `.scroll-x` class. When a table is
  wider than its card, edge shadows show the hidden part. Touch devices
  scroll without a visible scrollbar.
- Mark less important columns with `.col-opt` (shown from `@xl`) or
  `.col-opt-lg` (shown from `@3xl`). Set the class on the `th` and on the
  `td` of the same column. Keep the identifying column and the action
  column visible.
- Show `SkeletonRows.svelte` while a list loads. Show `EmptyRow.svelte`
  when a loaded list has no entries.

## Shapes and colors

| Element | Corner radius |
| --- | --- |
| Buttons, inputs, select boxes, segment buttons | `rounded-md` |
| Menu entries (`.nav-item`), boxes and code blocks in a card | `rounded-lg` |
| Cards, dialogs, popup menus | `rounded-xl` |
| Badges, round icons | `rounded-full` |

- Teal is the accent color. Use `.btn-primary` for the main action of a
  page or a form. Use `.btn-danger` for actions that delete data.
- Use the badge classes (`.badge-ok`, `.badge-warn`, `.badge-bad`,
  `.badge-muted`) for states.
- Give each color class a `dark:` variant.
- Use `.nav-item` with `aria-current="page"` for entries of the main menu
  and the settings menu.

## Loading states and motion

- Use one motion system. `web/src/app.css` defines the curve
  `--ease-out` and the durations `--dur-fast` (120 ms: hover, press,
  focus), `--dur` (180 ms: rows, panels, popups) and `--dur-slow`
  (260 ms: page content). Do not add other durations or curves.
- Skeletons (`Skeleton.svelte`, `SkeletonRows.svelte`) stay invisible for
  `--skeleton-delay` (180 ms). A fast answer thus shows no flash. A slow
  answer shows the skeleton with a soft fade and a light sweep. Use the
  classes `.skeleton` and `.skeleton-bar` for own placeholders, not
  `animate-pulse`.
- The root layout fades in the content of each new page (`.page-in`).
- `Modal.svelte` fades the backdrop and moves the panel in and out. Native
  `<dialog class="dialog-panel">` elements open with the same motion.
- Use `.row-in` for new list items, `.expand-in` for opened detail rows and
  `.popover-panel` for popups. `.progress-running` marks a running
  progress bar.
- Buttons, inputs, tabs and menu entries change their colors with a
  transition; buttons move down 1 px when pressed.
- With `prefers-reduced-motion: reduce`, all animations and transitions end
  at once and do not repeat. Svelte transitions get their duration from
  `motion()` in `web/src/lib/motion.ts`.

## Checks

`tests/browser/motion.mjs` checks the skeleton delay, the page fade, the
dialog transition and reduced motion.

`tests/browser/layout.mjs` opens all main pages at 360 px, 768 px and
1280 px in the light and the dark theme. The check fails when a page scrolls
sideways, when the stored theme is lost after a reload, or when the browser
reports a Content Security Policy error. CI runs this check after the build.
Run it locally against `npm run preview`:

```sh
cd web
npm run build
npm run preview -- --host 127.0.0.1 --port 5184 &
node ../tests/browser/layout.mjs
```
