<script lang="ts">
	import { api, type Queue } from '#lib/api.ts';
	import DestinationSelect from '#lib/components/DestinationSelect.svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import MemberPicker from '#lib/components/routing/MemberPicker.svelte';
	import { describe, loadTargets, targets } from '#lib/destinations.svelte.ts';
	import { t, type MessageKey } from '#lib/i18n/index.svelte.ts';
	import { hasRole } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	const strategies = [
		'longest-idle-agent',
		'ring-all',
		'round-robin',
		'top-down',
		'agent-with-fewest-calls',
		'random'
	];
	const blank = () => ({
		id: '',
		number: '',
		name: '',
		strategy: 'longest-idle-agent',
		max_wait_secs: 300,
		agent_timeout_secs: 20,
		wrap_up_secs: 5,
		timeout_type: 'none' as Queue['timeout_type'],
		timeout_id: null as string | null,
		enabled: true,
		members: [] as string[]
	});
	let form = $state(blank());
	let open = $state(false);
	let error = $state('');

	function edit(q: Queue | null) {
		error = '';
		form = q ? { ...q, number: q.number ?? '', members: [...q.members] } : blank();
		open = true;
	}

	async function save(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		const body = {
			...form,
			number: form.number || null,
			max_wait_secs: Number(form.max_wait_secs),
			agent_timeout_secs: Number(form.agent_timeout_secs),
			wrap_up_secs: Number(form.wrap_up_secs)
		};
		try {
			if (form.id) await api.put(`/queues/${form.id}`, body);
			else await api.post('/queues', body);
			open = false;
			await loadTargets();
		} catch (err) {
			error = errorMessage(err);
		}
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
		{#if hasRole('admin')}<button class="btn btn-primary" onclick={() => edit(null)}
				>{t('queues.new')}</button
			>{/if}
	</div>
	<ErrorBox error={open ? '' : error} />
	<div class="card overflow-x-auto">
		{#if targets.queues.length === 0}
			<p class="text-sm text-slate-500">{t('routing.none')}</p>
		{:else}
			<table class="table">
				<thead
					><tr
						><th>{t('routing.number')}</th><th>{t('common.name')}</th><th>{t('groups.strategy')}</th
						><th>{t('queues.agents')}</th><th>{t('routing.fallback')}</th><th></th></tr
					></thead
				>
				<tbody>
					{#each targets.queues as q (q.id)}
						<tr>
							<td class="font-mono">{q.number ?? '—'}</td>
							<td
								>{q.name}{#if !q.enabled}<span class="badge badge-muted"
										>{t('common.disabled')}</span
									>{/if}</td
							>
							<td>{t(`queues.s.${q.strategy}` as MessageKey)}</td>
							<td>{q.members.length}</td>
							<td>{describe(q.timeout_type, q.timeout_id)?.label ?? '—'}</td>
							<td class="space-x-1 text-right whitespace-nowrap">
								{#if hasRole('admin')}
									<button class="btn btn-sm" onclick={() => edit(q)}>{t('common.edit')}</button>
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

<Modal title={form.id ? t('common.edit') : t('queues.new')} bind:open>
	<form class="space-y-3" onsubmit={save}>
		<ErrorBox {error} />
		<div class="grid grid-cols-3 gap-3">
			<div>
				<label for="q-num">{t('routing.number')}</label><input
					id="q-num"
					class="input font-mono"
					bind:value={form.number}
					placeholder="80"
				/>
			</div>
			<div class="col-span-2">
				<label for="q-name">{t('common.name')}</label><input
					id="q-name"
					class="input"
					bind:value={form.name}
					required
				/>
			</div>
		</div>
		<div>
			<label for="q-strat">{t('groups.strategy')}</label>
			<select id="q-strat" class="input" bind:value={form.strategy}>
				{#each strategies as s (s)}<option value={s}>{t(`queues.s.${s}` as MessageKey)}</option
					>{/each}
			</select>
		</div>
		<div class="grid grid-cols-3 gap-3">
			<div>
				<label for="q-wait">{t('queues.maxWait')}</label><input
					id="q-wait"
					class="input"
					type="number"
					min="0"
					max="7200"
					bind:value={form.max_wait_secs}
				/>
			</div>
			<div>
				<label for="q-ring">{t('queues.agentTimeout')}</label><input
					id="q-ring"
					class="input"
					type="number"
					min="5"
					max="120"
					bind:value={form.agent_timeout_secs}
				/>
			</div>
			<div>
				<label for="q-wrap">{t('queues.wrapUp')}</label><input
					id="q-wrap"
					class="input"
					type="number"
					min="0"
					max="600"
					bind:value={form.wrap_up_secs}
				/>
			</div>
		</div>
		<div>
			<span class="text-sm font-medium">{t('queues.agents')}</span><MemberPicker
				bind:members={form.members}
			/>
		</div>
		<p class="hint">{t('queues.agentsHint')}</p>
		<div>
			<label for="q-fb">{t('routing.fallback')}</label><DestinationSelect
				inputId="q-fb"
				bind:type={form.timeout_type}
				bind:id={form.timeout_id}
				exclude={form.id}
				noneLabel={t('routing.hangup')}
			/>
		</div>
		<label class="flex items-center gap-2"
			><input type="checkbox" bind:checked={form.enabled} /> {t('common.enabled')}</label
		>
		<div class="flex justify-end gap-2">
			<button type="button" class="btn" onclick={() => (open = false)}>{t('common.cancel')}</button>
			<button class="btn btn-primary">{t('common.save')}</button>
		</div>
	</form>
</Modal>
