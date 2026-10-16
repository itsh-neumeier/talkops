<script lang="ts" module>
	import type { Permission } from '#lib/api.ts';
	import { can } from '#lib/session.svelte.ts';

	export type Section =
		| 'telephony'
		| 'calls'
		| 'voicemail'
		| 'email'
		| 'security'
		| 'system'
		| 'backup'
		| 'storage'
		| 'account';
	/** Settings sections; a section shows when the user has one of `perms`. */
	export const sections: { id: Section; icon: string; perms: Permission[] }[] = [
		{ id: 'telephony', icon: '☎', perms: ['settings.manage'] },
		{ id: 'calls', icon: '🎵', perms: ['settings.manage'] },
		{ id: 'voicemail', icon: '📮', perms: ['voicemail.settings'] },
		{ id: 'email', icon: '✉', perms: ['voicemail.settings'] },
		{ id: 'security', icon: '🛡', perms: ['identity.manage', 'security.manage'] },
		{ id: 'system', icon: '⚙', perms: ['diagnostics.manage'] },
		{ id: 'backup', icon: '💾', perms: ['backups.manage'] },
		{ id: 'storage', icon: '🗄', perms: ['storage.manage'] },
		{ id: 'account', icon: '👤', perms: [] }
	];

	export function sectionVisible(s: { perms: Permission[] }): boolean {
		return s.perms.length === 0 || s.perms.some((p) => can(p));
	}
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
		/** Needed permission; none for personal pages. */
		perm: Permission | null;
		group: 'personal' | 'setup';
	}[] = [
		{ href: '/me', key: 'nav.myPhones', icon: '📱', perm: null, group: 'personal' },
		{
			href: '/extensions',
			key: 'nav.extensions',
			icon: '☏',
			perm: 'extensions.view',
			group: 'setup'
		},
		{ href: '/users', key: 'nav.users', icon: '👥', perm: 'users.view', group: 'setup' },
		{ href: '/routing', key: 'nav.routing', icon: '🔀', perm: 'routing.view', group: 'setup' },
		{ href: '/trunks', key: 'nav.trunks', icon: '🔌', perm: 'trunks.view', group: 'setup' },
		{ href: '/numbers', key: 'nav.numbers', icon: '#', perm: 'trunks.view', group: 'setup' },
		{ href: '/phones', key: 'nav.phones', icon: '📟', perm: 'phones.view', group: 'setup' },
		{
			href: '/door-stations',
			key: 'nav.doorStations',
			icon: '🚪',
			perm: 'doors.manage',
			group: 'setup'
		},
		{ href: '/audit', key: 'nav.audit', icon: '📜', perm: 'audit.view', group: 'setup' }
	];
</script>

<script lang="ts">
	import { page } from '$app/state';
	import { t } from '#lib/i18n/index.svelte.ts';

	let {
		section = null,
		onsection
	}: {
		/** Active section on the settings page itself (null elsewhere). */
		section?: Section | null;
		/** Switches sections in place; without it, sections are links. */
		onsection?: (id: Section) => void;
	} = $props();

	const visible = $derived(sections.filter(sectionVisible));
	const allowed = (p: { perm: Permission | null }) => !p.perm || can(p.perm);
	const personal = $derived(settingsPages.filter((p) => p.group === 'personal' && allowed(p)));
	const pages = $derived(settingsPages.filter((p) => p.group === 'setup' && allowed(p)));
	const path = $derived(page.url.pathname);

	// In the horizontal strip (narrow screens) keep the active entry visible.
	let strip: HTMLElement | undefined = $state();
	$effect(() => {
		void path;
		void section;
		const el = strip?.querySelector('[aria-current="page"]');
		if (!strip || !el || strip.scrollWidth <= strip.clientWidth) return;
		const a = el.getBoundingClientRect();
		const b = strip.getBoundingClientRect();
		strip.scrollLeft += a.left - b.left - (b.width - a.width) / 2;
	});
</script>

<nav
	bind:this={strip}
	class="scroll-x -mx-1 flex gap-1 px-1 pb-1 lg:sticky lg:top-[calc(var(--header-h)+1rem)] lg:w-56 lg:shrink-0 lg:flex-col lg:overflow-visible lg:bg-none lg:pb-0"
	aria-label={t('nav.settings')}
>
	{#each visible as s (s.id)}
		{#if onsection}
			<button
				type="button"
				class="nav-item"
				aria-current={section === s.id ? 'page' : undefined}
				onclick={() => onsection(s.id)}
				><span aria-hidden="true" class="w-5 text-center">{s.icon}</span>
				{t(`settings.section.${s.id}`)}</button
			>
		{:else}
			<a href="/settings?section={s.id}" class="nav-item"
				><span aria-hidden="true" class="w-5 text-center">{s.icon}</span>
				{t(`settings.section.${s.id}`)}</a
			>
		{/if}
	{/each}
	{#each personal as p (p.href)}
		{@const on = path === p.href || path.startsWith(p.href + '/')}
		<a href={p.href} class="nav-item" aria-current={on ? 'page' : undefined}
			><span aria-hidden="true" class="w-5 text-center">{p.icon}</span> {t(p.key)}</a
		>
	{/each}
	{#if pages.length}
		<p
			class="hidden px-3 pt-3 pb-1 text-xs font-semibold tracking-wide text-slate-500 uppercase lg:block dark:text-slate-400"
		>
			{t('settings.setup')}
		</p>
		{#each pages as p (p.href)}
			{@const on = path === p.href || path.startsWith(p.href + '/')}
			<a href={p.href} class="nav-item" aria-current={on ? 'page' : undefined}
				><span aria-hidden="true" class="w-5 text-center">{p.icon}</span> {t(p.key)}</a
			>
		{/each}
	{/if}
</nav>
