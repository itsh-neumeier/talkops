<script lang="ts">
	import { onMount } from 'svelte';
	import { api, upload, type MediaKind, type PhoneMedia } from '#lib/api.ts';
	import { decode, encodeWav } from '#lib/audio.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	/** Ringtones: 8 kHz mono 16-bit WAV (Yealink custom ringtone format). */
	const RING_RATE = 8000;
	/** Wallpapers are scaled to fit this box (below every model's limit). */
	const MAX_W = 1280;
	const MAX_H = 800;

	let media = $state<PhoneMedia[]>([]);
	let error = $state('');
	let kind = $state<MediaKind>('ringtone');
	let name = $state('');
	let seconds = $state(6);
	let files = $state<FileList | null>(null);
	let busy = $state(false);

	const fileUrl = (m: PhoneMedia) => `/api/v1/phone-media/${m.id}/file`;

	async function load() {
		try {
			media = await api.get<PhoneMedia[]>('/phone-media');
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);

	/** Scales an image to fit MAX_W × MAX_H and encodes it as JPEG. */
	async function toJpeg(file: File): Promise<Blob> {
		const bitmap = await createImageBitmap(file);
		const scale = Math.min(1, MAX_W / bitmap.width, MAX_H / bitmap.height);
		const canvas = document.createElement('canvas');
		canvas.width = Math.round(bitmap.width * scale);
		canvas.height = Math.round(bitmap.height * scale);
		canvas.getContext('2d')!.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
		bitmap.close();
		return new Promise((resolve, reject) =>
			canvas.toBlob((b) => (b ? resolve(b) : reject(new Error('encode'))), 'image/jpeg', 0.9)
		);
	}

	async function add(e: SubmitEvent) {
		e.preventDefault();
		const file = files?.[0];
		if (!file) return;
		error = '';
		busy = true;
		try {
			const data =
				kind === 'ringtone'
					? encodeWav(await decode(await file.arrayBuffer(), RING_RATE, seconds))
					: await toJpeg(file);
			const form = new FormData();
			form.append('kind', kind);
			form.append('name', name.trim() || file.name.replace(/\.[^.]+$/, ''));
			form.append('file', data, kind === 'ringtone' ? 'ring.wav' : 'wallpaper.jpg');
			await upload<PhoneMedia>('/phone-media', form);
			name = '';
			files = null;
			await load();
		} catch (err) {
			error = err instanceof DOMException ? t('media.unreadable') : errorMessage(err);
		} finally {
			busy = false;
		}
	}

	async function remove(m: PhoneMedia) {
		if (!confirm(t('common.confirmDelete', { name: m.name }))) return;
		try {
			await api.del(`/phone-media/${m.id}`);
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	const size = (b: number) =>
		b < 1048576 ? `${Math.ceil(b / 1024)} KB` : `${(b / 1048576).toFixed(1)} MB`;
</script>

<section class="card space-y-3">
	<h2>{t('media.title')}</h2>
	<p class="text-sm text-slate-600 dark:text-slate-300">{t('media.hint')}</p>
	<ErrorBox {error} />
	<form class="flex flex-wrap items-end gap-2" onsubmit={add}>
		<div>
			<label for="pm-kind">{t('media.kind')}</label>
			<select id="pm-kind" class="input" bind:value={kind} onchange={() => (files = null)}>
				<option value="ringtone">{t('media.ringtone')}</option>
				<option value="wallpaper">{t('media.wallpaper')}</option>
			</select>
		</div>
		<div>
			<label for="pm-name">{t('common.name')}</label>
			<input id="pm-name" class="input" bind:value={name} maxlength="64" />
		</div>
		{#if kind === 'ringtone'}
			<div>
				<label for="pm-len">{t('media.length')}</label>
				<select id="pm-len" class="input" bind:value={seconds}>
					<option value={6}>{t('media.lengthShort')}</option>
					<option value={15}>15 s</option>
					<option value={30}>30 s</option>
				</select>
			</div>
		{/if}
		<div>
			<label for="pm-file">{t('media.file')}</label>
			<input
				id="pm-file"
				class="input"
				type="file"
				accept={kind === 'ringtone' ? 'audio/*' : 'image/*'}
				bind:files
				required
			/>
		</div>
		<button class="btn" disabled={busy}>{busy ? t('common.loading') : t('phones.upload')}</button>
	</form>
	<p class="hint">{kind === 'ringtone' ? t('media.ringtoneHint') : t('media.wallpaperHint')}</p>
	{#if media.length > 0}
		<ul class="divide-y divide-slate-100 dark:divide-slate-800">
			{#each media as m (m.id)}
				<li class="flex flex-wrap items-center justify-between gap-2 py-2">
					<div class="flex items-center gap-3">
						{#if m.kind === 'wallpaper'}
							<img src={fileUrl(m)} alt="" class="h-10 w-16 rounded object-cover" />
						{/if}
						<div>
							<div class="font-medium">{m.name}</div>
							<div class="text-xs text-slate-500">
								{t(`media.${m.kind}`)} · {size(m.size_bytes)}
							</div>
						</div>
					</div>
					<div class="flex items-center gap-2">
						{#if m.kind === 'ringtone'}
							<audio controls preload="none" src={fileUrl(m)} class="h-8"></audio>
						{/if}
						<button class="btn btn-sm btn-danger" onclick={() => remove(m)}
							>{t('common.delete')}</button
						>
					</div>
				</li>
			{/each}
		</ul>
	{/if}
</section>
