# Roles with fine-grained permissions and a check box editor

## Task state

- Task ID: 20261015-claude-roles-permissions
- Date: 2026-10-15, Europe/Berlin
- Tool: Claude Code
- Status: Completed. The PR records the readiness.
- Branch: claude/role-permissions
- Base commit: 702507b (origin/main, TalkOps 1.7.13)
- Related commits or PR: Pending

## Goal

PR 2 of 4 of the approved call log feedback plan (see
`20261015-claude-call-log-filters.md`). User management with permissions
for each role, a "Default user" role, admin-defined roles and a check box
editor. The admin role always has all permissions. The user selected
"one role for each user" (no permissions for each user).

## Result

Implemented, tested and documented on the feature branch.

## Changes

- `migrations/20261101000001_roles_permissions.sql`: table `roles`,
  `users.role_id`, function `create_builtin_roles`, trigger on `tenants`.
- `crates/talkops-core/src/roles.rs`: `Permission` enum (31 keys, area,
  sensitive flag), `Permissions` set, `OPERATOR_DEFAULTS`, role CRUD,
  `assignment`, `of_user`, `directory_role`.
- `crates/talkops-core/src/users.rs`: `role_id` in `User`, `NewUser`,
  `UserUpdate`; `SessionUser.permissions` from the session query.
- `crates/talkops-core/src/identity.rs`: `ExternalAccount.groups`; own
  role from the directory group in `upsert_user`.
- `crates/talkops-api/src/auth.rs`: `AuthUser.permissions`, `can`,
  `require_perm`, `is_admin`.
- `crates/talkops-api/src/routes/*.rs`: all `require(Role::…)` checks and
  role comparisons replaced with permissions (mapping in ADR 0029 and
  `docs/en/users-and-roles.md`).
- `crates/talkops-api/src/routes/roles.rs`: `/api/v1/roles` endpoints.
- `crates/talkops-api/src/routes/users.rs`: `assignable` and `manageable`
  guards against more rights.
- `crates/talkops-api/src/routes/auth.rs`: `Me.permissions`.
- `crates/talkops-api/src/ldap.rs`, `routes/oidc.rs`: groups and own roles,
  also in the hourly LDAP sync.
- Web: `session.svelte.ts` (`can`, `isAdmin`), `lib/roles.ts`,
  `components/RoleEditor.svelte`, users page with tabs, all `hasRole`
  checks replaced except the main menu (all entries are for every user).
- Tests: `crates/talkops-api/tests/roles.rs`, `roles::tests`,
  `domain.rs::directory_groups_select_own_roles`,
  `tests/browser/roles.mjs` (added to CI).
- Docs: `docs/adr/0029-roles-and-permissions.md`,
  `docs/en/users-and-roles.md`, `docs/de/benutzer-und-rollen.md`, links in
  sign-in and recording docs.

## Decisions

- `users.role` stays for the built-in role; `users.role_id` only for own
  roles (compatibility with API clients, LDAP sync and the last-admin rule).
- Migration defaults reproduce the old fixed roles exactly. A test compares
  the migration with `OPERATOR_DEFAULTS`.
- Permissions are read for each request (session query), so changes apply
  without a new login.
- Only admins change roles. `users.manage` holders can only give roles
  inside their own permissions and cannot change users with more rights.
- Built-in role names and descriptions are translated in the UI by
  `builtin` key; the stored English text is a fallback.
- Validation errors (change admin role, delete built-in role) return 422,
  a role with users returns 409 on delete.

## Checks

- Passed: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`.
- Passed: `DATABASE_URL=… cargo test --workspace` with PostgreSQL 16
  (all existing tests unchanged and green; new roles tests).
- Passed: `npm run lint`, `npm run check`, `npm test`, `npm run build`.
- Passed: `tests/browser/roles.mjs`, `layout.mjs`, `active-calls.mjs`,
  `transcription-settings.mjs` (Playwright 1.56.1, local Chromium).
- Not run: Docker/SIPp end-to-end test. No FreeSWITCH or routing change.
- STE review: full standard not available; language review pending.

## Dependencies and open work

- Migration ID `20261101000001`. ADR number 0029. Renumber at integration
  if a concurrent task uses them.
- PR 3 (transcription jobs) adds new permissions with a migration.
- Independent of PR 1 (call log); both change `routes/calls/+page.svelte`
  and `routes/settings.rs` (`list_calls`). Expect a small merge conflict:
  keep the call log code of PR 1 and use `auth.can(Permission::CallsViewAll)`
  and `can('recordings.delete')`.

## Next action

Release manager: review and integrate. Claude Code: continue with PR 3.

## References

- `docs/ai-memory/20261015-claude-call-log-filters.md`
- `docs/adr/0029-roles-and-permissions.md`
