<script lang="ts">
	import { settingsPages } from '#lib/components/SettingsNav.svelte';
	import '../app.css';
	import { afterNavigate, goto } from '$app/navigation';
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import { api, type Permission } from '#lib/api.ts';
	import {
		i18n,
		locales,
		setLocale,
		t,
		type Locale,
		type MessageKey
	} from '#lib/i18n/index.svelte.ts';
	import { can, hasRole, loadSession, logout, session } from '#lib/session.svelte.ts';
	import { theme, toggleTheme } from '#lib/theme.svelte.ts';
	import Skeleton from '#lib/components/Skeleton.svelte';
	import { pageChanged } from '#lib/net.svelte.ts';
	import Softphone from '#lib/components/Softphone.svelte';
	import AppFooter from '#lib/components/AppFooter.svelte';

	let { children } = $props();
	let menuOpen = $state(false);
	let phoneOwner = $state('');
	$effect(() => {
		if (!session.user) phoneOwner = '';
		else if (page.url.pathname === '/phone') phoneOwner = session.user.id;
	});

	const publicPaths = ['/login', '/setup'];

	const nav: {
		href: string;
		key: MessageKey;
		role: 'admin' | 'operator' | 'user';
		perms?: Permission[];
	}[] = [
		{ href: '/', key: 'nav.dashboard', role: 'user' },
		{ href: '/phone', key: 'nav.phone', role: 'user' },
		{ href: '/calls', key: 'nav.calls', role: 'user' },
		{ href: '/voicemail', key: 'nav.voicemail', role: 'user' },
		{ href: '/fax', key: 'fax.title', role: 'user' },
		{ href: '/doors', key: 'nav.doors', role: 'user' },
		{ href: '/phonebook', key: 'nav.phonebook', role: 'user' },
		{ href: '/search', key: 'nav.search', role: 'user' },
		{
			href: '/transcriptions',
			key: 'nav.transcriptions',
			role: 'user',
			perms: ['transcriptions.view', 'transcriptions.manage']
		},
		{ href: '/settings', key: 'nav.settings', role: 'user' }
	];

	/** "Door" only appears in the menu once a door station exists. */
	let hasDoors = $state(false);
	const shown = (href: string) => href !== '/doors' || hasDoors;

	// Skeletons until the new page has its data.
	afterNavigate(pageChanged);

	onMount(async () => {
		document.documentElement.lang = i18n.locale;
		await loadSession();
		const path = page.url.pathname;
		if (!session.user && !publicPaths.includes(path)) {
			const { needs_setup } = await api.get<{ needs_setup: boolean }>('/setup');
			goto(needs_setup ? '/setup' : '/login');
		}
	});

	// Re-checked on every page change, so a new door station shows up at once.
	$effect(() => {
		void page.url.pathname;
		if (!session.user) return;
		api
			.get<unknown[]>('/door-stations')
			.then((list) => (hasDoors = list.length > 0))
			.catch(() => {});
	});

	function active(href: string) {
		const path = page.url.pathname;
		const under = (h: string) => path === h || path.startsWith(h + '/');
		// Setup pages are part of the settings area.
		if (href === '/settings') return under(href) || settingsPages.some((p) => under(p.href));
		return href === '/' ? path === '/' : under(href);
	}
</script>

<svelte:head>
	<title>TalkOps</title>
</svelte:head>

<!-- Escape closes the mobile menu. -->
<svelte:window onkeydown={(e) => menuOpen && e.key === 'Escape' && (menuOpen = false)} />

<div class="min-h-dvh">
	<header
		class="sticky top-0 z-30 flex h-(--header-h) items-center justify-between gap-4 border-b border-slate-200 bg-white px-4 dark:border-slate-800 dark:bg-slate-900"
	>
		<div class="flex items-center gap-3">
			{#if session.user}
				<button
					class="btn btn-sm md:hidden"
					aria-label={t('nav.menu')}
					aria-expanded={menuOpen}
					onclick={() => (menuOpen = !menuOpen)}>☰</button
				>
			{/if}
			<img src="/favicon.svg" alt="" class="h-8 w-8" />
			<div class="hidden sm:block">
				<div class="font-semibold">TalkOps</div>
				<div class="hidden text-xs text-slate-500 sm:block dark:text-slate-400">
					{t('app.tagline')}
				</div>
			</div>
		</div>
		<div class="flex items-center gap-2">
			<label class="sr-only" for="locale">{t('locale.label')}</label>
			<select
				id="locale"
				class="rounded-md border border-slate-300 bg-transparent px-2 py-1 text-sm dark:border-slate-700"
				value={i18n.locale}
				onchange={(e) => setLocale(e.currentTarget.value as Locale)}
			>
				{#each Object.keys(locales) as code (code)}
					<option value={code}>{code.toUpperCase()}</option>
				{/each}
			</select>
			<button
				type="button"
				class="btn btn-sm"
				aria-label={t('theme.toggle')}
				title={t('theme.toggle')}
				onclick={toggleTheme}
			>
				{theme.dark ? '☀' : '☾'}
			</button>
			{#if session.user}
				<span class="hidden text-sm text-slate-500 sm:inline">{session.user.display_name}</span>
				<button class="btn btn-sm whitespace-nowrap" onclick={logout}>{t('nav.logout')}</button>
			{/if}
		</div>
	</header>

	<div class="app-body">
		<div class="mx-auto flex max-w-7xl">
			{#if session.user}
				<nav
					class="{menuOpen
						? 'menu-in flex'
						: 'hidden'} fixed inset-x-0 top-(--header-h) z-20 max-h-[calc(100dvh-var(--header-h)-var(--footer-h))] flex-col gap-1 overflow-y-auto border-b border-slate-200 bg-white p-2 shadow-lg md:sticky md:flex md:h-[calc(100dvh-var(--header-h)-var(--footer-h))] md:max-h-none md:w-56 md:shrink-0 md:border-r md:border-b-0 md:bg-transparent md:shadow-none dark:border-slate-800 dark:bg-slate-900 md:dark:bg-transparent"
					aria-label={t('nav.menu')}
				>
					{#each nav.filter((n) => hasRole(n.role) && (!n.perms || n.perms.some( (p) => can(p) )) && shown(n.href)) as item (item.href)}
						<a
							href={item.href}
							onclick={() => (menuOpen = false)}
							class="nav-item"
							aria-current={active(item.href) ? 'page' : undefined}>{t(item.key)}</a
						>
					{/each}
				</nav>
			{/if}
			<main
				class="@container min-w-0 flex-1 p-4 {session.user
					? 'pb-[calc(var(--footer-h)+1rem)]'
					: ''}"
			>
				{#if session.loaded && (session.user || publicPaths.includes(page.url.pathname))}
					{#key page.url.pathname}
						<div class="page-in">{@render children()}</div>
					{/key}
					{#if session.user && phoneOwner === session.user.id}
						{#key phoneOwner}<Softphone full={page.url.pathname === '/phone'} />{/key}
					{/if}
				{:else}
					<div class="space-y-4" aria-label={t('common.loading')}>
						<div class="skeleton"><div class="skeleton-bar h-7 w-48"></div></div>
						<div class="card"><Skeleton lines={6} /></div>
					</div>
				{/if}
			</main>
		</div>
	</div>
	{#if session.user}<AppFooter />{/if}
</div>
