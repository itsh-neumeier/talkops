<script lang="ts">
	import { api, type Extension, type PhoneNumber, type User } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import InternalNumberInput from '#lib/components/InternalNumberInput.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import { describe } from '#lib/destinations.svelte.ts';
	import { ringsExtension, saveNumber } from '#lib/numbers.ts';
	import { hasRole } from '#lib/session.svelte.ts';

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

	/** Incoming numbers that ring this extension, as edited. */
	// svelte-ignore state_referenced_locally
	let ringing = $state<string[]>(
		extension ? numbers.filter((n) => ringsExtension(n, extension.id)).map((n) => n.id) : []
	);

	function toggleNumber(id: string, on: boolean) {
		ringing = on ? [...ringing, id] : ringing.filter((x) => x !== id);
	}

	/** What a number does now, if it does not ring this extension. */
	function elsewhere(n: PhoneNumber): string {
		if (n.destination_type === 'none' || !n.destination_id) return '';
		if (extension && ringsExtension(n, extension.id)) return '';
		const target =
			describe(n.destination_type, n.destination_id)?.label ?? t('ext.numberElsewhere');
		// Ticking adds this extension to another one's number; it replaces anything else.
		return n.destination_type === 'extension'
			? t('ext.numberMainIs', { target })
			: t('ext.numberNow', { target });
	}

	/** Adds or removes the extension on the numbers whose box changed. */
	async function applyNumbers(extId: string) {
		for (const n of numbers) {
			const was = ringsExtension(n, extId);
			const now = ringing.includes(n.id);
			if (was === now) continue;
			if (now) {
				await saveNumber(
					n,
					n.destination_type === 'extension' && n.destination_id
						? { extra_extensions: [...n.extra_extensions, extId] }
						: { destination_type: 'extension', destination_id: extId, extra_extensions: [] }
				);
			} else if (n.destination_id === extId) {
				// The main extension leaves: the next one takes over (voicemail too).
				const [next, ...rest] = n.extra_extensions;
				await saveNumber(
					n,
					next
						? { destination_id: next, extra_extensions: rest }
						: { destination_type: 'none', destination_id: null, extra_extensions: [] }
				);
			} else {
				await saveNumber(n, {
					extra_extensions: n.extra_extensions.filter((x) => x !== extId)
				});
			}
		}
	}

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
			if (hasRole('admin')) await applyNumbers(saved.id);
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
			<InternalNumberInput
				id="e-number"
				bind:value={form.number}
				keep={extension?.number}
				placeholder="100"
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
	{#if hasRole('admin') && numbers.length}
		<fieldset class="space-y-1">
			<legend class="text-sm font-medium">{t('ext.incomingNumbers')}</legend>
			{#each numbers as n (n.id)}
				<label class="flex flex-wrap items-center gap-2 text-sm font-normal">
					<input
						type="checkbox"
						checked={ringing.includes(n.id)}
						onchange={(e) => toggleNumber(n.id, e.currentTarget.checked)}
					/>
					<span class="font-mono">{n.e164}</span>
					{n.label}
					{#if extension && n.destination_type === 'extension' && n.destination_id === extension.id}
						<span class="badge badge-muted">{t('ext.numberMain')}</span>
					{:else if elsewhere(n)}
						<span class="text-xs text-slate-500">({elsewhere(n)})</span>
					{/if}
				</label>
			{/each}
			<p class="hint">{t('ext.incomingHint')}</p>
		</fieldset>
	{/if}
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
