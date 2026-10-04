<script lang="ts">
	import { onMount } from 'svelte';
	import { api, type Extension, type PhoneNumber, type Trunk } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import NumberDestination from '#lib/components/NumberDestination.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { hasRole } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let numbers = $state<PhoneNumber[]>([]);
	let trunks = $state<Trunk[]>([]);
	let extensions = $state<Extension[]>([]);
	let error = $state('');

	async function load() {
		try {
			[numbers, trunks, extensions] = await Promise.all([
				api.get<PhoneNumber[]>('/numbers'),
				api.get<Trunk[]>('/trunks'),
				api.get<Extension[]>('/extensions')
			]);
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);
</script>

<div class="space-y-4">
	<h1>{t('nav.numbers')}</h1>
	<ErrorBox {error} />
	<div class="card overflow-x-auto">
		<table class="table">
			<thead>
				<tr
					><th>{t('numbers.number')}</th><th>{t('numbers.label')}</th><th>{t('numbers.trunk')}</th
					><th>{t('trunks.destination')}</th></tr
				>
			</thead>
			<tbody>
				{#each numbers as n (n.id)}
					{@const trunk = trunks.find((tr) => tr.id === n.trunk_id)}
					<tr>
						<td class="font-mono">{n.e164}</td>
						<td>{n.label}</td>
						<td><a class="hover:underline" href="/trunks/{n.trunk_id}">{trunk?.name ?? '—'}</a></td>
						<td
							><NumberDestination
								number={n}
								{extensions}
								editable={hasRole('admin')}
								onchange={load}
							/></td
						>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
</div>
