<script lang="ts">
	// Search field of the settings navigation (like Android settings). Finds
	// sections, headings inside them and setup pages, opens the target and
	// marks the heading. "/" or Ctrl+K moves the focus to the field.
	import { goto } from '$app/navigation';
	import { tick } from 'svelte';
	import { t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { can } from '#lib/session.svelte.ts';
	import type { Permission } from '#lib/api.ts';
	import {
		buildIndex,
		entryUrl,
		fold,
		searchSettings,
		type SettingsEntry
	} from '#lib/settingsIndex.ts';
	import { sections, settingsPages } from './SettingsNav.svelte';

	const index = buildIndex(sections, settingsPages);
	const allowed = (perms: Permission[]) => perms.length === 0 || perms.some((p) => can(p));

	let query = $state('');
	let open = $state(false);
	let active = $state(0);
	let input: HTMLInputElement | undefined = $state();
	const hits = $derived(searchSettings(query, index, (k) => t(k), allowed));

	$effect(() => {
		void query;
		active = 0;
	});

	function path(entry: SettingsEntry): string {
		const parts = [entry.parent ? t(entry.parent) : t('nav.settings')];
		if (entry.tab && entry.section === 'system')
			parts.push(t(`system.tab.${entry.tab}` as MessageKey));
		return parts.join(' › ');
	}

	/** Waits for the heading of the target, scrolls to it and marks it. */
	async function mark(heading: MessageKey) {
		const text = fold(t(heading));
		for (let i = 0; i < 40; i++) {
			await tick();
			const el = [...document.querySelectorAll<HTMLElement>('main h2, main h3')].find(
				(h) => fold(h.textContent?.trim() ?? '') === text
			);
			if (el) {
				const target = el.closest<HTMLElement>('.card') ?? el;
				target.scrollIntoView({ block: 'start', behavior: 'smooth' });
				target.classList.remove('search-hit');
				void target.offsetWidth;
				target.classList.add('search-hit');
				setTimeout(() => target.classList.remove('search-hit'), 2000);
				return;
			}
			await new Promise((r) => setTimeout(r, 50));
		}
	}

	async function choose(entry: SettingsEntry) {
		open = false;
		query = '';
		input?.blur();
		await goto(entryUrl(entry));
		if (entry.heading) await mark(entry.heading);
	}

	function onkeydown(e: KeyboardEvent) {
		if (e.key === 'ArrowDown' && hits.length) {
			e.preventDefault();
			open = true;
			active = (active + 1) % hits.length;
		} else if (e.key === 'ArrowUp' && hits.length) {
			e.preventDefault();
			active = (active - 1 + hits.length) % hits.length;
		} else if (e.key === 'Enter' && hits[active]) {
			e.preventDefault();
			void choose(hits[active].entry);
		} else if (e.key === 'Escape') {
			if (query) query = '';
			else input?.blur();
			open = false;
		}
	}

	function onglobalkey(e: KeyboardEvent) {
		const el = e.target as HTMLElement | null;
		const typing =
			el?.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(el?.tagName ?? '');
		if ((e.key === '/' && !typing) || (e.key === 'k' && (e.ctrlKey || e.metaKey))) {
			e.preventDefault();
			input?.focus();
		}
	}
</script>

<svelte:window onkeydown={onglobalkey} />

<div class="relative">
	<label for="settings-search" class="sr-only">{t('settings.search.label')}</label>
	<svg
		viewBox="0 0 24 24"
		class="pointer-events-none absolute top-1/2 left-3 h-4 w-4 -translate-y-1/2 text-slate-400"
		fill="none"
		stroke="currentColor"
		stroke-width="2"
		stroke-linecap="round"
		aria-hidden="true"><circle cx="11" cy="11" r="7" /><path d="m20 20-3.5-3.5" /></svg
	>
	<input
		id="settings-search"
		bind:this={input}
		bind:value={query}
		type="search"
		class="input pr-8 pl-9"
		placeholder={t('settings.search.placeholder')}
		autocomplete="off"
		role="combobox"
		aria-expanded={open && query.trim() !== ''}
		aria-controls="settings-search-results"
		aria-autocomplete="list"
		aria-activedescendant={open && hits[active] ? `settings-hit-${active}` : undefined}
		onfocus={() => (open = true)}
		onblur={() => setTimeout(() => (open = false), 150)}
		oninput={() => (open = true)}
		{onkeydown}
	/>
	{#if !query}
		<kbd
			class="pointer-events-none absolute top-1/2 right-2 hidden -translate-y-1/2 rounded border border-slate-300 px-1.5 text-xs text-slate-400 lg:block dark:border-slate-600"
			aria-hidden="true">/</kbd
		>
	{/if}
	{#if open && query.trim() !== ''}
		<ul
			id="settings-search-results"
			role="listbox"
			aria-label={t('settings.search.label')}
			class="menu-in absolute top-full right-0 left-0 z-30 mt-1 max-h-96 overflow-y-auto rounded-xl border border-slate-200 bg-white p-1 shadow-lg lg:w-80 dark:border-slate-700 dark:bg-slate-900"
		>
			{#each hits as hit, i (hit.entry.id)}
				<!-- The combobox input handles the keys (ARIA combobox pattern). -->
				<!-- svelte-ignore a11y_click_events_have_key_events -->
				<li
					id="settings-hit-{i}"
					role="option"
					aria-selected={i === active}
					class="flex cursor-pointer items-start gap-2 rounded-lg px-2.5 py-2 text-sm {i === active
						? 'bg-teal-50 dark:bg-teal-900/40'
						: ''}"
					onmousedown={(e) => e.preventDefault()}
					onclick={() => choose(hit.entry)}
					onmouseenter={() => (active = i)}
				>
					<span aria-hidden="true" class="w-5 shrink-0 text-center">{hit.entry.icon}</span>
					<span class="min-w-0">
						<span class="block truncate font-medium">{t(hit.entry.title)}</span>
						<span class="block truncate text-xs text-slate-500 dark:text-slate-400"
							>{path(hit.entry)}</span
						>
					</span>
				</li>
			{:else}
				<li class="px-2.5 py-2 text-sm text-slate-500 dark:text-slate-400">
					{t('settings.search.none')}
				</li>
			{/each}
		</ul>
	{/if}
</div>
