<script lang="ts">
	import { api, upload, type IvrMenu, type MenuOption } from '#lib/api.ts';
	import DestinationSelect from '#lib/components/DestinationSelect.svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import { describe, loadTargets, targets } from '#lib/destinations.svelte.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { hasRole } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	const keys = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0', '*', '#'];
	const blank = () => ({
		id: '',
		number: '',
		name: '',
		language: null as IvrMenu['language'],
		greeting: 'tts' as IvrMenu['greeting'],
		greeting_text: '',
		greeting_status: 'none' as IvrMenu['greeting_status'],
		timeout_secs: 5,
		max_tries: 3,
		direct_dial: false,
		options: [] as MenuOption[],
		timeout_type: 'none' as IvrMenu['timeout_type'],
		timeout_id: null as string | null
	});
	let form = $state(blank());
	let open = $state(false);
	let error = $state('');
	let file = $state<FileList | null>(null);
	let preview = $state(0);

	// Refresh while greetings are being rendered.
	$effect(() => {
		if (!targets.menus.some((m) => m.greeting_status === 'pending')) return;
		const timer = setTimeout(() => loadTargets().then(() => preview++), 3000);
		return () => clearTimeout(timer);
	});

	function edit(m: IvrMenu | null) {
		error = '';
		file = null;
		form = m
			? { ...m, number: m.number ?? '', options: m.options.map((o) => ({ ...o })) }
			: blank();
		open = true;
	}

	const freeKeys = $derived(keys.filter((k) => !form.options.some((o) => o.digit === k)));

	async function save(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		const body = {
			...form,
			number: form.number || null,
			timeout_secs: Number(form.timeout_secs),
			max_tries: Number(form.max_tries),
			options: form.options.filter((o) => o.type !== 'none')
		};
		try {
			const saved = form.id
				? await api.put<IvrMenu>(`/ivr-menus/${form.id}`, body)
				: await api.post<IvrMenu>('/ivr-menus', body);
			const wav = file?.[0];
			if (form.greeting === 'upload' && wav) {
				const data = new FormData();
				data.append('file', wav);
				await upload(`/ivr-menus/${saved.id}/greeting`, data);
			}
			open = false;
			await loadTargets();
			preview++;
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function remove(m: IvrMenu) {
		if (!confirm(t('common.confirmDelete', { name: m.name }))) return;
		try {
			await api.del(`/ivr-menus/${m.id}`);
			await loadTargets();
		} catch (err) {
			error = errorMessage(err);
		}
	}
</script>

<div class="space-y-3">
	<div class="flex items-center justify-between gap-2">
		<p class="text-sm text-slate-600 dark:text-slate-300">{t('ivr.hint')}</p>
		{#if hasRole('admin')}<button class="btn btn-primary" onclick={() => edit(null)}
				>{t('ivr.new')}</button
			>{/if}
	</div>
	<ErrorBox error={open ? '' : error} />
	{#if targets.menus.length === 0}
		<div class="card"><p class="text-sm text-slate-500">{t('routing.none')}</p></div>
	{/if}
	{#each targets.menus as m (m.id)}
		<section class="card space-y-2">
			<div class="flex flex-wrap items-center justify-between gap-2">
				<h3 class="font-medium"><span class="font-mono">{m.number ?? ''}</span> {m.name}</h3>
				{#if hasRole('admin')}
					<span class="space-x-1">
						<button class="btn btn-sm" onclick={() => edit(m)}>{t('common.edit')}</button>
						<button class="btn btn-sm btn-danger" onclick={() => remove(m)}
							>{t('common.delete')}</button
						>
					</span>
				{/if}
			</div>
			{#if m.greeting_status === 'ready'}
				{#key preview}<audio
						controls
						preload="none"
						src="/api/v1/ivr-menus/{m.id}/greeting?v={preview}"
					></audio>{/key}
			{:else if m.greeting_status === 'pending'}
				<p class="text-sm text-slate-500">{t('vm.greetingPending')}</p>
			{:else if m.greeting_status === 'failed'}
				<p class="text-sm text-red-600">{t('vm.greetingFailed')}</p>
			{/if}
			<!-- The menu as a simple flow: key → destination. -->
			<ul class="grid gap-1 text-sm sm:grid-cols-2">
				{#each m.options as o (o.digit)}
					<li class="flex items-center gap-2">
						<span
							class="inline-flex h-7 w-7 items-center justify-center rounded-full bg-teal-600 font-mono text-white"
							>{o.digit}</span
						>
						→ {describe(o.type, o.id)?.label ?? '—'}
					</li>
				{/each}
				{#if m.direct_dial}<li class="text-slate-500">{t('ivr.directDial')}</li>{/if}
				<li class="text-slate-500">
					{t('ivr.noInput')} → {describe(m.timeout_type, m.timeout_id)?.label ??
						t('routing.hangup')}
				</li>
			</ul>
		</section>
	{/each}
</div>

<Modal title={form.id ? t('common.edit') : t('ivr.new')} bind:open>
	<form class="space-y-3" onsubmit={save}>
		<ErrorBox {error} />
		<div class="grid grid-cols-3 gap-3">
			<div>
				<label for="ivr-num">{t('routing.number')}</label><input
					id="ivr-num"
					class="input font-mono"
					bind:value={form.number}
					placeholder="70"
				/>
			</div>
			<div class="col-span-2">
				<label for="ivr-name">{t('common.name')}</label><input
					id="ivr-name"
					class="input"
					bind:value={form.name}
					required
				/>
			</div>
		</div>
		<fieldset class="space-y-2">
			<legend class="text-sm font-medium">{t('vm.greeting')}</legend>
			<label class="flex items-center gap-2"
				><input type="radio" value="tts" bind:group={form.greeting} /> {t('vm.greetingTts')}</label
			>
			{#if form.greeting === 'tts'}
				<textarea
					class="input"
					rows="3"
					maxlength="2000"
					bind:value={form.greeting_text}
					placeholder={t('ivr.textPlaceholder')}></textarea>
				<select class="input" bind:value={form.language} aria-label={t('vm.language')}>
					<option value={null}>{t('vm.languageDefault')}</option>
					<option value="de">Deutsch</option>
					<option value="en">English</option>
				</select>
			{/if}
			<label class="flex items-center gap-2"
				><input type="radio" value="upload" bind:group={form.greeting} /> {t('ivr.upload')}</label
			>
			{#if form.greeting === 'upload'}
				<input class="input" type="file" accept="audio/wav,.wav" bind:files={file} />
				<p class="hint">{t('ivr.uploadHint')}</p>
			{/if}
		</fieldset>
		<fieldset class="space-y-2">
			<legend class="text-sm font-medium">{t('ivr.options')}</legend>
			{#each form.options as o, i (o.digit)}
				<div class="flex items-center gap-2">
					<span
						class="inline-flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-teal-600 font-mono text-white"
						>{o.digit}</span
					>
					<DestinationSelect bind:type={o.type} bind:id={o.id} exclude={form.id} />
					<button
						type="button"
						class="btn btn-sm"
						onclick={() => (form.options = form.options.filter((_, j) => j !== i))}
						aria-label={t('common.delete')}>✕</button
					>
				</div>
			{/each}
			{#if freeKeys.length > 0}
				<div class="flex flex-wrap gap-1">
					{#each freeKeys as k (k)}
						<button
							type="button"
							class="btn btn-sm font-mono"
							onclick={() =>
								(form.options = [...form.options, { digit: k, type: 'none', id: null }])}
							>+ {k}</button
						>
					{/each}
				</div>
			{/if}
		</fieldset>
		<label class="flex items-center gap-2"
			><input type="checkbox" bind:checked={form.direct_dial} /> {t('ivr.directDial')}</label
		>
		<div class="grid grid-cols-2 gap-3">
			<div>
				<label for="ivr-to">{t('ivr.timeout')}</label><input
					id="ivr-to"
					class="input"
					type="number"
					min="1"
					max="30"
					bind:value={form.timeout_secs}
				/>
			</div>
			<div>
				<label for="ivr-tries">{t('ivr.tries')}</label><input
					id="ivr-tries"
					class="input"
					type="number"
					min="1"
					max="10"
					bind:value={form.max_tries}
				/>
			</div>
		</div>
		<div>
			<label for="ivr-none">{t('ivr.noInput')}</label><DestinationSelect
				inputId="ivr-none"
				bind:type={form.timeout_type}
				bind:id={form.timeout_id}
				exclude={form.id}
				noneLabel={t('routing.hangup')}
			/>
		</div>
		<div class="flex justify-end gap-2">
			<button type="button" class="btn" onclick={() => (open = false)}>{t('common.cancel')}</button>
			<button class="btn btn-primary">{t('common.save')}</button>
		</div>
	</form>
</Modal>
