<script lang="ts" generics="T extends string">
	let {
		tabs,
		value = $bindable(),
		onchange,
		label,
		variant = 'line'
	}: {
		tabs: { id: T; label: string }[];
		value: T;
		onchange?: (id: T) => void;
		/** Accessible name of the tab list. */
		label?: string;
		/** `line`: page sections; `pills`: small switch inside a card. */
		variant?: 'line' | 'pills';
	} = $props();

	let list: HTMLElement | undefined = $state();

	function select(id: T) {
		value = id;
		onchange?.(id);
	}

	// Arrow keys, Home and End move between tabs (WAI-ARIA tabs pattern).
	function keydown(e: KeyboardEvent) {
		const i = tabs.findIndex((x) => x.id === value);
		const next =
			e.key === 'ArrowRight'
				? (i + 1) % tabs.length
				: e.key === 'ArrowLeft'
					? (i - 1 + tabs.length) % tabs.length
					: e.key === 'Home'
						? 0
						: e.key === 'End'
							? tabs.length - 1
							: -1;
		if (next < 0) return;
		e.preventDefault();
		select(tabs[next].id);
		list?.querySelectorAll<HTMLElement>('[role="tab"]')[next]?.focus();
	}
</script>

<div
	bind:this={list}
	role="tablist"
	tabindex="-1"
	aria-label={label}
	onkeydown={keydown}
	class={variant === 'line'
		? 'scroll-x flex gap-1 shadow-[inset_0_-1px_0_var(--color-slate-200)] dark:shadow-[inset_0_-1px_0_var(--color-slate-800)]'
		: 'flex flex-wrap gap-1'}
>
	{#each tabs as x (x.id)}
		<button
			type="button"
			role="tab"
			aria-selected={value === x.id}
			tabindex={value === x.id ? 0 : -1}
			class={variant === 'line'
				? `shrink-0 border-b-2 px-3 py-2 text-sm whitespace-nowrap ${
						value === x.id
							? 'border-teal-600 font-medium text-teal-800 dark:border-teal-400 dark:text-teal-200'
							: 'border-transparent text-slate-600 hover:border-slate-300 hover:text-slate-900 dark:text-slate-300 dark:hover:text-slate-100'
					}`
				: `btn btn-sm ${value === x.id ? 'btn-primary' : ''}`}
			onclick={() => select(x.id)}>{x.label}</button
		>
	{/each}
</div>
