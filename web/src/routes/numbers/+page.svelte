<script lang="ts">
	import EmptyRow from '#lib/components/EmptyRow.svelte';
	import PageHeader from '#lib/components/PageHeader.svelte';
	import SkeletonRows from '#lib/components/SkeletonRows.svelte';
	import { net } from '#lib/net.svelte.ts';
	import { onMount } from 'svelte';
	import { api, type PhoneNumber, type Trunk } from '#lib/api.ts';
	import { loadTargets } from '#lib/destinations.svelte.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import NumberDestination from '#lib/components/NumberDestination.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { hasRole } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let numbers = $state<PhoneNumber[]>([]);
	let trunks = $state<Trunk[]>([]);
	let error = $state('');

	async function load() {
		try {
			[numbers, trunks] = await Promise.all([
				api.get<PhoneNumber[]>('/numbers'),
				api.get<Trunk[]>('/trunks'),
				loadTargets()
			]);
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);
</script>

<div class="space-y-4">
	<PageHeader title={t('nav.numbers')} />
	<ErrorBox {error} />
	<div class="card scroll-x">
		<table class="table">
			<thead>
				<tr
					><th>{t('numbers.number')}</th><th class="col-opt">{t('numbers.label')}</th><th
						class="col-opt-lg">{t('numbers.trunk')}</th
					><th>{t('trunks.destination')}</th></tr
				>
			</thead>
			<tbody>
				{#each numbers as n (n.id)}
					{@const trunk = trunks.find((tr) => tr.id === n.trunk_id)}
					<tr>
						<td class="font-mono">{n.e164}</td>
						<td class="col-opt">{n.label}</td>
						<td class="col-opt-lg"
							><a class="hover:underline" href="/trunks/{n.trunk_id}">{trunk?.name ?? '—'}</a></td
						>
						<td><NumberDestination number={n} editable={hasRole('admin')} onchange={load} /></td>
					</tr>
				{:else}
					{#if !net.settled}<SkeletonRows cols={4} />{:else}<EmptyRow cols={4} />{/if}
				{/each}
			</tbody>
		</table>
	</div>
</div>
