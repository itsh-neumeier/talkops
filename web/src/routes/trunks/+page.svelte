<script lang="ts">
	import SkeletonRows from '#lib/components/SkeletonRows.svelte';
	import { net } from '#lib/net.svelte.ts';
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import { api, type Preset, type Trunk, type TrunkDetail } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { presetLabel, presetNotes, statusClass, statusKey } from '#lib/presets.ts';
	import { hasRole } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let trunks = $state<TrunkDetail[]>([]);
	let presets = $state<Preset[]>([]);
	let error = $state('');
	let open = $state(false);
	let presetId = $state('');
	let name = $state('');
	let registrar = $state('');

	const preset = $derived(presets.find((p) => p.id === presetId));
	const countries = $derived(
		[...new Set(presets.map((p) => p.country))].sort((a, b) =>
			a === 'DE' ? -1 : b === 'DE' ? 1 : a.localeCompare(b)
		)
	);

	async function load() {
		try {
			[trunks, presets] = await Promise.all([
				api.get<TrunkDetail[]>('/trunks'),
				api.get<Preset[]>('/presets')
			]);
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);

	$effect(() => {
		if (preset && !name) name = preset.name;
	});

	async function create(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		const overrides: Record<string, unknown> = {};
		if (registrar.trim()) overrides.registrar = registrar.trim();
		try {
			const trunk = await api.post<Trunk>('/trunks', { name, preset: presetId, overrides });
			open = false;
			goto(`/trunks/${trunk.id}`);
		} catch (err) {
			error = errorMessage(err);
		}
	}

	function registered(trunk: TrunkDetail) {
		return trunk.accounts.filter((a) => a.state?.state === 'REGED').length;
	}
</script>

<div class="space-y-4">
	<div class="flex items-center justify-between">
		<h1>{t('nav.trunks')}</h1>
		{#if hasRole('admin')}<button class="btn btn-primary" onclick={() => (open = true)}
				>{t('trunks.new')}</button
			>{/if}
	</div>
	<ErrorBox {error} />
	<div class="card overflow-x-auto">
		<table class="table">
			<thead>
				<tr
					><th>{t('common.name')}</th><th>{t('trunks.preset')}</th><th>{t('trunks.numbers')}</th><th
						>{t('trunks.state')}</th
					></tr
				>
			</thead>
			<tbody>
				{#each trunks as trunk (trunk.id)}
					{@const p = presets.find((x) => x.id === trunk.preset)}
					<tr>
						<td><a class="hover:underline" href="/trunks/{trunk.id}">{trunk.name}</a></td>
						<td>{p ? presetLabel(p) : trunk.preset}</td>
						<td class="font-mono text-xs">{trunk.numbers.map((n) => n.e164).join(', ') || '—'}</td>
						<td>
							{#if !trunk.enabled}<span class="badge badge-muted">{t('common.disabled')}</span>
							{:else}<span
									class="badge {registered(trunk) === trunk.accounts.length && trunk.accounts.length
										? 'badge-ok'
										: 'badge-bad'}">{registered(trunk)}/{trunk.accounts.length}</span
								>{/if}
						</td>
					</tr>
				{:else}
					{#if !net.settled}<SkeletonRows cols={4} />{/if}
				{/each}
			</tbody>
		</table>
	</div>
</div>

<Modal title={t('trunks.new')} bind:open>
	<form class="space-y-3" onsubmit={create}>
		<ErrorBox {error} />
		<div>
			<label for="t-preset">{t('trunks.preset')}</label>
			<select id="t-preset" class="input" bind:value={presetId} required>
				<option value="" disabled>{t('trunks.choosePreset')}</option>
				{#each countries as country (country)}
					<optgroup label={country === '*' ? 'International' : country}>
						{#each presets.filter((p) => p.country === country) as p (p.id)}
							<option value={p.id}>{presetLabel(p)}</option>
						{/each}
					</optgroup>
				{/each}
			</select>
		</div>
		{#if preset}
			<div class="space-y-2 rounded-md bg-slate-50 p-3 text-sm dark:bg-slate-800/50">
				<span class="badge {statusClass[preset.status]}">{t(statusKey[preset.status])}</span>
				{#if preset.status === 'untested'}<p class="hint">{t('trunks.untestedHint')}</p>{/if}
				<p>{presetNotes(preset)}</p>
				{#if preset.sources.length}
					<details>
						<summary class="cursor-pointer text-xs text-slate-500">{t('trunks.sources')}</summary>
						<ul class="mt-1 list-disc pl-5 text-xs">
							{#each preset.sources as s (s)}<li class="break-all">
									{#if s.startsWith('http')}<a
											class="underline"
											href={s}
											target="_blank"
											rel="noreferrer">{s}</a
										>{:else}{s}{/if}
								</li>{/each}
						</ul>
					</details>
				{/if}
			</div>
			<div>
				<label for="t-name">{t('common.name')}</label>
				<input id="t-name" class="input" bind:value={name} required />
			</div>
			{#if !preset.sip.registrar}
				<div>
					<label for="t-reg">{t('trunks.registrar')}</label>
					<input id="t-reg" class="input font-mono" bind:value={registrar} required />
					<p class="hint">{t('trunks.registrarRequired')}</p>
				</div>
			{/if}
		{/if}
		<div class="flex justify-end gap-2">
			<button type="button" class="btn" onclick={() => (open = false)}>{t('common.cancel')}</button>
			<button class="btn btn-primary" disabled={!preset}>{t('common.add')}</button>
		</div>
	</form>
</Modal>
