<script lang="ts" module>
	export interface ComboOption {
		value: string;
		label: string;
		/** Second line, for example an assignment. */
		hint?: string;
	}
</script>

<script lang="ts">
	// A select with a search field (ARIA combobox): typing filters the
	// options; arrow keys, Enter and Escape work as in a select.
	import { t } from '#lib/i18n/index.svelte.ts';

	let {
		options,
		value = $bindable(''),
		label,
		placeholder = '',
		id: idProp
	}: {
		options: ComboOption[];
		value?: string;
		label: string;
		placeholder?: string;
		id?: string;
	} = $props();
	const uid = $props.id();
	const id = $derived(idProp ?? uid);

	/** Lower case without accents, so "muller" finds "Müller". */
	const fold = (s: string) => s.toLowerCase().normalize('NFD').replace(/\p{M}/gu, '');

	let query = $state('');
	let open = $state(false);
	let active = $state(0);
	const selected = $derived(options.find((o) => o.value === value));
	const shown = $derived(
		query.trim()
			? options.filter((o) => fold(`${o.label} ${o.hint ?? ''}`).includes(fold(query.trim())))
			: options
	);

	function choose(o: ComboOption) {
		value = o.value;
		query = '';
		open = false;
	}
	function onkeydown(e: KeyboardEvent) {
		if (e.key === 'ArrowDown') {
			e.preventDefault();
			if (!open) open = true;
			else active = Math.min(shown.length - 1, active + 1);
		} else if (e.key === 'ArrowUp') {
			e.preventDefault();
			active = Math.max(0, active - 1);
		} else if (e.key === 'Enter' && open && shown[active]) {
			e.preventDefault();
			choose(shown[active]);
		} else if (e.key === 'Escape' && open) {
			e.preventDefault();
			e.stopPropagation();
			open = false;
			query = '';
		}
	}
</script>

<div class="relative">
	<label for={id}>{label}</label>
	<div class="relative">
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
			{id}
			class="input pl-9"
			role="combobox"
			autocomplete="off"
			aria-expanded={open}
			aria-controls="{id}-list"
			aria-autocomplete="list"
			aria-activedescendant={open && shown[active] ? `${id}-opt-${active}` : undefined}
			placeholder={selected?.label ?? placeholder}
			value={open ? query : (selected?.label ?? '')}
			oninput={(e) => {
				query = e.currentTarget.value;
				active = 0;
				open = true;
			}}
			onfocus={() => (open = true)}
			onblur={() => setTimeout(() => ((open = false), (query = '')), 150)}
			{onkeydown}
		/>
	</div>
	{#if open}
		<ul
			id="{id}-list"
			role="listbox"
			aria-label={label}
			class="menu-in absolute top-full right-0 left-0 z-50 mt-1 max-h-64 overflow-y-auto rounded-xl border border-slate-200 bg-white p-1 shadow-lg dark:border-slate-700 dark:bg-slate-900"
		>
			{#each shown as o, i (o.value)}
				<!-- The combobox input handles the keys (ARIA combobox pattern). -->
				<!-- svelte-ignore a11y_click_events_have_key_events -->
				<li
					id="{id}-opt-{i}"
					role="option"
					aria-selected={o.value === value}
					class="cursor-pointer rounded-lg px-2.5 py-2 text-sm {i === active
						? 'bg-teal-50 dark:bg-teal-900/40'
						: ''}"
					onmousedown={(e) => e.preventDefault()}
					onclick={() => choose(o)}
					onmouseenter={() => (active = i)}
				>
					<span class="block">{o.label}</span>
					{#if o.hint}<span class="block text-xs text-slate-500 dark:text-slate-400">{o.hint}</span
						>{/if}
				</li>
			{:else}
				<li class="px-2.5 py-2 text-sm text-slate-500">{t('common.noMatches')}</li>
			{/each}
		</ul>
	{/if}
</div>
