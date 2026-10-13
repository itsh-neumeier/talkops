<script lang="ts">
	import { onMount } from 'svelte';
	import { api, type DoorStation, type Extension } from '#lib/api.ts';
	import DestinationSelect from '#lib/components/DestinationSelect.svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { loadTargets } from '#lib/destinations.svelte.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let {
		station = null,
		onsaved,
		oncancel
	}: {
		station?: DoorStation | null;
		onsaved: () => void;
		oncancel: () => void;
	} = $props();

	// svelte-ignore state_referenced_locally
	let form = $state({
		name: station?.name ?? '',
		extension_id: station?.extension_id ?? '',
		host: station?.host ?? '',
		port: station?.port ?? 80,
		username: station?.username ?? 'admin',
		password: '',
		doors: station?.doors ?? 1,
		destination_type: station?.destination_type ?? 'none',
		destination_id: station?.destination_id ?? null,
		buttons: (station?.buttons ?? []).map((b) => ({ ...b })),
		events_enabled: station?.events_enabled ?? true,
		snapshots: station?.snapshots ?? true,
		webhook_url: '',
		remove_webhook: false,
		unlock_code_1: '',
		unlock_code_2: '',
		remove_codes: false,
		// One number per line.
		dtmf_allowed_numbers: (station?.dtmf_allowed_numbers ?? []).join('\n'),
		enabled: station?.enabled ?? true
	});
	let extensions = $state<Extension[]>([]);
	let error = $state('');

	onMount(async () => {
		try {
			extensions = await api.get<Extension[]>('/extensions');
			await loadTargets();
		} catch (err) {
			error = errorMessage(err);
		}
	});

	async function save(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		// Empty code fields keep the stored code; "remove" clears both.
		const code = (c: string) => (form.remove_codes ? '' : c === '' ? null : c);
		const body = {
			...form,
			unlock_code_1: code(form.unlock_code_1),
			unlock_code_2: Number(form.doors) > 1 ? code(form.unlock_code_2) : '',
			dtmf_allowed_numbers: form.dtmf_allowed_numbers
				.split('\n')
				.map((n) => n.trim())
				.filter((n) => n !== ''),
			port: Number(form.port),
			doors: Number(form.doors),
			// Empty password/webhook fields keep the stored value.
			password: form.password === '' ? null : form.password,
			webhook_url: form.remove_webhook ? '' : form.webhook_url === '' ? null : form.webhook_url
		};
		try {
			if (station) await api.put(`/door-stations/${station.id}`, body);
			else await api.post('/door-stations', body);
			onsaved();
		} catch (err) {
			error = errorMessage(err);
		}
	}
</script>

