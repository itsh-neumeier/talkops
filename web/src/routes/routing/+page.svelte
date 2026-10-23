<script lang="ts">
	import { onMount } from 'svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import PageHeader from '#lib/components/PageHeader.svelte';
	import Tabs from '#lib/components/Tabs.svelte';
	import Attendants from '#lib/components/routing/Attendants.svelte';
	import CallBlocking from '#lib/components/routing/CallBlocking.svelte';
	import DialingProfiles from '#lib/components/routing/DialingProfiles.svelte';
	import Queues from '#lib/components/routing/Queues.svelte';
	import RingGroups from '#lib/components/routing/RingGroups.svelte';
	import TimeConditions from '#lib/components/routing/TimeConditions.svelte';
	import { loadTargets } from '#lib/destinations.svelte.ts';
	import { t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import { can } from '#lib/session.svelte.ts';

	const allTabs: { id: string; key: MessageKey }[] = [
		{ id: 'groups', key: 'routing.groups' },
		{ id: 'schedules', key: 'routing.schedules' },
		{ id: 'menus', key: 'routing.menus' },
		{ id: 'queues', key: 'routing.queues' },
		{ id: 'blocking', key: 'routing.blocking' },
		{ id: 'dialing', key: 'routing.dialing' }
	];
	const tabs = $derived(
		allTabs.filter((x) =>
			x.id === 'blocking'
				? can('blocking.manage') || can('blocking.allowlist')
				: x.id === 'dialing'
					? can('dialing.manage')
					: can('routing.view')
		)
	);
	let tab = $state('groups');
	let loaded = $state(false);
	let error = $state('');

	onMount(async () => {
		tab = tabs[0]?.id ?? '';
		try {
			const saved = localStorage.getItem('talkops.routingTab');
			if (saved && tabs.some((x) => x.id === saved)) tab = saved;
		} catch {
			// storage unavailable
		}
		try {
			if (can('routing.view')) await loadTargets();
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
	<PageHeader title={t('nav.routing')} intro={t('routing.intro')} />
	<ErrorBox {error} />
	<Tabs
		tabs={tabs.map((x) => ({ id: x.id, label: t(x.key) }))}
		value={tab}
		onchange={select}
		label={t('nav.routing')}
	/>
	{#if loaded}
		{#if tab === 'groups'}<RingGroups />
		{:else if tab === 'schedules'}<TimeConditions />
		{:else if tab === 'menus'}<Attendants />
		{:else if tab === 'blocking'}<CallBlocking />
		{:else if tab === 'dialing'}<DialingProfiles />
		{:else}<Queues />{/if}
	{/if}
</div>
