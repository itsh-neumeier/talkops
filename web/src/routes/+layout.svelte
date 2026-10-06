<script lang="ts">
	import '../app.css';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import { api } from '#lib/api.ts';
	import {
		i18n,
		locales,
		setLocale,
		t,
		type Locale,
		type MessageKey
	} from '#lib/i18n/index.svelte.ts';
	import { hasRole, loadSession, logout, session } from '#lib/session.svelte.ts';
	import { theme, toggleTheme } from '#lib/theme.svelte.ts';

	let { children } = $props();
	let menuOpen = $state(false);

	const publicPaths = ['/login', '/setup'];

	const nav: { href: string; key: MessageKey; role: 'admin' | 'operator' | 'user' }[] = [
		{ href: '/', key: 'nav.dashboard', role: 'user' },
		{ href: '/me', key: 'nav.myPhones', role: 'user' },
		{ href: '/voicemail', key: 'nav.voicemail', role: 'user' },
		{ href: '/doors', key: 'nav.doors', role: 'user' },
		{ href: '/extensions', key: 'nav.extensions', role: 'operator' },
		{ href: '/users', key: 'nav.users', role: 'operator' },
		{ href: '/trunks', key: 'nav.trunks', role: 'operator' },
		{ href: '/numbers', key: 'nav.numbers', role: 'operator' },
		{ href: '/routing', key: 'nav.routing', role: 'operator' },
		{ href: '/phones', key: 'nav.phones', role: 'operator' },
		{ href: '/phonebook', key: 'nav.phonebook', role: 'user' },
		{ href: '/calls', key: 'nav.calls', role: 'user' },
		{ href: '/search', key: 'nav.search', role: 'user' },
		{ href: '/settings', key: 'nav.settings', role: 'user' },
		{ href: '/audit', key: 'nav.audit', role: 'admin' }
	];

	onMount(async () => {
		document.documentElement.lang = i18n.locale;
		await loadSession();
		const path = page.url.pathname;
		if (!session.user && !publicPaths.includes(path)) {
			const { needs_setup } = await api.get<{ needs_setup: boolean }>('/setup');
			goto(needs_setup ? '/setup' : '/login');
		}
	});

	function active(href: string) {
		const path = page.url.pathname;
		return href === '/' ? path === '/' : path.startsWith(href);
	}
</script>

<svelte:head>
	<title>TalkOps</title>
</svelte:head>

<div class="min-h-screen">
	<header
		class="sticky top-0 z-30 flex items-center justify-between gap-4 border-b border-slate-200 bg-white px-4 py-3 dark:border-slate-800 dark:bg-slate-900"
	>
		<div class="flex items-center gap-3">
			{#if session.user}
				<button
					class="btn btn-sm md:hidden"
					aria-label={t('nav.menu')}
					onclick={() => (menuOpen = !menuOpen)}>☰</button
				>
			{/if}
			<img src="/favicon.svg" alt="" class="h-8 w-8" />
			<div>
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

	<div class="mx-auto flex max-w-7xl">
		{#if session.user}
			<nav
				class="{menuOpen
					? 'block'
					: 'hidden'} fixed inset-x-0 top-[57px] z-20 border-b border-slate-200 bg-white p-2 md:sticky md:top-[57px] md:block md:h-[calc(100vh-57px)] md:w-56 md:shrink-0 md:border-r md:border-b-0 md:bg-transparent dark:border-slate-800 dark:bg-slate-900 md:dark:bg-transparent"
			>
				{#each nav.filter((n) => hasRole(n.role)) as item (item.href)}
					<a
						href={item.href}
						onclick={() => (menuOpen = false)}
						class="block rounded-md px-3 py-2 text-sm {active(item.href)
							? 'bg-teal-50 font-medium text-teal-800 dark:bg-teal-950 dark:text-teal-200'
							: 'hover:bg-slate-100 dark:hover:bg-slate-800'}">{t(item.key)}</a
					>
				{/each}
			</nav>
		{/if}
		<main class="min-w-0 flex-1 p-4">
			{#if session.loaded && (session.user || publicPaths.includes(page.url.pathname))}
				{@render children()}
			{:else}
				<p class="text-slate-500">{t('common.loading')}</p>
			{/if}
		</main>
	</div>
</div>
