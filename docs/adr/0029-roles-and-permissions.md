# 0029 - Roles with fine-grained permissions

- Status: Accepted
- Date: 2026-10-15
- Amends: [ADR 0014](0014-identitaet-oidc-ldap-totp.md) (roles from
  directory groups)

## Context

TalkOps had three fixed roles: `user` < `operator` < `admin`. Each API
handler compared the role of the user with a minimum role. The user wants
to set the rights of each role in detail, with a check box editor. The
administrator must keep all rights. A family or a small business needs
roles such as "Reception" or "Children" with a small set of rights.

## Options

1. Keep the fixed roles and add permissions for each user.
2. Roles with a permission list, one role for each user, plus own roles.
3. Roles plus permissions for each user (allow and deny).

## Decision

Option 2. The user selected it in the review. One role for each user keeps
the rights easy to see. A user with special rights gets an own role.

- Table `roles`: three built-in roles for each tenant (`admin`, `operator`,
  `user`) and own roles. Each role has a list of permission keys and an
  optional directory group. A trigger on `tenants` creates the built-in
  roles for a new tenant.
- `users.role` stays. It holds the built-in role. `users.role_id` holds an
  own role; such a user has `users.role = 'user'`. Thus the existing API,
  the LDAP sync and the "last admin" rule stay compatible.
- The `admin` role has all permissions. Nobody can change it.
- The permission catalog is an enum in `talkops_core::roles`. Each API
  handler checks one permission (`AuthUser::require_perm`). Access to own
  data (own extensions, calls, recordings, voicemail) needs no permission.
- The migration gives `operator` the rights of the old fixed operator role
  and `user` no extra rights. Thus an update does not change any access.
- The session lookup reads the permissions of the role for each request.
  A role change applies at once, without a new login.
- Only admins change roles and give the admin role. A user with
  `users.manage` can only give roles whose permissions they have, and can
  only change users whose permissions they have.
- An own role can have a directory group. At each LDAP/OIDC login and in
  the hourly LDAP sync, a member of this group gets the own role. Admins
  stay admins. The user group of the directory login still controls who
  can log in.

## Consequences

- Positive: admins can give rights in detail. No behavior change at update.
- Positive: new features add a permission key and a default in a migration.
- Negative: permissions such as `users.manage`, `identity.manage` and
  `backups.manage` can still give indirect power (for example a restore).
  The editor marks them as critical.
- Negative: the web UI shows or hides controls from the permission list
  in `/auth/me`. The server check remains the only security boundary.
