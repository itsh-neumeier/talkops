<script lang="ts" module>
	export interface PanelTab {
		id: string;
		label: string;
		/** SVG path data of a 24×24 stroke icon. */
		icon: string;
	}
</script>

<script lang="ts">
	// Details of a list entry beside the list, like the inspector of UniFi
	// Talk: a header with the name, icon tabs and the content of the tab.
	// Wide screens keep the list visible; narrow screens show the panel in
	// full. Escape and the close button close it; the focus goes back to
	// the element that opened it.
	import type { Snippet } from 'svelte';
	import { fly } from 'svelte/transition';
	import { cubicOut } from 'svelte/easing';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { motion } from '#lib/motion.ts';

	let {
		title,
		subtitle = '',
		tabs,
		tab = $bindable(),
		onclose,
		children
	}: {
		title: string;
		subtitle?: string;
		tabs: PanelTab[];
		tab: string;
		onclose: () => void;
		children: Snippet;
	} = $props();

	let panel: HTMLElement | undefined = $state();
	const opener = typeof document === 'undefined' ? null : document.activeElement;
	$effect(() => {
		panel?.focus();
		return () => {
			if (opener instanceof HTMLElement && opener.isConnected) opener.focus();
		};
	});

	function onkeydown(e: KeyboardEvent) {
		if (e.key !== 'Escape' || e.defaultPrevented) return;
		// A dialog above the panel closes first.
		if (document.querySelector('[role="dialog"][aria-modal="true"]')) return;
		onclose();
	}
	function tabKey(e: KeyboardEvent, i: number) {
		if (e.key !== 'ArrowRight' && e.key !== 'ArrowLeft') return;
		e.preventDefault();
		const next = tabs[(i + (e.key === 'ArrowRight' ? 1 : tabs.length - 1)) % tabs.length];
		tab = next.id;
		panel?.querySelector<HTMLElement>(`#panel-tab-${next.id}`)?.focus();
	}
</script>

<svelte:window {onkeydown} />

<aside
	bind:this={panel}
	tabindex="-1"
	class="side-panel @container fixed inset-0 z-30 flex flex-col border-slate-200 bg-white shadow-2xl outline-none lg:top-[var(--header-h)] lg:bottom-(--footer-h) lg:left-auto lg:w-(--side-panel-w) lg:border-l dark:border-slate-800 dark:bg-slate-900"
	aria-label={title}
	transition:fly={{ x: 24, duration: motion(220), easing: cubicOut }}
	data-testid="side-panel"
>
	<header class="flex items-start justify-between gap-3 px-4 pt-4">
		<div class="min-w-0">
			<h2 class="truncate">{title}</h2>
			{#if subtitle}<p class="truncate text-sm text-slate-500 dark:text-slate-400">
					{subtitle}
				</p>{/if}
		</div>
		<button class="btn btn-sm shrink-0" onclick={onclose} aria-label={t('common.close')}>✕</button>
	</header>
	<div
		class="mx-4 mt-3 grid gap-1 rounded-xl bg-slate-100 p-1 dark:bg-slate-800"
		style="grid-template-columns: repeat({tabs.length}, minmax(0, 1fr))"
		role="tablist"
		aria-label={title}
	>
		{#each tabs as item, i (item.id)}
			<button
				id="panel-tab-{item.id}"
				type="button"
				role="tab"
				aria-selected={tab === item.id}
				aria-controls="panel-body"
				tabindex={tab === item.id ? 0 : -1}
				title={item.label}
				class="flex items-center justify-center gap-1.5 rounded-lg px-2 py-1.5 text-xs font-medium transition-colors {tab ===
				item.id
					? 'bg-white text-teal-700 shadow-sm dark:bg-slate-900 dark:text-teal-300'
					: 'text-slate-500 hover:text-slate-700 dark:text-slate-400 dark:hover:text-slate-200'}"
				onclick={() => (tab = item.id)}
				onkeydown={(e) => tabKey(e, i)}
			>
				<svg
					viewBox="0 0 24 24"
					class="h-4 w-4 shrink-0"
					fill="none"
					stroke="currentColor"
					stroke-width="2"
					stroke-linecap="round"
					stroke-linejoin="round"
					aria-hidden="true"><path d={item.icon} /></svg
				>
				<span class="truncate">{item.label}</span>
			</button>
		{/each}
	</div>
	<div
		id="panel-body"
		role="tabpanel"
		aria-labelledby="panel-tab-{tab}"
		class="min-h-0 flex-1 space-y-3 overflow-y-auto p-4"
	>
		{@render children()}
	</div>
</aside>
