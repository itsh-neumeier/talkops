<script lang="ts">
	import { api, type ContactImportReport, type PhonebookSection } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import Modal from '#lib/components/Modal.svelte';
	import { i18n, t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let {
		open = $bindable(false),
		sections,
		onimported
	}: { open: boolean; sections: PhonebookSection[]; onimported: () => void } = $props();

	let csv = $state('');
	let fileName = $state('');
	let sectionId = $state('');
	let updateExisting = $state(false);
	let report = $state<ContactImportReport | null>(null);
	let done = $state(false);
	let busy = $state(false);
	let error = $state('');

	const sample = $derived(
		i18n.locale === 'de' ? '/telefonbuch-beispiel.csv' : '/phonebook-sample.csv'
	);
	const changes = $derived(report ? report.created + (updateExisting ? report.updated : 0) : 0);

	$effect(() => {
		if (open) {
			csv = '';
			fileName = '';
			report = null;
			done = false;
			error = '';
		}
	});

	/** Excel saves "CSV" in the Windows code page unless "UTF-8" is chosen. */
	function decode(bytes: ArrayBuffer): string {
		try {
			return new TextDecoder('utf-8', { fatal: true }).decode(bytes);
		} catch {
			return new TextDecoder('windows-1252').decode(bytes);
		}
	}

	async function pick(e: Event) {
		const file = (e.currentTarget as HTMLInputElement).files?.[0];
		if (!file) return;
		fileName = file.name;
		csv = decode(await file.arrayBuffer());
		await run(true);
	}

	async function run(dryRun: boolean) {
		if (!csv) return;
		busy = true;
		error = '';
		try {
			report = await api.post<ContactImportReport>('/contacts/import', {
				csv,
				section_id: sectionId || null,
				update_existing: updateExisting,
				dry_run: dryRun
			});
			if (!dryRun) {
				done = true;
				onimported();
			}
		} catch (err) {
			report = null;
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}
</script>

<Modal title={t('contacts.importTitle')} bind:open>
	<div class="space-y-3">
		<p class="hint">{t('contacts.importHint')}</p>
		<a class="text-sm underline" href={sample} download>{t('contacts.importSample')}</a>
		<div>
			<label for="ci-file">{t('contacts.importFile')}</label>
			<input
				id="ci-file"
				class="input"
				type="file"
				accept=".csv,.txt,text/csv"
				onchange={pick}
				disabled={done}
			/>
		</div>
		<div>
			<label for="ci-section">{t('contacts.importSection')}</label>
			<select
				id="ci-section"
				class="input"
				bind:value={sectionId}
				onchange={() => run(true)}
				disabled={done}
			>
				<option value="">{t('contacts.global')}</option>
				{#each sections as sec (sec.id)}<option value={sec.id}>{sec.name}</option>{/each}
			</select>
			<p class="hint">{t('contacts.importSectionHint')}</p>
		</div>
		<label class="flex items-center gap-2">
			<input
				type="checkbox"
				bind:checked={updateExisting}
				onchange={() => run(true)}
				disabled={done}
			/>
			{t('contacts.importUpdate')}
		</label>
		<ErrorBox {error} />
		{#if report}
			<div class="rounded border border-slate-200 p-3 text-sm dark:border-slate-700">
				{#if done}<p class="mb-1 font-medium">{t('contacts.importDone')}</p>{:else}<p
						class="mb-1 font-medium"
					>
						{t('contacts.importPreview')} · {fileName}
					</p>{/if}
				<ul class="space-y-0.5">
					<li>{t('contacts.importCreated', { n: report.created })}</li>
					{#if report.updated}<li>{t('contacts.importUpdated', { n: report.updated })}</li>{/if}
					{#if report.unchanged}<li>
							{t('contacts.importUnchanged', { n: report.unchanged })}
						</li>{/if}
					{#if report.skipped}<li>{t('contacts.importSkipped', { n: report.skipped })}</li>{/if}
					{#if report.sections_created.length}<li>
							{t('contacts.importSections', { names: report.sections_created.join(', ') })}
						</li>{/if}
					{#if report.failed}<li class="text-red-600 dark:text-red-400">
							{t('contacts.importFailed', { n: report.failed })}
						</li>{/if}
				</ul>
				{#if report.errors.length}
					<ul
						class="mt-2 max-h-40 overflow-y-auto font-mono text-xs text-red-600 dark:text-red-400"
					>
						{#each report.errors as e (e.line)}
							<li>{t('contacts.importLine', { line: e.line })}: {e.message}</li>
						{/each}
						{#if report.failed > report.errors.length}
							<li>{t('contacts.importMoreErrors', { n: report.failed - report.errors.length })}</li>
						{/if}
					</ul>
				{/if}
				{#if !done && changes === 0 && !report.sections_created.length}
					<p class="mt-2 text-slate-500">{t('contacts.importNothing')}</p>
				{/if}
			</div>
		{/if}
		<div class="flex justify-end gap-2">
			<button type="button" class="btn" onclick={() => (open = false)}>
				{done ? t('common.close') : t('common.cancel')}
			</button>
			{#if !done}
				<button
					class="btn btn-primary"
					disabled={busy || !report || (changes === 0 && !report.sections_created.length)}
					onclick={() => run(false)}>{t('contacts.import')}</button
				>
			{/if}
		</div>
	</div>
</Modal>
