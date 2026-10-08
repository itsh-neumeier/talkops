<script lang="ts">
	import { onMount } from 'svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Attendants from '#lib/components/routing/Attendants.svelte';
	import Queues from '#lib/components/routing/Queues.svelte';
	import RingGroups from '#lib/components/routing/RingGroups.svelte';
	import TimeConditions from '#lib/components/routing/TimeConditions.svelte';
	import { loadTargets } from '#lib/destinations.svelte.ts';
	import { t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	const tabs: { id: string; key: MessageKey }[] = [
		{ id: 'groups', key: 'routing.groups' },
		{ id: 'schedules', key: 'routing.schedules' },
		{ id: 'menus', key: 'routing.menus' },
		{ id: 'queues', key: 'routing.queues' }
	];
	let tab = $state('groups');
	let loaded = $state(false);
	let error = $state('');

	onMount(async () => {
		try {
			const saved = localStorage.getItem('talkops.routingTab');
			if (saved && tabs.some((x) => x.id === saved)) tab = saved;
		} catch {
			// storage unavailable
		}
		try {
			await loadTargets();
			loaded = true;
		} catch (err) {
			error = errorMessage(err);
		}
	});

	function select(id: string) {
		tab = id;
		try {
			localStorage.setItem('talkops.routingTab', id);
		} catch {
			// storage unavailable
		}
	}
</script>

<div class="space-y-4">
	<h1>{t('nav.routing')}</h1>
	<p class="text-sm text-slate-600 dark:text-slate-300">{t('routing.intro')}</p>
	<ErrorBox {error} />
	<div class="flex flex-wrap gap-1 border-b border-slate-200 dark:border-slate-800" role="tablist">
		{#each tabs as x (x.id)}
			<button
				role="tab"
				aria-selected={tab === x.id}
				class="rounded-t-md px-3 py-2 text-sm {tab === x.id
					? 'border-b-2 border-teal-600 font-medium text-teal-800 dark:text-teal-200'
					: 'text-slate-600 hover:text-slate-900 dark:text-slate-300'}"
				onclick={() => select(x.id)}>{t(x.key)}</button
			>
		{/each}
	</div>
	{#if loaded}
		{#if tab === 'groups'}<RingGroups />
		{:else if tab === 'schedules'}<TimeConditions />
		{:else if tab === 'menus'}<Attendants />
		{:else}<Queues />{/if}
	{/if}
</div>
