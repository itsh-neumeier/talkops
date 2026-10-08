<script lang="ts">
	import { api, type Queue } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { describe, loadTargets, targets } from '#lib/destinations.svelte.ts';
	import { t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { hasRole } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let error = $state('');

	function unanswered(q: Queue) {
		if (q.voicemail_recipients.length)
			return `${t('dest.voicemail')} (${q.voicemail_recipients.length})`;
		return describe(q.timeout_type, q.timeout_id)?.label ?? t('routing.hangup');
	}

	async function remove(q: Queue) {
		if (!confirm(t('common.confirmDelete', { name: q.name }))) return;
		try {
			await api.del(`/queues/${q.id}`);
			await loadTargets();
		} catch (err) {
			error = errorMessage(err);
		}
	}
</script>

<div class="space-y-3">
	<div class="flex items-center justify-between gap-2">
		<p class="text-sm text-slate-600 dark:text-slate-300">{t('queues.hint')}</p>
		{#if hasRole('admin')}<a class="btn btn-primary" href="/routing/queues/new">{t('queues.new')}</a
			>{/if}
	</div>
	<ErrorBox {error} />
	<div class="card overflow-x-auto">
		{#if targets.queues.length === 0}
			<p class="text-sm text-slate-500">{t('routing.none')}</p>
		{:else}
			<table class="table">
				<thead
					><tr
						><th>{t('routing.number')}</th><th>{t('common.name')}</th><th>{t('groups.strategy')}</th
						><th>{t('queues.agents')}</th><th>{t('queues.tab.schedule')}</th><th
							>{t('queues.unanswered')}</th
						><th></th></tr
					></thead
				>
				<tbody>
					{#each targets.queues as q (q.id)}
						{@const hours = targets.conditions.find((c) => c.id === q.time_condition_id)}
						<tr>
							<td class="font-mono">{q.number ?? '—'}</td>
							<td
								>{q.name}{#if !q.enabled}<span class="badge badge-muted"
										>{t('common.disabled')}</span
									>{/if}</td
							>
							<td>{t(`queues.s.${q.strategy}` as MessageKey)}</td>
							<td>{q.members.length}</td>
							<td
								>{#if hours}{hours.name}
									<span class="badge {hours.state.open ? 'badge-ok' : 'badge-muted'}"
										>{hours.state.open ? t('flow.branch.open') : t('flow.branch.closed')}</span
									>{:else}{t('queues.alwaysOpen')}{/if}</td
							>
							<td>{unanswered(q)}</td>
							<td class="space-x-1 text-right whitespace-nowrap">
								<a class="btn btn-sm" href="/routing/queues/{q.id}"
									>{hasRole('admin') ? t('common.edit') : t('flow.view')}</a
								>
								{#if hasRole('admin')}
									<button class="btn btn-sm btn-danger" onclick={() => remove(q)}
										>{t('common.delete')}</button
									>
								{/if}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{/if}
	</div>
</div>
