<script lang="ts">
	import '../app.css';
	import { i18n, locales, setLocale, t, type Locale } from '#lib/i18n/index.svelte.ts';
	import { theme, toggleTheme } from '#lib/theme.svelte.ts';

	let { children } = $props();
</script>

<svelte:head>
	<title>TalkOps</title>
</svelte:head>

<div class="min-h-screen">
	<header
		class="flex items-center justify-between gap-4 border-b border-slate-200 bg-white px-4 py-3 dark:border-slate-800 dark:bg-slate-900"
	>
		<div class="flex items-center gap-3">
			<img src="/favicon.svg" alt="" class="h-8 w-8" />
			<div>
				<div class="font-semibold">TalkOps</div>
				<div class="text-xs text-slate-500 dark:text-slate-400">{t('app.tagline')}</div>
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
				class="rounded-md border border-slate-300 px-2 py-1 text-sm dark:border-slate-700"
				aria-label={t('theme.toggle')}
				title={t('theme.toggle')}
				onclick={toggleTheme}
			>
				{theme.dark ? '☀' : '☾'}
			</button>
		</div>
	</header>
	<main class="mx-auto max-w-5xl p-4">
		{@render children()}
	</main>
</div>
