<script lang="ts" module>
	export type Section =
		'telephony' | 'calls' | 'voicemail' | 'email' | 'security' | 'system' | 'account';
	export const sections: { id: Section; icon: string; admin: boolean }[] = [
		{ id: 'telephony', icon: '☎', admin: true },
		{ id: 'calls', icon: '🎵', admin: true },
		{ id: 'voicemail', icon: '📮', admin: true },
		{ id: 'email', icon: '✉', admin: true },
		{ id: 'security', icon: '🛡', admin: true },
		{ id: 'system', icon: '⚙', admin: true },
		{ id: 'account', icon: '👤', admin: false }
	];
	type PageKey =
		| 'nav.myPhones'
		| 'nav.extensions'
		| 'nav.users'
		| 'nav.routing'
		| 'nav.trunks'
		| 'nav.numbers'
		| 'nav.phones'
		| 'nav.doorStations'
		| 'nav.audit';
	/**
	 * Pages that live under Settings instead of the main menu: personal ones
	 * (with the sections) and the setup of the system.
	 */
	export const settingsPages: {
		href: string;
		key: PageKey;
		icon: string;
		role: 'admin' | 'operator' | 'user';
		group: 'personal' | 'setup';
	}[] = [
		{ href: '/me', key: 'nav.myPhones', icon: '📱', role: 'user', group: 'personal' },
		{ href: '/extensions', key: 'nav.extensions', icon: '☏', role: 'operator', group: 'setup' },
		{ href: '/users', key: 'nav.users', icon: '👥', role: 'operator', group: 'setup' },
		{ href: '/routing', key: 'nav.routing', icon: '🔀', role: 'operator', group: 'setup' },
		{ href: '/trunks', key: 'nav.trunks', icon: '🔌', role: 'operator', group: 'setup' },
		{ href: '/numbers', key: 'nav.numbers', icon: '#', role: 'operator', group: 'setup' },
		{ href: '/phones', key: 'nav.phones', icon: '📟', role: 'operator', group: 'setup' },
		{ href: '/door-stations', key: 'nav.doorStations', icon: '🚪', role: 'admin', group: 'setup' },
		{ href: '/audit', key: 'nav.audit', icon: '📜', role: 'admin', group: 'setup' }
	];
</script>

<script lang="ts">
	import { page } from '$app/state';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { hasRole } from '#lib/session.svelte.ts';

	let {
		section = null,
		onsection
	}: {
		/** Active section on the settings page itself (null elsewhere). */
		section?: Section | null;
		/** Switches sections in place; without it, sections are links. */
		onsection?: (id: Section) => void;
	} = $props();

	const visible = $derived(sections.filter((s) => !s.admin || hasRole('admin')));
	const personal = $derived(settingsPages.filter((p) => p.group === 'personal' && hasRole(p.role)));
	const pages = $derived(settingsPages.filter((p) => p.group === 'setup' && hasRole(p.role)));
	const path = $derived(page.url.pathname);
	const cls = (on: boolean) =>
		`flex shrink-0 items-center gap-2 rounded-lg px-3 py-2 text-left text-sm whitespace-nowrap ${
			on
				? 'bg-teal-50 font-medium text-teal-800 dark:bg-teal-900/40 dark:text-teal-200'
				: 'text-slate-700 hover:bg-slate-100 dark:text-slate-300 dark:hover:bg-slate-800'
		}`;
</script>

<nav
	class="-mx-1 flex gap-1 overflow-x-auto px-1 md:sticky md:top-4 md:w-56 md:shrink-0 md:flex-col md:overflow-visible"
	aria-label={t('nav.settings')}
>
	{#each visible as s (s.id)}
		{#if onsection}
			<button
				type="button"
				class={cls(section === s.id)}
				aria-current={section === s.id ? 'page' : undefined}
				onclick={() => onsection(s.id)}
				><span aria-hidden="true" class="w-5 text-center">{s.icon}</span>
				{t(`settings.section.${s.id}`)}</button
			>
		{:else}
			<a href="/settings#{s.id}" class={cls(false)}
				><span aria-hidden="true" class="w-5 text-center">{s.icon}</span>
				{t(`settings.section.${s.id}`)}</a
			>
		{/if}
	{/each}
	{#each personal as p (p.href)}
		{@const on = path === p.href || path.startsWith(p.href + '/')}
		<a href={p.href} class={cls(on)} aria-current={on ? 'page' : undefined}
			><span aria-hidden="true" class="w-5 text-center">{p.icon}</span> {t(p.key)}</a
		>
	{/each}
	{#if pages.length}
		<p
			class="hidden px-3 pt-3 pb-1 text-xs font-semibold tracking-wide text-slate-500 uppercase md:block dark:text-slate-400"
		>
			{t('settings.setup')}
		</p>
		{#each pages as p (p.href)}
			{@const on = path === p.href || path.startsWith(p.href + '/')}
			<a href={p.href} class={cls(on)} aria-current={on ? 'page' : undefined}
				><span aria-hidden="true" class="w-5 text-center">{p.icon}</span> {t(p.key)}</a
			>
		{/each}
	{/if}
</nav>
