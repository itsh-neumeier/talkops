<script lang="ts">
	import { api, upload } from '#lib/api.ts';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';
	import { onMount } from 'svelte';
	import ErrorBox from '#lib/components/ErrorBox.svelte';

	type CiscoLocale = { name: string; lang_code: string; version: string };
	let locales = $state<CiscoLocale[]>([]),
		file = $state<FileList | null>(null),
		uploading = $state(false),
		error = $state('');

	async function load() {
		try {
			locales = await api.get<CiscoLocale[]>('/phone-locales/cisco');
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		const chosen = file?.[0];
		if (!chosen) return;
		error = '';
		uploading = true;
		try {
			const data = new FormData();
			data.append('file', chosen);
			await upload<CiscoLocale>('/phone-locales/cisco', data);
			file = null;
			await load();
		} catch (err) {
			error = errorMessage(err);
		} finally {
			uploading = false;
		}
	}

	async function remove(locale: CiscoLocale) {
		if (!confirm(t('common.confirmDelete', { name: locale.name }))) return;
		try {
			await api.del(`/phone-locales/cisco/${encodeURIComponent(locale.name)}`);
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}
</script>

<section class="card space-y-3">
	<h2>{t('ciscoLocale.title')}</h2>
	<p class="text-sm text-slate-600 dark:text-slate-300">{t('ciscoLocale.hint')}</p>
	<ErrorBox {error} />
	<form class="flex flex-wrap items-end gap-2" onsubmit={submit}>
		<div>
			<label for="cisco-locale-file">{t('ciscoLocale.file')}</label>
			<input
				id="cisco-locale-file"
				class="input"
				type="file"
				accept=".sha512,.cop"
				bind:files={file}
				required
			/>
		</div>
		<button class="btn" disabled={uploading}
			>{uploading ? t('common.loading') : t('phones.upload')}</button
		>
	</form>
	{#if locales.length > 0}
		<div class="scroll-x">
			<table class="table">
				<thead>
					<tr
						><th>{t('ciscoLocale.name')}</th><th>{t('ciscoLocale.language')}</th><th
							>{t('ciscoLocale.version')}</th
						><th></th></tr
					>
				</thead>
				<tbody>
					{#each locales as locale (locale.name)}
						<tr>
							<td>{locale.name}</td>
							<td class="font-mono text-xs">{locale.lang_code}</td>
							<td class="font-mono text-xs">{locale.version}</td>
							<td class="text-right">
								<button class="btn btn-sm btn-danger" onclick={() => remove(locale)}
									>{t('common.delete')}</button
								>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
	{/if}
</section>
