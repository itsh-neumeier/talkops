<script lang="ts">
	import { api, type IvrMenu, type MenuOption } from '#lib/api.ts';
	import { clipUrl } from '#lib/audio.ts';
	import AudioPicker, { type AudioMode } from '#lib/components/AudioPicker.svelte';
	import AudioPlayer from '#lib/components/AudioPlayer.svelte';
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
		greeting_clip_id: null as string | null,
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
	let preview = $state(0);
	let mode = $state<AudioMode>('generate');
	let clipId = $state<string | null>(null);
	let picker = $state<ReturnType<typeof AudioPicker>>();

	/** The greeting's audio, for the list and the editor. */
	function greetingSrc(
		m: Pick<IvrMenu, 'id' | 'greeting' | 'greeting_clip_id' | 'greeting_status'>
	) {
		if (m.greeting === 'clip' && m.greeting_clip_id) return clipUrl(m.greeting_clip_id);
		if ((m.greeting === 'tts' || m.greeting === 'upload') && m.greeting_status === 'ready')
			return `/api/v1/ivr-menus/${m.id}/greeting?v=${preview}`;
		return null;
	}

	// Greetings from before audio clips.
	const legacy = $derived.by(() => {
		const src = form.id ? greetingSrc(form) : null;
		if (!src || form.greeting === 'clip') return null;
		return form.greeting === 'tts'
			? { mode: 'generate' as const, src, text: form.greeting_text }
			: { mode: 'upload' as const, src };
	});

	// Refresh while greetings are being rendered.
	$effect(() => {
		if (!targets.menus.some((m) => m.greeting_status === 'pending')) return;
		const timer = setTimeout(() => loadTargets().then(() => preview++), 3000);
		return () => clearTimeout(timer);
	});

	function edit(m: IvrMenu | null) {
		error = '';
		form = m
			? { ...m, number: m.number ?? '', options: m.options.map((o) => ({ ...o })) }
			: blank();
		clipId = form.greeting === 'clip' ? form.greeting_clip_id : null;
		mode = form.greeting === 'none' ? 'none' : form.greeting === 'upload' ? 'upload' : 'generate';
		open = true;
	}

	const freeKeys = $derived(keys.filter((k) => !form.options.some((o) => o.digit === k)));

	async function save(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		let clip: string | null;
		try {
			clip = (await picker?.ensure()) ?? null;
		} catch (err) {
			error = errorMessage(err);
			return;
		}
		const greeting = mode === 'none' ? 'none' : clip ? 'clip' : form.greeting;
		const body = {
			...form,
			greeting,
			greeting_clip_id: clip,
			number: form.number || null,
			timeout_secs: Number(form.timeout_secs),
			max_tries: Number(form.max_tries),
			options: form.options.filter((o) => o.type !== 'none')
		};
		try {
			if (form.id) await api.put<IvrMenu>(`/ivr-menus/${form.id}`, body);
			else await api.post<IvrMenu>('/ivr-menus', body);
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
			{#if greetingSrc(m)}
				<AudioPlayer src={greetingSrc(m) ?? ''} />
			{:else if m.greeting_status === 'pending' && m.greeting === 'tts'}
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
		<div>
			<label for="ivr-lang">{t('vm.language')}</label>
			<select id="ivr-lang" class="input" bind:value={form.language}>
				<option value={null}>{t('vm.languageDefault')}</option>
				<option value="de">Deutsch</option>
				<option value="en">English</option>
			</select>
		</div>
		<fieldset class="space-y-2">
			<legend class="text-sm font-medium">{t('vm.greeting')}</legend>
			{#if open}
				<AudioPicker
					bind:this={picker}
					bind:mode
					bind:clipId
					modes={['generate', 'record', 'upload', 'none']}
					language={form.language ?? 'de'}
					defaultText={t('ivr.textPlaceholder')}
					{legacy}
				/>
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
