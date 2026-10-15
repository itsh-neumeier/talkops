<script lang="ts">
	import type { Snippet } from 'svelte';
	import { tick } from 'svelte';
	import { t } from '#lib/i18n/index.svelte.ts';

	/**
	 * Filter button for a table column. The panel opens in the top layer, so
	 * the scroll area of the table cannot clip it.
	 */
	let {
		label,
		active = false,
		onreset,
		children
	}: {
		/** Column name, used in the button label and the panel title. */
		label: string;
		/** A filter of this column is set. */
		active?: boolean;
		onreset?: () => void;
		children: Snippet<[close: () => void]>;
	} = $props();

	const panelId = $props.id();
	let open = $state(false);
	let root = $state<HTMLElement>();
	let trigger = $state<HTMLButtonElement>();
	let panel = $state<HTMLElement>();
	let place = $state({ top: 0, left: 0 });

	/** Below the button; above it near the bottom edge; inside the window. */
	function position() {
		if (!trigger || !panel) return;
		const at = trigger.getBoundingClientRect();
		const gap = 6;
		const margin = 8;
		const below = at.bottom + gap;
		const top =
			below + panel.offsetHeight > innerHeight - margin &&
			at.top - gap - panel.offsetHeight > margin
				? at.top - gap - panel.offsetHeight
				: below;
		const left = Math.min(
			Math.max(margin, at.left),
			Math.max(margin, innerWidth - panel.offsetWidth - margin)
		);
		place = { top, left };
	}

	async function show() {
		open = true;
		await tick();
		if (panel && 'showPopover' in panel && !panel.matches(':popover-open')) panel.showPopover();
		position();
		panel?.querySelector<HTMLElement>('input, select, button:not([data-close])')?.focus();
	}

	function close() {
		if (panel && 'hidePopover' in panel && panel.matches(':popover-open')) panel.hidePopover();
		open = false;
	}

	function outside(event: PointerEvent) {
		const target = event.target as Node;
		if (!root?.contains(target) && !panel?.contains(target)) close();
	}

	$effect(() => {
		if (!open) return;
		document.addEventListener('pointerdown', outside);
		addEventListener('scroll', position, true);
		addEventListener('resize', position);
		return () => {
			document.removeEventListener('pointerdown', outside);
			removeEventListener('scroll', position, true);
			removeEventListener('resize', position);
		};
	});
</script>

<span bind:this={root} class="inline-flex align-middle">
	<button
		bind:this={trigger}
		type="button"
		class="relative inline-flex h-6 w-6 items-center justify-center rounded-md transition-colors duration-150 hover:bg-slate-200/70 focus-visible:outline-2 focus-visible:outline-teal-500 motion-reduce:transition-none dark:hover:bg-slate-700/70 {active
			? 'text-teal-700 dark:text-teal-300'
			: 'text-slate-400 dark:text-slate-500'}"
		aria-haspopup="dialog"
		aria-expanded={open}
		aria-controls={panelId}
		aria-label={t(active ? 'filter.activeLabel' : 'filter.label', { column: label })}
		title={t(active ? 'filter.activeLabel' : 'filter.label', { column: label })}
		onclick={() => (open ? close() : show())}
	>
		<svg viewBox="0 0 20 20" class="h-3.5 w-3.5" fill="currentColor" aria-hidden="true">
			<path
				d="M3 4.5A1.5 1.5 0 0 1 4.5 3h11A1.5 1.5 0 0 1 17 4.5v.88a1.5 1.5 0 0 1-.44 1.06L12 11v4.38a1 1 0 0 1-.55.9l-2 1A1 1 0 0 1 8 16.38V11L3.44 6.44A1.5 1.5 0 0 1 3 5.38V4.5Z"
			/>
		</svg>
		{#if active}
			<span
				class="absolute -top-0.5 -right-0.5 h-2 w-2 rounded-full bg-teal-500 ring-2 ring-white dark:ring-slate-900"
				aria-hidden="true"
			></span>
		{/if}
	</button>
	{#if open}
		<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
		<div
			bind:this={panel}
			id={panelId}
			popover="manual"
			role="dialog"
			tabindex="-1"
			aria-label={t('filter.title', { column: label })}
			class="popover-panel fixed inset-auto z-[60] m-0 w-72 max-w-[calc(100vw-1rem)] rounded-xl border border-slate-200 bg-white p-3 text-sm font-normal text-slate-900 shadow-lg dark:border-slate-700 dark:bg-slate-900 dark:text-slate-100"
			style="top: {place.top}px; left: {place.left}px"
			onkeydown={(e) => {
				if (e.key === 'Escape') {
					e.stopPropagation();
					close();
					trigger?.focus();
				}
			}}
		>
			<p
				class="mb-2 text-xs font-semibold tracking-wide text-slate-500 uppercase dark:text-slate-400"
			>
				{t('filter.title', { column: label })}
			</p>
			{@render children(close)}
			<div
				class="mt-3 flex justify-between gap-2 border-t border-slate-100 pt-3 dark:border-slate-800"
			>
				<button
					type="button"
					class="btn btn-sm"
					disabled={!active}
					onclick={() => {
						onreset?.();
						close();
					}}>{t('filter.reset')}</button
				>
				<button type="button" class="btn btn-sm btn-primary" data-close onclick={close}
					>{t('common.close')}</button
				>
			</div>
		</div>
	{/if}
</span>
