<script lang="ts">
	import { targets } from '#lib/destinations.svelte.ts';
	import { t } from '#lib/i18n/index.svelte.ts';

	let { members = $bindable() }: { members: string[] } = $props();
	let add = $state('');

	const ext = (id: string) => targets.extensions.find((e) => e.id === id);
	const available = $derived(targets.extensions.filter((e) => !members.includes(e.id)));

	function move(i: number, d: number) {
		const next = [...members];
		[next[i], next[i + d]] = [next[i + d], next[i]];
		members = next;
	}
</script>

<div class="space-y-2">
	<ol class="space-y-1">
		{#each members as m, i (m)}
			<li
				class="flex items-center justify-between gap-2 rounded border border-slate-200 px-2 py-1 dark:border-slate-700"
			>
				<span
					><span class="text-slate-500">{i + 1}.</span>
					{ext(m)?.number}
					{ext(m)?.display_name}</span
				>
				<span class="space-x-1">
					<button
						type="button"
						class="btn btn-sm"
						disabled={i === 0}
						onclick={() => move(i, -1)}
						aria-label={t('routing.up')}>↑</button
					>
					<button
						type="button"
						class="btn btn-sm"
						disabled={i === members.length - 1}
						onclick={() => move(i, 1)}
						aria-label={t('routing.down')}>↓</button
					>
					<button
						type="button"
						class="btn btn-sm btn-danger"
						onclick={() => (members = members.filter((x) => x !== m))}
						aria-label={t('common.delete')}>✕</button
					>
				</span>
			</li>
		{/each}
	</ol>
	{#if available.length > 0}
		<div class="flex gap-2">
			<select class="input" bind:value={add} aria-label={t('routing.addMember')}>
				<option value="">{t('routing.addMember')}</option>
				{#each available as e (e.id)}<option value={e.id}>{e.number} {e.display_name}</option
					>{/each}
			</select>
			<button
				type="button"
				class="btn"
				disabled={!add}
				onclick={() => {
					members = [...members, add];
					add = '';
				}}>{t('common.add')}</button
			>
		</div>
	{/if}
</div>