<form class="space-y-3" onsubmit={save}>
	<ErrorBox {error} />
	<div>
		<label for="d-name">{t('common.name')}</label>
		<input id="d-name" class="input" bind:value={form.name} required maxlength="64" />
	</div>
	<div>
		<label for="d-ext">{t('door.extension')}</label>
		<select id="d-ext" class="input" bind:value={form.extension_id} required>
			<option value="" disabled>—</option>
			{#each extensions as x (x.id)}<option value={x.id}>{x.number} {x.display_name}</option>{/each}
		</select>
		<p class="hint">{t('door.extensionHint')}</p>
	</div>
	<div>
		<label for="d-dest">{t('door.destination')}</label>
		<DestinationSelect
			inputId="d-dest"
			bind:type={form.destination_type}
			bind:id={form.destination_id}
		/>
	</div>
	<fieldset class="space-y-2">
		<legend class="text-sm font-medium">{t('door.buttons')}</legend>
		{#each form.buttons as b, i (i)}
			<div class="flex items-center gap-2">
				<input
					class="input mt-0 w-28 font-mono"
					bind:value={b.number}
					placeholder="9902"
					aria-label={t('door.buttonNumber')}
					required
				/>
				<div class="flex-1">
					<DestinationSelect bind:type={b.type} bind:id={b.id} />
				</div>
				<button
					type="button"
					class="btn btn-sm"
					aria-label={t('common.delete')}
					onclick={() => form.buttons.splice(i, 1)}>✕</button
				>
			</div>
		{/each}
		<button
			type="button"
			class="btn btn-sm"
			onclick={() => form.buttons.push({ number: '', type: 'none', id: null })}
			>{t('door.addButton')}</button
		>
		<p class="hint">{t('door.buttonsHint')}</p>
	</fieldset>
	<fieldset class="space-y-3">
		<legend class="text-sm font-medium">{t('door.http')}</legend>
		<p class="hint">{t('door.httpHint')}</p>
		<div class="grid grid-cols-3 gap-3">
			<div class="col-span-2">
				<label for="d-host">{t('door.host')}</label>
				<input
					id="d-host"
					class="input font-mono"
					bind:value={form.host}
					placeholder="192.168.1.20"
				/>
			</div>
			<div>
				<label for="d-port">{t('door.port')}</label>
				<input id="d-port" class="input" type="number" min="1" max="65535" bind:value={form.port} />
			</div>
		</div>
		<div class="grid grid-cols-2 gap-3">
			<div>
				<label for="d-user">{t('door.username')}</label>
				<input id="d-user" class="input" bind:value={form.username} autocomplete="off" />
			</div>
			<div>
				<label for="d-pw">{t('door.password')}</label>
				<input
					id="d-pw"
					class="input"
					type="password"
					bind:value={form.password}
					autocomplete="new-password"
					placeholder={station?.has_password ? t('door.passwordKeep') : ''}
				/>
			</div>
		</div>
		<div>
			<label for="d-doors">{t('door.doors')}</label>
			<select id="d-doors" class="input w-32" bind:value={form.doors}>
				<option value={1}>1</option>
				<option value={2}>2</option>
			</select>
			<p class="hint">{t('door.doorsHint')}</p>
		</div>
		<div class="grid grid-cols-2 gap-3">
			<div>
				<label for="d-code-1">{t('door.unlockCode', { door: 1 })}</label>
				<input
					id="d-code-1"
					class="input font-mono"
					type="password"
					inputmode="tel"
					autocomplete="new-password"
					maxlength="8"
					pattern={'[0-9*#]{2,8}'}
					disabled={form.remove_codes}
					bind:value={form.unlock_code_1}
					placeholder={station?.has_unlock_code_1 ? t('door.passwordKeep') : '*1'}
				/>
			</div>
			{#if Number(form.doors) > 1}
				<div>
					<label for="d-code-2">{t('door.unlockCode', { door: 2 })}</label>
					<input
						id="d-code-2"
						class="input font-mono"
						type="password"
						inputmode="tel"
						autocomplete="new-password"
						maxlength="8"
						pattern={'[0-9*#]{2,8}'}
						disabled={form.remove_codes}
						bind:value={form.unlock_code_2}
						placeholder={station?.has_unlock_code_2 ? t('door.passwordKeep') : '*2'}
					/>
				</div>
			{/if}
		</div>
		{#if station?.has_unlock_code_1 || station?.has_unlock_code_2}
			<label class="flex items-center gap-2 text-sm"
				><input type="checkbox" bind:checked={form.remove_codes} />
				{t('door.unlockCodeRemove')}</label
			>
		{/if}
		<p class="hint">{t('door.unlockCodeHint')}</p>
		<div>
			<label for="d-allowed">{t('door.allowedNumbers')} ({t('common.optional')})</label>
			<textarea
				id="d-allowed"
				class="input font-mono"
				rows="3"
				bind:value={form.dtmf_allowed_numbers}
				placeholder="+491701234567"></textarea>
			<p class="hint">{t('door.allowedNumbersHint')}</p>
		</div>
		<label class="flex items-center gap-2"
			><input type="checkbox" bind:checked={form.snapshots} /> {t('door.snapshots')}</label
		>
		<label class="flex items-center gap-2"
			><input type="checkbox" bind:checked={form.events_enabled} /> {t('door.events')}</label
		>
	</fieldset>
	<div>
		<label for="d-hook">{t('door.webhook')}</label>
		<input
			id="d-hook"
			class="input font-mono"
			type="url"
			bind:value={form.webhook_url}
			disabled={form.remove_webhook}
			placeholder={station?.has_webhook
				? t('door.webhookKeep')
				: 'http://homeassistant.local:8123/api/webhook/…'}
		/>
		{#if station?.has_webhook}
			<label class="mt-1 flex items-center gap-2 text-sm"
				><input type="checkbox" bind:checked={form.remove_webhook} />
				{t('door.webhookRemove')}</label
			>
		{/if}
		<p class="hint">{t('door.webhookHint')}</p>
	</div>
	<label class="flex items-center gap-2"
		><input type="checkbox" bind:checked={form.enabled} /> {t('common.enabled')}</label
	>
	<div class="flex justify-end gap-2">
		<button type="button" class="btn" onclick={oncancel}>{t('common.cancel')}</button>
		<button class="btn btn-primary">{t('common.save')}</button>
	</div>
</form>
