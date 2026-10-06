<script lang="ts">
	import { onMount } from 'svelte';
	import { api, type VoicemailMessage } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import TranscriptView from '#lib/components/TranscriptView.svelte';
	import { t } from '#lib/i18n/index.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	let { extensionId }: { extensionId: string } = $props();

	let messages = $state<VoicemailMessage[] | null>(null);
	let error = $state('');

	async function load() {
		try {
			messages = await api.get<VoicemailMessage[]>(
				`/voicemail/messages?extension_id=${extensionId}`
			);
		} catch (err) {
			error = errorMessage(err);
		}
	}
	onMount(load);

	async function setHeard(m: VoicemailMessage, heard: boolean) {
		try {
			await api.put(`/voicemail/messages/${m.id}`, { heard });
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	async function remove(m: VoicemailMessage) {
		if (!confirm(t('vm.deleteConfirm'))) return;
		try {
			await api.del(`/voicemail/messages/${m.id}`);
			await load();
		} catch (err) {
			error = errorMessage(err);
		}
	}

	function played(m: VoicemailMessage) {
		if (m.status === 'new') setHeard(m, true);
	}

	const duration = (s: number) => `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
</script>

<div class="space-y-3">
	<h2>{t('vm.messages')}</h2>
	<ErrorBox {error} />
	{#if messages && messages.length === 0}
		<p class="text-sm text-slate-500">{t('vm.noMessages')}</p>
	{/if}
	<ul class="divide-y divide-slate-100 dark:divide-slate-800">
		{#each messages ?? [] as m (m.id)}
			<li class="space-y-2 py-3">
				<div class="flex flex-wrap items-center justify-between gap-2">
					<div>
						<span class="font-medium">{m.caller_name || m.caller_number || t('vm.unknown')}</span>
						{#if m.caller_name && m.caller_number}<span class="font-mono text-sm text-slate-500">
								{m.caller_number}</span
							>{/if}
						{#if m.status === 'new'}<span class="badge badge-ok">{t('vm.new')}</span>{/if}
						<div class="text-sm text-slate-500">
							{new Date(m.created_at).toLocaleString()} · {duration(m.duration_secs)}
						</div>
					</div>
					<div class="space-x-1">
						{#if m.status === 'new'}
							<button class="btn btn-sm" onclick={() => setHeard(m, true)}
								>{t('vm.markHeard')}</button
							>
						{:else}
							<button class="btn btn-sm" onclick={() => setHeard(m, false)}
								>{t('vm.markNew')}</button
							>
						{/if}
						<a class="btn btn-sm" href="/api/v1/voicemail/messages/{m.id}/audio" download
							>{t('vm.download')}</a
						>
						<button class="btn btn-sm btn-danger" onclick={() => remove(m)}
							>{t('common.delete')}</button
						>
					</div>
				</div>
				<audio
					class="w-full"
					controls
					preload="none"
					src="/api/v1/voicemail/messages/{m.id}/audio"
					onplay={() => played(m)}
				></audio>
				{#if m.transcript_status !== 'none'}
					<TranscriptView
						url="/voicemail/messages/{m.id}/transcript"
						status={m.transcript_status}
					/>
				{/if}
			</li>
		{/each}
	</ul>
</div>
