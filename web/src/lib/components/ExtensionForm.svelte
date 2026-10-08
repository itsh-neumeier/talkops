<script lang="ts">
	import { api, type Extension, type PhoneNumber, type User } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let {
		extension = null,
		users,
		numbers,
		onsaved,
		oncancel
	}: {
		extension?: Extension | null;
		users: User[];
		numbers: PhoneNumber[];
		onsaved: (ext: Extension) => void;
		oncancel: () => void;
	} = $props();

	// svelte-ignore state_referenced_locally
	let form = $state({
		number: extension?.number ?? '',
		display_name: extension?.display_name ?? '',
		user_id: extension?.user_id ?? '',
		outbound_number_id: extension?.outbound_number_id ?? '',
		hide_caller_id: extension?.hide_caller_id ?? false,
		ring_timeout_secs: extension?.ring_timeout_secs ?? 30,
		enabled: extension?.enabled ?? true,
		record_calls: extension?.record_calls ?? 'inherit',
		video_enabled: extension?.video_enabled ?? false,
		// edited separately (CallSettings), passed through unchanged
		dnd: extension?.dnd ?? false,
		forward_all: extension?.forward_all ?? null
	});
	let error = $state('');

	async function save(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		const body = {
			...form,
			user_id: form.user_id || null,
			outbound_number_id: form.outbound_number_id || null,
			ring_timeout_secs: Number(form.ring_timeout_secs)
		};
		try {
			const saved = extension
				? await api.put<Extension>(`/extensions/${extension.id}`, body)
				: await api.post<Extension>('/extensions', body);
			onsaved(saved);
		} catch (err) {
			error = errorMessage(err);
		}
	}
</script>

<form class="space-y-3" onsubmit={save}>
	<ErrorBox {error} />
	<div class="grid grid-cols-3 gap-3">
		<div>
			<label for="e-number">{t('ext.number')}</label>
			<input
				id="e-number"
				class="input font-mono"
				inputmode="numeric"
				pattern="[1-9][0-9]{'{1,7}'}"
				bind:value={form.number}
				required
			/>
		</div>
		<div class="col-span-2">
			<label for="e-name">{t('ext.displayName')}</label>
			<input id="e-name" class="input" bind:value={form.display_name} required />
		</div>
	</div>
	<div>
		<label for="e-user">{t('ext.user')}</label>
		<select id="e-user" class="input" bind:value={form.user_id}>
			<option value="">—</option>
			{#each users as u (u.id)}<option value={u.id}>{u.display_name} ({u.username})</option>{/each}
		</select>
	</div>
	<div>
		<label for="e-out">{t('ext.outboundNumber')}</label>
		<select id="e-out" class="input" bind:value={form.outbound_number_id}>
			<option value="">{t('ext.defaultNumber')}</option>
			{#each numbers as n (n.id)}<option value={n.id}>{n.e164} {n.label}</option>{/each}
		</select>
	</div>
	<div>
		<label for="e-ring">{t('ext.ringTimeout')}</label>
		<input
			id="e-ring"
			class="input"
			type="number"
			min="5"
			max="300"
			bind:value={form.ring_timeout_secs}
		/>
	</div>
	<div>
		<label for="e-rec">{t('rec.extensionPolicy')}</label>
		<select id="e-rec" class="input" bind:value={form.record_calls}>
			<option value="inherit">{t('rec.inherit')}</option>
			<option value="always">{t('rec.always')}</option>
			<option value="never">{t('rec.never')}</option>
		</select>
	</div>
	<label class="flex items-center gap-2"
		><input type="checkbox" bind:checked={form.hide_caller_id} /> {t('ext.hideCallerId')}</label
	>
	<div>
		<label class="flex items-center gap-2"
			><input type="checkbox" bind:checked={form.video_enabled} /> {t('ext.video')}</label
		>
		<p class="hint">{t('ext.videoHint')}</p>
	</div>
	<label class="flex items-center gap-2"
		><input type="checkbox" bind:checked={form.enabled} /> {t('common.enabled')}</label
	>
	<div class="flex justify-end gap-2">
		<button type="button" class="btn" onclick={oncancel}>{t('common.cancel')}</button>
		<button class="btn btn-primary">{t('common.save')}</button>
	</div>
</form>
