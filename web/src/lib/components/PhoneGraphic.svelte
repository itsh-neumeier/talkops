<script lang="ts">
	import type { PhoneModel } from '#lib/api.ts';
	let { model, small = false }: { model?: PhoneModel; small?: boolean } = $props();
	const layout = $derived(model?.layout);
	const rightOnly = $derived(!!layout && layout.left === 0 && layout.right > 0);
	const touch = $derived(layout?.touch ?? false);
	const screen = $derived(
		touch
			? { x: 70, y: 23, w: 112, h: 70 }
			: { x: 85, y: 28, w: rightOnly ? 74 : 82, h: rightOnly ? 55 : 48 }
	);
</script>

<svg
	viewBox="0 0 220 174"
	role="img"
	aria-label={model?.name ?? ''}
	class={small ? 'h-12 w-16 shrink-0' : 'mx-auto w-full max-w-60'}
>
	<title>{model?.name ?? ''}</title>
	<path
		d="M16 23 Q16 14 26 14 H188 Q204 14 204 30 L212 152 Q213 162 202 162 H20 Q8 162 10 150Z"
		fill="currentColor"
		class="text-slate-700 dark:text-slate-500"
	/>
	<rect
		x="18"
		y="9"
		width="39"
		height="151"
		rx="13"
		fill="currentColor"
		class="text-slate-600 dark:text-slate-400"
		stroke="currentColor"
		stroke-width="2"
	/>
	<path
		d="M25 38 Q37 43 50 38 M25 125 Q37 120 50 125"
		fill="none"
		stroke="currentColor"
		class="text-slate-400 dark:text-slate-600"
		stroke-width="3"
	/>
	<rect
		x={screen.x - 4}
		y={screen.y - 4}
		width={screen.w + 8}
		height={screen.h + 8}
		rx="4"
		fill="currentColor"
		class="text-slate-950 dark:text-slate-800"
	/>
	<rect
		x={screen.x}
		y={screen.y}
		width={screen.w}
		height={screen.h}
		rx="2"
		fill="currentColor"
		class={layout?.color
			? 'text-teal-100 dark:text-teal-900'
			: 'text-slate-200 dark:text-slate-300'}
	/>
	{#if touch}
		{#each Array.from({ length: 9 }, (_, i) => i) as i (i)}
			<rect
				x={screen.x + 6 + (i % 3) * 34}
				y={screen.y + 12 + Math.floor(i / 3) * 17}
				width="29"
				height="12"
				rx="2"
				fill="currentColor"
				class="text-teal-600/30 dark:text-teal-300/40"
			/>
		{/each}
	{:else if layout}
		{#each Array.from({ length: layout.left }, (_, i) => i) as i (i)}<rect
				x={screen.x - 18}
				y={screen.y + 5 + i * ((screen.h - 8) / layout.left)}
				width="10"
				height="5"
				rx="2"
				fill="currentColor"
				class="text-slate-400 dark:text-slate-200"
			/>{/each}
		{#each Array.from({ length: layout.right }, (_, i) => i) as i (i)}<rect
				x={screen.x + screen.w + 8}
				y={screen.y + 5 + i * ((screen.h - 8) / layout.right)}
				width="10"
				height="5"
				rx="2"
				fill="currentColor"
				class="text-slate-400 dark:text-slate-200"
			/>{/each}
	{/if}
	{#each Array.from({ length: 12 }, (_, i) => i) as i (i)}<rect
			x={79 + (i % 3) * 17}
			y={104 + Math.floor(i / 3) * 12}
			width="12"
			height="7"
			rx="2"
			fill="currentColor"
			class="text-slate-400 dark:text-slate-200"
		/>{/each}
	<circle
		cx="151"
		cy="121"
		r="15"
		fill="currentColor"
		class="text-slate-500 dark:text-slate-300"
	/><circle
		cx="151"
		cy="121"
		r="6"
		fill="currentColor"
		class="text-slate-300 dark:text-slate-500"
	/>
	{#each [0, 1, 2, 3] as i (i)}<rect
			x="181"
			y={103 + i * 12}
			width="11"
			height="7"
			rx="2"
			fill="currentColor"
			class="text-slate-400 dark:text-slate-200"
		/>{/each}
	{#if model?.id === 'vp59'}<circle
			cx="125"
			cy="13"
			r="5"
			fill="currentColor"
			class="text-slate-300 dark:text-slate-900"
		/>{/if}
</svg>
