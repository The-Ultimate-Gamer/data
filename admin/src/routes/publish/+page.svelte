<script lang="ts">
	import { enhance, type SubmitFunction } from '$app/forms';
	import { keep } from '#lib/forms.ts';
	import Feedback from '#lib/components/Feedback.svelte';
	import type { PageProps } from './$types';

	let { data, form }: PageProps = $props();

	let busy = $state(false);
	const submitting: SubmitFunction = () => {
		busy = true;
		return async ({ update }) => {
			await update({ reset: false });
			busy = false;
		};
	};
</script>

<h1>Publish</h1>
<Feedback {form} />

<div class="cols">
	<section class="panel">
		<h2>Validation</h2>
		{#if data.validation.errors.length}
			<p class="badge danger">{data.validation.errors.length} error(s)</p>
			<ul class="errors">
				{#each data.validation.errors as e, i (i)}<li>{e}</li>{/each}
			</ul>
		{:else}
			<p><span class="badge live">✓ All checks pass</span></p>
		{/if}
		{#if data.validation.warnings.length}
			<ul class="warnings">
				{#each data.validation.warnings as w, i (i)}<li>{w}</li>{/each}
			</ul>
		{/if}
		<form method="POST" action="?/export" use:enhance={keep}>
			<button>Re-export &amp; validate</button>
		</form>
	</section>

	<section class="panel">
		<h2>Changes</h2>
		{#if !data.git}
			<p class="muted">Publishing is only available for the real database.</p>
		{:else if data.git.error}
			<p class="errors">{data.git.error}</p>
		{:else}
			<p class="subtle">
				Branch <strong class="num">{data.git.branch}</strong>{#if data.git.ahead}
					· {data.git.ahead} commit(s) not pushed{/if}
			</p>
			{#if data.git.changes.length}
				<ul class="files num">
					{#each data.git.changes as c (c.file)}<li>
							<span class="st">{c.status}</span>
							{c.file}
						</li>{/each}
				</ul>
			{:else}
				<p class="muted">No uncommitted changes in database/ or data/.</p>
			{/if}

			<form method="POST" action="?/publish" use:enhance={submitting} class="stack">
				<label>Commit message <input name="message" value={data.defaultMessage} /></label>
				<div class="row">
					<button
						class="primary"
						disabled={busy ||
							!!data.validation.errors.length ||
							(!data.git.changes.length && !data.git.ahead)}
					>
						{busy ? 'Working…' : '🚀 Commit & publish'}
					</button>
					<button formaction="?/commit" disabled={busy || !data.git.changes.length}
						>Commit only</button
					>
				</div>
			</form>
			<p class="hint">
				Publishing commits <code>database/</code> and <code>data/</code> and pushes to GitHub. The live
				site updates after the GitHub Pages deployment (usually a few minutes; CDN caching can add up
				to ~10).
			</p>
		{/if}
	</section>
</div>

<style>
	ul {
		margin: 0;
		padding-left: 1.2rem;
		font-size: var(--fs-sm);
	}

	.errors {
		color: var(--c-danger);
	}

	.warnings {
		color: var(--c-gold-bright);
	}

	.files {
		list-style: none;
		padding: 0;
		max-height: 16rem;
		overflow: auto;
	}

	.st {
		display: inline-block;
		width: 1.5rem;
		color: var(--c-gold);
	}
</style>
