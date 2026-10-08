<script lang="ts">
	// Picks extensions (people to ring, voicemail recipients): chosen ones as
	// chips, the rest searchable below.
	import { targets } from '#lib/destinations.svelte.ts';
	import { t } from '#lib/i18n/index.svelte.ts';

	let { selected = $bindable(), label }: { selected: string[]; label: string } = $props();
	const ids = $props.id();
	let query = $state('');

	const chosen = $derived(
		selected.map((id) => targets.extensions.find((e) => e.id === id)).filter((e) => e !== undefined)
	);
	const available = $derived(
		targets.extensions.filter(
			(e) =>
				!selected.includes(e.id) &&
				`${e.number} ${e.display_name}`.toLowerCase().includes(query.trim().toLowerCase())
		)
	);
</script>

<div class="space-y-2">
	<label for="{ids}-q">{label}</label>
	{#if chosen.length > 0}
		<ul class="flex flex-wrap gap-1">
			{#each chosen as e (e.id)}
				<li
					class="inline-flex items-center gap-1 rounded-full bg-teal-600/10 py-0.5 pr-1 pl-2 text-sm text-teal-800 dark:text-teal-200"
				>
					<span class="font-mono">{e.number}</span>
					{e.display_name}
					<button
						type="button"
						class="rounded-full px-1 hover:bg-teal-600/20"
						onclick={() => (selected = selected.filter((x) => x !== e.id))}
						aria-label={t('common.delete')}>✕</button
					>
				</li>
			{/each}
		</ul>
	{/if}
	<input
		id="{ids}-q"
		class="input"
		type="search"
		bind:value={query}
		placeholder={t('flow.searchExtensions')}
	/>
	<ul
		class="max-h-48 divide-y divide-slate-100 overflow-y-auto rounded-lg border border-slate-200 dark:divide-slate-800 dark:border-slate-700"
	>
		{#each available as e (e.id)}
			<li>
				<button
					type="button"
					class="flex w-full items-center gap-2 px-3 py-1.5 text-left text-sm hover:bg-slate-50 dark:hover:bg-slate-800"
					onclick={() => (selected = [...selected, e.id])}
				>
					<span class="w-10 font-mono text-slate-500">{e.number}</span>
					<span class="flex-1">{e.display_name}</span>
					<span class="text-teal-700 dark:text-teal-300">+</span>
				</button>
			</li>
		{:else}
			<li class="px-3 py-1.5 text-sm text-slate-500">{t('flow.noExtensions')}</li>
		{/each}
	</ul>
</div>
