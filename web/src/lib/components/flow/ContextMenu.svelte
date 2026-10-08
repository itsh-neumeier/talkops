<script lang="ts">
	// Right-click menu of the flow editor; closes on Escape, scrolling or a
	// click elsewhere.
	import { t } from '#lib/i18n/index.svelte.ts';
	import type { FlowEditor, MenuItem } from './editor.svelte.ts';

	let { editor }: { editor: FlowEditor } = $props();

	let element = $state<HTMLElement>();
	let open = $state.raw<MenuItem | null>(null);
	let pos = $state({ x: 0, y: 0 });

	// Keep the menu inside the window.
	$effect(() => {
		const menu = editor.menu;
		open = null;
		if (!menu) return;
		pos = { x: menu.x, y: menu.y };
		requestAnimationFrame(() => {
			if (!element) return;
			const r = element.getBoundingClientRect();
			pos = {
				x: Math.max(4, Math.min(menu.x, window.innerWidth - r.width - 4)),
				y: Math.max(4, Math.min(menu.y, window.innerHeight - r.height - 4))
			};
			element.querySelector<HTMLButtonElement>('button')?.focus();
		});
	});

	function close() {
		editor.menu = null;
	}

	function run(item: MenuItem) {
		if (item.children) {
			open = open === item ? null : item;
			return;
		}
		close();
		item.action?.();
	}

	function onWindow(e: Event) {
		if (!editor.menu) return;
		if (e instanceof KeyboardEvent) {
			if (e.key === 'Escape') close();
			return;
		}
		if (e.target instanceof Node && element?.contains(e.target)) return;
		close();
	}
</script>

<svelte:window
	onkeydown={onWindow}
	onmousedown={onWindow}
	onscroll={onWindow}
	onresize={onWindow}
/>

{#if editor.menu}
	<div
		bind:this={element}
		class="fixed z-50 max-h-[80vh] w-60 overflow-y-auto rounded-xl border border-slate-200 bg-white p-1 text-sm shadow-lg dark:border-slate-700 dark:bg-slate-900"
		style="left: {pos.x}px; top: {pos.y}px"
		role="menu"
		tabindex="-1"
		aria-label={t('flow.menu.title')}
		oncontextmenu={(e) => e.preventDefault()}
	>
		{#each editor.menu.items as item (item.label)}
			<button
				type="button"
				role="menuitem"
				class="flex w-full items-center justify-between rounded-lg px-3 py-1.5 text-left hover:bg-slate-100 focus:bg-slate-100 focus:outline-none dark:hover:bg-slate-800 dark:focus:bg-slate-800 {item.danger
					? 'text-red-600 dark:text-red-400'
					: ''}"
				aria-expanded={item.children ? open === item : undefined}
				onclick={() => run(item)}
			>
				{item.label}
				{#if item.children}<span class="text-slate-400">{open === item ? '▾' : '▸'}</span>{/if}
			</button>
			{#if item.children && open === item}
				<div class="ml-3 border-l border-slate-200 pl-1 dark:border-slate-700">
					{#each item.children as child (child.label)}
						<button
							type="button"
							role="menuitem"
							class="w-full rounded-lg px-3 py-1 text-left hover:bg-slate-100 focus:bg-slate-100 focus:outline-none dark:hover:bg-slate-800 dark:focus:bg-slate-800"
							onclick={() => run(child)}>{child.label}</button
						>
					{/each}
				</div>
			{/if}
		{/each}
	</div>
{/if}
