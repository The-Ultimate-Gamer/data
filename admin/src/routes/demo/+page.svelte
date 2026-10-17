<script lang="ts">
	import { enhance } from '$app/forms';
	import { resolve } from '$app/paths';
	import Feedback from '#lib/components/Feedback.svelte';
	import type { PageProps } from './$types';

	let { data, form }: PageProps = $props();
	let busy = $state(false);
</script>

<h1>Demo data</h1>

<p class="intro muted">
	Want to show the website to your friends before the first real tournament? Add a ready-made set of
	<strong>fictional</strong> players, games and two tournaments (one finished, one in progress), publish
	it, and the website comes alive. When you’re ready for the real thing, remove it again with one click
	— your own players, games and tournaments are never touched.
</p>

<Feedback {form} />

<section class="panel state" class:on={data.present}>
	{#if data.present}
		<p class="big">🎭 Demo data is <strong>on</strong></p>
		<p class="muted">
			The website shows a “fictional demo data” banner while it’s published. Demo tournaments are
			also marked “Demo”.
		</p>
		<form
			method="POST"
			action="?/remove"
			use:enhance={({ cancel }) => {
				if (!confirm('Remove all demo players, games and tournaments?')) return cancel();
				busy = true;
				return async ({ update }) => {
					await update();
					busy = false;
				};
			}}
		>
			<button class="danger" disabled={busy}>{busy ? 'Removing…' : 'Remove demo data'}</button>
		</form>
	{:else}
		<p class="big">Demo data is <strong>off</strong></p>
		<form
			method="POST"
			action="?/add"
			use:enhance={() => {
				busy = true;
				return async ({ update }) => {
					await update();
					busy = false;
				};
			}}
		>
			<button class="primary" disabled={busy}>{busy ? 'Adding…' : '✨ Add demo data'}</button>
		</form>
	{/if}
</section>

<section class="panel">
	<h2>How it works</h2>
	<ol class="muted steps">
		<li><strong>Add demo data</strong> here. You can explore it in the admin straight away.</li>
		<li>
			<a href={resolve('/publish')}>Publish</a> to put it on the website and share the link with your
			friends.
		</li>
		<li>
			Before the first real tournament: <strong>Remove demo data</strong>, then publish again.
		</li>
	</ol>
	<p class="hint">
		Demo players are called Nova, Blitz, Echo, Rook, Vex and Kite. If you already have real players
		with those names, adding the demo is refused so nothing gets mixed up. Games with the same name
		as yours are shared and never removed.
	</p>
</section>

<style>
	.intro {
		max-width: 68ch;
	}

	.state.on {
		border-color: rgb(92 200 255 / 0.5);
		background: var(--c-info-soft);
	}

	.big {
		font-family: var(--font-display);
		font-size: var(--fs-xl);
	}

	.steps {
		margin: 0;
		padding-left: 1.2rem;
		display: grid;
		gap: var(--sp-2);
	}
</style>
