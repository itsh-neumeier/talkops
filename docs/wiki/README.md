# Handbook and GitHub Wiki maintenance

Edit [English sources](../en/index.md) and [German translations](../de/index.md)
in the main repository. Review both languages in the same PR. Do not edit a
generated Wiki page independently. The release chat publishes the reviewed copy.
The application PR and Wiki publication are separate operations.

## Source map

[pages.json](pages.json) maps 34 topic pairs and the two language indexes.
Each pair has a stable ID, source filenames, translated titles and unique Wiki
slugs. Section order sets navigation order. `routes` maps UI pages to chapters;
`routeExceptions` explains pages covered through another entry.

Keep existing source filenames and Wiki slugs stable. For a new chapter, add
both translations, update both indexes and add a manifest pair. If the UI gains
a page, map its route to the responsible chapter. The coverage test rejects an
unmapped guide or route. It does not verify the factual accuracy of prose.
Review instructions against the UI, API, deployment files and existing tests.

Each source page has a handbook link and a language switch. Procedures give
required rights, prerequisites, steps, expected results and error guidance.
English follows [documentation rules](../documentation-style.md). A formal
ASD-STE100 review is separate from the automated documentation checks.

## Build and check

Use Node.js 22 or later. No npm install or additional packages are required.
Run from the repository root:

```sh
node tests/docs/wiki.mjs
node scripts/docs/build-wiki.mjs --check
node scripts/docs/build-wiki.mjs
```

The default output is the ignored `build/wiki/` directory. It contains 73
Markdown pages: 68 topic pages, two language indexes, Home, sidebar and footer.
Local images, when used, are copied under `assets/` with unique source-path hashes.
The generator does not clone, initialize or push a Wiki.

`--check` validates the current sources and compares two in-memory builds. It
does not require or modify output files. A normal build can preview uncommitted
edits, with an explicit working-copy note. Source links in that preview point
to its base commit and may not contain the uncommitted text yet.

For publication, use a clean checkout of the reviewed source commit:

```sh
node scripts/docs/build-wiki.mjs --release
```

`--release` rejects a dirty checkout. Every page records the full source commit
and source URL. `--output /path/to/empty-build-directory` selects another output
directory. Use a dedicated directory; never point the build at a Wiki checkout.
The writer refuses unmanaged files and symlinks. It deletes no existing files.
Use a fresh build directory after removing or renaming a source page or asset.

## Link rules

- Use relative Markdown links between source guides. The build changes them to
  Wiki slugs and keeps valid heading fragments, including German characters.
- Use ATX headings (`#`, `##`, and so on). The validator checks their generated
  GitHub-style IDs, including duplicate headings. Keep fragment links in sync
  when changing a heading. Generated section navigation is checked too.
- Source-only targets, such as ADRs or Compose files, become links at the exact
  source commit. Directories become repository tree links.
- Markdown inline destinations and reference definitions are supported, including
  balanced URL parentheses and angle-bracket destinations. Code spans and fenced
  examples are left unchanged. Use Markdown links/images rather than raw HTML
  link attributes or custom HTML anchors in handbook sources.
- HTTPS, HTTP, mail and telephone links stay unchanged. Their remote availability
  is not checked by the offline build. Other schemes and paths outside the
  repository are rejected. Local targets and Markdown fragments must exist.

## Publication by the release chat

The Wiki is a separate repository at
`https://github.com/itsh-neumeier/talkops.wiki.git`.
See GitHub's official [Wiki editing guide](https://docs.github.com/en/communities/documenting-your-project-with-wikis/adding-or-editing-wiki-pages)
and [sidebar/footer guide](https://docs.github.com/en/communities/documenting-your-project-with-wikis/creating-a-footer-or-sidebar-for-your-wiki).
No feature-branch push automatically publishes the handbook.

1. Select the reviewed source commit and record its SHA. Confirm that the main
   repository PR is integrated or explicitly selected by the release manager.
2. Run the tests, `--check` and a `--release` build from that clean commit.
3. Check Wiki access with `git ls-remote --symref` and inspect existing pages.
   If the Wiki Git endpoint is unavailable, distinguish access failure from
   missing initialization. GitHub documents creating an initial page through the
   Wiki interface before cloning. Keep publication pending until access works.
4. Clone the Wiki into a separate directory and verify its actual default branch.
   Do not assume the branch is named `main`.
5. Compare each generated path with the Wiki checkout. Preserve unrelated pages.
   Resolve page-name conflicts or independent edits before copying generated
   content. Review removals separately; the generator does not delete old pages.
6. Copy the reviewed generated files, inspect `git diff --check` and the full
   Wiki diff, and commit with the source SHA in the message.
7. Push the verified Wiki default branch. Never force-push unrelated history.
8. Check Home, both indexes, section navigation, language switches, source links
   and representative long procedures in GitHub's rendered Wiki.
9. Record the source SHA, Wiki commit, published URLs and checks in the release
   handoff. A built directory or merged application PR is not publication proof.

When updating an existing Wiki, retain its previous commit for recovery.
If publication fails, preserve the generated files and report the failed step.
To undo a published documentation update, the release manager can revert its
Wiki commit after reviewing concurrent edits. Application versions and database
migrations are unaffected by handbook publication.

## Review checklist

- Installation samples select matching configuration and image versions.
- Recovery instructions preserve secrets, volumes and Compose identity.
- Role requirements match the current controls and API.
- All topic pairs appear in both indexes and have a language switch.
- Hardware/provider status is explicit; automated tests do not imply live tests.
- Examples contain no real secrets or private data.
- Source validation and generator fixtures pass on the final source revision.
- Live Wiki verification is recorded separately from local checks.
