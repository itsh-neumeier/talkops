// The logged-in user. Loaded once by the root layout.
import { goto } from '$app/navigation';
import { api, setCsrf, setUnauthorizedHandler, type Me, type Role, type User } from './api.ts';

export const session = $state<{ user: User | null; loaded: boolean }>({
	user: null,
	loaded: false
});

const rank: Record<Role, number> = { user: 0, operator: 1, admin: 2 };

export function hasRole(role: Role): boolean {
	return !!session.user && rank[session.user.role] >= rank[role];
}

export function setSession(me: Me) {
	session.user = me.user;
	setCsrf(me.csrf_token);
}

export async function loadSession() {
	try {
		setSession(await api.get<Me>('/auth/me'));
	} catch {
		session.user = null;
	} finally {
		session.loaded = true;
	}
}

export async function logout() {
	try {
		await api.post('/auth/logout');
	} finally {
		session.user = null;
		setCsrf('');
		goto('/login');
	}
}

setUnauthorizedHandler(() => {
	session.user = null;
	setCsrf('');
	goto('/login');
});
