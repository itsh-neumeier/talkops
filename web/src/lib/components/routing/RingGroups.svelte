<script lang="ts">
	import InternalNumberInput from '#lib/components/InternalNumberInput.svelte';
	import { api, type RingGroup } from '#lib/api.ts';
	import DestinationSelect from '#lib/components/DestinationSelect.svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import MemberPicker from '#lib/components/routing/MemberPicker.svelte';
	import { describe, loadTargets, targets } from '#lib/destinations.svelte.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { hasRole } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	const blank = () => ({
		id: '',
		number: '',
		name: '',
		strategy: 'simultaneous' as RingGroup['strategy'],
		ring_timeout_secs: 25,
		caller_id_prefix: '',
		fallback_type: 'none' as RingGroup['fallback_type'],
		fallback_id: null as string | null,
		enabled: true,
		members: [] as string[]
	});
	let form = $state(blank());
	let keepNumber = $state<string | null>(null);
	let open = $state(false);
	let error = $state('');

	function edit(g: RingGroup | null) {
		error = '';
		keepNumber = g?.number ?? null;
		form = g ? { ...g, number: g.number ?? '', members: [...g.members] } : blank();
		open = true;
	}

	async function save(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		const body = {
			...form,
			number: form.number || null,
			ring_timeout_secs: Number(form.ring_timeout_secs)
		};
		try {
			if (form.id) await api.put(`/ring-groups/${form.id}`, body);
			else await api.post('/ring-groups', body);
			open = false;
			await loadTargets();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function remove(g: RingGroup) {
		if (!confirm(t('common.confirmDelete', { name: g.name }))) return;
		try {
			await api.del(`/ring-groups/${g.id}`);
			await loadTargets();
		} catch (err) {
			error = errorMessage(err);
		}
	}
</script>

<div class="space-y-3">
	<div class="flex items-center justify-between">
		<p class="text-sm text-slate-600 dark:text-slate-300">{t('groups.hint')}</p>
		{#if hasRole('admin')}<button class="btn btn-primary" onclick={() => edit(null)}
				>{t('groups.new')}</button
			>{/if}
	</div>
	<ErrorBox error={open ? '' : error} />
	<div class="card overflow-x-auto">
		{#if targets.groups.length === 0}
			<p class="text-sm text-slate-500">{t('routing.none')}</p>
		{:else}
			<table class="table">
				<thead
					><tr
						><th>{t('routing.number')}</th><th>{t('common.name')}</th><th>{t('groups.strategy')}</th
						><th>{t('groups.members')}</th><th>{t('routing.fallback')}</th><th></th></tr
					></thead
				>
				<tbody>
					{#each targets.groups as g (g.id)}
						<tr>
							<td class="font-mono">{g.number ?? '—'}</td>
							<td
								>{g.name}{#if !g.enabled}<span class="badge badge-muted"
										>{t('common.disabled')}</span
									>{/if}</td
							>
							<td>{t(g.strategy === 'sequential' ? 'groups.sequential' : 'groups.simultaneous')}</td
							>
							<td>{g.members.length}</td>
							<td>{describe(g.fallback_type, g.fallback_id)?.label ?? '—'}</td>
							<td class="space-x-1 text-right whitespace-nowrap">
								{#if hasRole('admin')}
									<button class="btn btn-sm" onclick={() => edit(g)}>{t('common.edit')}</button>
									<button class="btn btn-sm btn-danger" onclick={() => remove(g)}
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

<Modal title={form.id ? t('common.edit') : t('groups.new')} bind:open>
	<form class="space-y-3" onsubmit={save}>
		<ErrorBox {error} />
		<div class="grid grid-cols-3 gap-3">
			<div>
				<label for="g-num">{t('routing.number')}</label><InternalNumberInput
					id="g-num"
					bind:value={form.number}
					keep={keepNumber}
					placeholder="500"
				/>
			</div>
			<div class="col-span-2">
				<label for="g-name">{t('common.name')}</label><input
					id="g-name"
					class="input"
					bind:value={form.name}
					required
				/>
			</div>
		</div>
		<div class="grid grid-cols-2 gap-3">
			<div>
				<label for="g-strat">{t('groups.strategy')}</label>
				<select id="g-strat" class="input" bind:value={form.strategy}>
					<option value="simultaneous">{t('groups.simultaneous')}</option>
					<option value="sequential">{t('groups.sequential')}</option>
				</select>
			</div>
			<div>
				<label for="g-time">{t('groups.ringTime')}</label><input
					id="g-time"
					class="input"
					type="number"
					min="5"
					max="300"
					bind:value={form.ring_timeout_secs}
				/>
			</div>
		</div>
		<div>
			<label for="g-prefix">{t('groups.prefix')}</label><input
				id="g-prefix"
				class="input"
				maxlength="20"
				bind:value={form.caller_id_prefix}
				placeholder="Support: "
			/>
		</div>
		<div>
			<span class="text-sm font-medium">{t('groups.members')}</span><MemberPicker
				bind:members={form.members}
			/>
		</div>
		<div>
			<label for="g-fb">{t('routing.fallback')}</label>
			<DestinationSelect
				inputId="g-fb"
				bind:type={form.fallback_type}
				bind:id={form.fallback_id}
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
