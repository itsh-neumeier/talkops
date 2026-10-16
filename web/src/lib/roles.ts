// Display helpers for roles.
import type { RoleRecord, User } from './api.ts';
import { t } from './i18n/index.svelte.ts';

/** Built-in roles have translated names; own roles their stored name. */
export function roleName(r: Pick<RoleRecord, 'builtin' | 'name'>): string {
	return r.builtin ? t(`roles.${r.builtin}`) : r.name;
}

/** Built-in roles have translated descriptions. */
export function roleDescription(r: Pick<RoleRecord, 'builtin' | 'description'>): string {
	return r.builtin ? t(`roles.desc.${r.builtin}`) : r.description;
}

/** The role record of a user: the own role, else the built-in role. */
export function roleOf(user: Pick<User, 'role' | 'role_id'>, roles: RoleRecord[]) {
	return user.role_id
		? roles.find((r) => r.id === user.role_id)
		: roles.find((r) => r.builtin === user.role);
}
