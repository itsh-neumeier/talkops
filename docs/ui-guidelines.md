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

## Footer

The root layout shows `AppFooter.svelte` for signed-in users. It is fixed at
the bottom with the height `--footer-h` (`app.css`). Fixed or sticky
elements at the bottom must stay above it, for example with
`bottom-[calc(var(--footer-h)+1rem)]`. Do not add a separate page footer.

## Side panel

List pages for phones, extensions and users show details in a side panel,
like the inspector of UniFi Talk.

- Use `SidePanel.svelte` with icon tabs (`panelIcons.ts`), usually
  **Overview**, a content tab and **Settings**. Wide screens keep the list
  visible: the root layout keeps the panel width free while the panel is
  open (`.app-body:has(.side-panel)` in `app.css`). Do not add a padding to
  the page itself, because the page is inside the centred content column.
  Narrow screens show the panel in full.
- Keep the open entry in `?panel=<id>` with `#lib/panel.ts`. A row click and
  the name link (`data-panel-link`) open the panel; Ctrl/Cmd-click still
  opens the full page in a new tab.
- In **Settings**, use `Collapsible.svelte` groups **General** and
  **Manage**. Each action is a `PanelAction.svelte` with an info hint; a
  delete action uses `danger`.
- Keep detailed editors on the full page. The panel links to it with
  **Open all settings**.
- Use `Combobox.svelte` for a select with many options that needs a search.

## Sort and filter lists

The call list, the phone book and the transcription history use the same
parts. Use these parts for other sortable or filterable tables.

- `SortButton.svelte` is the sort button in a column title. Set
  `aria-sort` on the `th` with `ariaSort()` from `#lib/list/sort.ts`.
  `nextSort()` gives the state after a click.
- `ColumnFilter.svelte` is the funnel button and the filter panel of a
  column. Put `FilterOptions.svelte` (check boxes, or radio buttons with
  `single`) or other fields in the panel.
- Keep the filters and the sort order in the page address. Write and read
  the query string in a module of the list (for example
  `#lib/calls/filters.ts`). Use `pickList()`, `parseOrder()` and
  `queryString()` from `#lib/list/sort.ts`. `syncAddress()` from
  `#lib/list/address.ts` replaces the address without a new history entry.
- Sort and filter in the browser only when the page loads the full list.
  Otherwise, send the filters to the API.
- Show "*n* of *m*" below the table and **Reset all filters** when a filter
  is active.

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
- Skeletons (`Skeleton.svelte`, `SkeletonRows.svelte`) are visible from
  the first frame. Use `.skeleton` and `.skeleton-bar` for placeholders.
  The bars have a light sweep. Do not use `animate-pulse`.
- For the first load of call content, use `LoadingContent.svelte`. Set
  `pending` from the request for that content. Supply a `placeholder`
  snippet and a child snippet. The component keeps the placeholder visible
  for at least 300 ms. Put `.content-in` on the loaded content for a 180 ms
  opacity change. The component adds no wrapper, so it can contain table
  bodies. Reduced motion removes the minimum interval and the animation.
- Keep loaded call content during polling and filter requests. Show a
  status message during a filter request. Add placeholder rows while the
  next page loads. Do not show an empty state before the first response.
- Use `PhoneNumber.svelte` for numbers with country flags. Enable `flag`
  only on inbound callers and outbound destinations. Set `internal` when
  extension metadata identifies the party. Use the normalized API number
  as `countryNumber` when the displayed number has another format.
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

`tests/browser/motion.mjs` checks immediate skeleton visibility, the page
fade, the dialog transition and reduced motion. `call-presentation.mjs`
checks call flags, initial loading, independent dashboard requests, polling,
filters, pagination, empty and error states, and reduced motion.

Country detection uses the pinned `libphonenumber-js` package with full
metadata. SVG assets come from the pinned `flag-icons` package. Both ship
with the application. The browser makes no external lookup. The resolver
accepts international numbers only; tenant normalization stays in the API.
Shared service ranges and numbers without a resolved country have no flag.
The softphone endpoint supplies raw records, so its flags require a `+` or
`00` international prefix. Names and dial targets do not change.
See the upstream [number library](https://github.com/catamphetamine/libphonenumber-js)
and [flag assets](https://github.com/lipis/flag-icons) for metadata and licenses.

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
