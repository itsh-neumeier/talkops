<script lang="ts">
	// Smart Attendant editor: the call flow as a tree on the left, the
	// settings of the selected step on the right.
	import { beforeNavigate, goto } from '$app/navigation';
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import { api, type Attendant } from '#lib/api.ts';
	import ErrorBox from '#lib/components/ErrorBox.svelte';
	import ContextMenu from '#lib/components/flow/ContextMenu.svelte';
	import FlowSlot from '#lib/components/flow/FlowSlot.svelte';
	import NodeSettings from '#lib/components/flow/NodeSettings.svelte';
	import { FlowEditor } from '#lib/components/flow/editor.svelte.ts';
	import { loadTargets } from '#lib/destinations.svelte.ts';
	import { createNode } from '#lib/flow.ts';
	import { i18n, t } from '#lib/i18n/index.svelte.ts';
	import { hasRole } from '#lib/session.svelte.ts';
	import { errorMessage } from '#lib/util.ts';

	const id = $derived(page.params.id ?? 'new');
	let editor = $state<FlowEditor | null>(null);
	let form = $state({ number: '', name: '', language: null as Attendant['language'] });
	let error = $state('');
	let saving = $state(false);
	let saved = $state(false);
	let snapshot = $state('');

	const current = () => JSON.stringify([form, editor?.root.flow]);
	const dirty = $derived(editor !== null && current() !== snapshot);

	onMount(async () => {
		try {
			await loadTargets();
			if (id === 'new') {
				form.name = t('flow.newName');
				editor = new FlowEditor(createNode('menu', 'start', 'start'));
			} else {
				const a = await api.get<Attendant>(`/attendants/${id}`);
				form = { number: a.number ?? '', name: a.name, language: a.language };
				editor = new FlowEditor(a.flow);
				snapshot = current();
			}
		} catch (err) {
			error = errorMessage(err);
		}
	});

	beforeNavigate((nav) => {
		if (dirty && !saving && !confirm(t('flow.unsaved'))) nav.cancel();
	});

	async function save() {
		if (!editor) return;
		error = '';
		saved = false;
		saving = true;
		try {
			await editor.flush?.();
			if (!editor.root.flow) throw new Error(t('flow.emptyFlow'));
			const body = {
				number: form.number || null,
				name: form.name,
				language: form.language,
				flow: editor.root.flow
			};
			const a =
				id === 'new'
					? await api.post<Attendant>('/attendants', body)
					: await api.put<Attendant>(`/attendants/${id}`, body);
			snapshot = current();
			saved = true;
			if (id === 'new') await goto(`/routing/attendants/${a.id}`, { replaceState: true });
		} catch (err) {
			error = errorMessage(err);
		} finally {
			saving = false;
		}
	}

	const language = $derived(form.language ?? (i18n.locale === 'en' ? 'en' : 'de'));
</script>

<div class="space-y-4">
	<div class="flex flex-wrap items-center justify-between gap-2">
		<div>
			<a href="/routing" class="text-sm text-teal-700 hover:underline dark:text-teal-300"
				>← {t('nav.routing')}</a
			>
			<h1>{t('flow.title')}</h1>
		</div>
		{#if hasRole('admin')}
			<div class="flex items-center gap-3">
				{#if saved && !dirty}<span class="text-sm text-emerald-700 dark:text-emerald-400"
						>{t('common.saved')}</span
					>{:else if dirty}<span class="text-sm text-amber-600">{t('flow.dirty')}</span>{/if}
				<button class="btn btn-primary" onclick={save} disabled={saving || !editor}
					>{t('common.save')}</button
				>
			</div>
		{/if}
	</div>
	<ErrorBox {error} />
	{#if editor}
		<div class="card grid gap-3 sm:grid-cols-[8rem_1fr_12rem]">
			<div>
				<label for="att-number">{t('routing.number')}</label>
				<input id="att-number" class="input font-mono" bind:value={form.number} placeholder="70" />
			</div>
			<div>
				<label for="att-name">{t('common.name')}</label>
				<input id="att-name" class="input" bind:value={form.name} required />
			</div>
			<div>
				<label for="att-lang">{t('vm.language')}</label>
				<select id="att-lang" class="input" bind:value={form.language}>
					<option value={null}>{t('vm.languageDefault')}</option>
					<option value="de">Deutsch</option>
					<option value="en">English</option>
				</select>
			</div>
		</div>
		<div class="grid items-start gap-4 lg:grid-cols-[1fr_24rem]">
			<div
				class="flow-canvas min-h-[28rem] overflow-auto rounded-xl border border-slate-200 p-6 dark:border-slate-700"
			>
				<div class="flex w-max min-w-full flex-col items-center">
					<span class="rounded-full bg-teal-600 px-3 py-1 text-sm font-medium text-white shadow-sm"
						>{t('flow.incoming')}</span
					>
					<div class="h-5 w-px bg-slate-300 dark:bg-slate-600"></div>
					<FlowSlot {editor} holder={editor.rootSlot.holder} key="flow" />
				</div>
			</div>
			<ContextMenu {editor} />
			<aside class="card lg:sticky lg:top-4">
				<NodeSettings {editor} attendantId={id} {language} />
			</aside>
		</div>
	{/if}
</div>

<style>
	.flow-canvas {
		background-image: radial-gradient(circle, rgb(148 163 184 / 0.35) 1px, transparent 1px);
		background-size: 18px 18px;
	}
</style>
