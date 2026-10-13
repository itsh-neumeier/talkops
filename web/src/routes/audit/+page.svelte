<script lang="ts">
	import EmptyRow from '#lib/components/EmptyRow.svelte';
	import PageHeader from '#lib/components/PageHeader.svelte';
	import SkeletonRows from '#lib/components/SkeletonRows.svelte';
	import { net } from '#lib/net.svelte.ts';
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
	<PageHeader title={t('nav.audit')} />
	<ErrorBox {error} />
	<div class="card scroll-x">
		<table class="table">
			<thead>
				<tr
					><th>{t('audit.time')}</th><th>{t('audit.user')}</th><th>{t('audit.action')}</th><th
						class="col-opt">{t('audit.entity')}</th
					><th class="col-opt-lg"></th></tr
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
						<td class="col-opt">{e.entity_type}</td>
						<td
							class="col-opt-lg max-w-md truncate font-mono text-xs"
							title={JSON.stringify(e.details)}>{JSON.stringify(e.details)}</td
						>
					</tr>
				{:else}
					{#if !net.settled}<SkeletonRows cols={5} />{:else}<EmptyRow cols={5} />{/if}
				{/each}
			</tbody>
		</table>
	</div>
</div>
