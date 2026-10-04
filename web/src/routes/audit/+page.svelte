<script lang="ts">
	import { onMount } from 'svelte';
	import { api, type AuditEntry } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { formatDateTime, t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let entries = $state<AuditEntry[]>([]);
	let error = $state('');

	onMount(async () => {
		try {
			entries = await api.get<AuditEntry[]>('/audit');
		} catch (err) {
			error = errorMessage(err);
		}
	});
</script>

<div class="space-y-4">
	<h1>{t('nav.audit')}</h1>
	<ErrorBox {error} />
	<div class="card overflow-x-auto">
		<table class="table">
			<thead>
				<tr
					><th>{t('audit.time')}</th><th>{t('audit.user')}</th><th>{t('audit.action')}</th><th
						>{t('audit.entity')}</th
					><th></th></tr
				>
			</thead>
			<tbody>
				{#each entries as e (e.id)}
					<tr>
						<td class="whitespace-nowrap">{formatDateTime(e.created_at)}</td>
						<td
							>{e.username ?? '—'}
							<div class="text-xs text-slate-500">{e.ip ?? ''}</div></td
						>
						<td>{e.action}</td>
						<td>{e.entity_type}</td>
						<td class="max-w-md truncate font-mono text-xs" title={JSON.stringify(e.details)}
							>{JSON.stringify(e.details)}</td
						>
					</tr>
				{/each}
			</tbody>
		</table>
	</div>
</div>
