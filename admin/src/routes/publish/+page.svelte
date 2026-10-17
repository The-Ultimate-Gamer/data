<script lang="ts">
	import { enhance } from '$app/forms';
	import type { SubmitFunction } from '$app/forms';
	import { refreshAll } from '$app/navigation';
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

	const errors = $derived(data.validation.errors);
	const deploy = $derived(data.deployment);
	// Waiting for GitHub: pushed, but the latest finished deployment isn't for this commit yet.
	const deploying = $derived(
		!!data.git &&
			!data.unpublished &&
			!!deploy &&
			deploy.state !== 'unknown' &&
			(deploy.state === 'running' || !deploy.current)
	);
	const live = $derived(!data.unpublished && deploy?.state === 'success' && deploy.current);

	$effect(() => {
		if (!deploying) return;
		const id = setInterval(() => refreshAll(), 8000);
		return () => clearInterval(id);
	});

	const statusText = { new: 'New', changed: 'Updated', removed: 'Removed' } as const;

	// "Player profile: nova" + "Player profile: kite" → one line "Player profiles: nova, kite".
	const grouped = $derived.by(() => {
		const lines = new Map<
			string,
			{ key: string; status: keyof typeof statusText; title: string; items: string[] }
		>();
		for (const c of data.git?.changes ?? []) {
			const m = c.label.match(/^(Player profile|Tournament page): (.+)$/);
			const key = `${c.status}|${m ? m[1] : 'pages'}`;
			const line = lines.get(key) ?? {
				key,
				status: c.status,
				title: m ? `${m[1]}s` : 'Pages',
				items: []
			};
			line.items.push(m ? m[2] : c.label);
			lines.set(key, line);
		}
		return [...lines.values()];
	});
</script>

<h1>Publish to the website</h1>

<p class="intro muted">
	Everything you do in this app is saved on your laptop straight away. The website only changes when
	you press <strong>Publish</strong>: your changes are then sent to GitHub, which updates the
	website within a few minutes.
</p>

{#if form?.error}
	<div class="alert" role="alert">{form.error}</div>
{:else if form?.message}
	<div class="ok" role="status">{form.message}</div>
{/if}

{#if !data.git}
	<section class="panel">
		<p>
			<span class="badge danger">Practice database</span> You’re using a practice database, so
			publishing is turned off. Start the admin with <code>npm start</code> to work on the real data.
		</p>
	</section>
{:else}
	<ol class="steps">
		<li class="done">
			<span class="icon">✓</span>
			<div><strong>Saved</strong><span>on this laptop</span></div>
		</li>
		<li class:done={!errors.length} class:bad={errors.length}>
			<span class="icon">{errors.length ? '!' : '✓'}</span>
			<div>
				<strong>Checked</strong><span
					>{errors.length ? `${errors.length} problem(s) to fix` : 'data looks good'}</span
				>
			</div>
		</li>
		<li class:done={!data.unpublished} class:todo={data.unpublished}>
			<span class="icon">{data.unpublished ? '3' : '✓'}</span>
			<div>
				<strong>Published</strong><span
					>{data.unpublished ? 'waiting for you' : 'sent to GitHub'}</span
				>
			</div>
		</li>
		<li
			class:done={live}
			class:running={deploying}
			class:bad={!data.unpublished && deploy?.state === 'failed' && deploy.current}
		>
			<span class="icon">{live ? '✓' : deploying ? '⟳' : '4'}</span>
			<div>
				<strong>Live</strong>
				<span>
					{#if live}on the website
					{:else if deploying}updating the website…
					{:else if deploy?.state === 'failed' && deploy.current}update failed
					{:else if deploy?.state === 'unknown'}check GitHub
					{:else}after publishing{/if}
				</span>
			</div>
		</li>
	</ol>

	{#if errors.length}
		<section class="panel problems">
			<h2>Fix these first</h2>
			<p class="muted">
				The data has inconsistencies that would show wrong results on the website. Fix them in the
				admin (usually on the tournament or round page), then come back.
			</p>
			<ul>
				{#each errors as e, i (i)}<li>{e}</li>{/each}
			</ul>
		</section>
	{/if}

	{#if data.unpublished}
		<section class="panel">
			<h2>What will change on the website</h2>
			{#if grouped.length}
				<ul class="changes">
					{#each grouped as g (g.key)}
						<li>
							<span
								class="badge {g.status === 'removed' ? 'danger' : g.status === 'new' ? 'live' : ''}"
								>{statusText[g.status]}</span
							>
							<strong>{g.title}:</strong>
							{g.items.join(', ')}
						</li>
					{/each}
				</ul>
			{:else}
				<p class="muted">
					Behind-the-scenes database changes (nothing visible changes on the website yet).
				</p>
			{/if}
			<form method="POST" action="?/publish" use:enhance={submitting} class="stack">
				<button class="primary huge" disabled={busy || !!errors.length}>
					{busy ? 'Publishing…' : '🚀 Publish to website'}
				</button>
				<details>
					<summary class="subtle">Advanced</summary>
					<label
						>Description of this update (optional) <input
							name="message"
							placeholder="Auto-generated from the changes above"
						/></label
					>
				</details>
			</form>
		</section>
	{:else}
		<section class="panel">
			{#if live}
				<p class="big">✓ The website is up to date.</p>
			{:else if deploying}
				<p class="big">⟳ GitHub is updating the website…</p>
				<p class="muted">
					This usually takes 1–2 minutes. This page refreshes by itself. Visitors may see the old
					version for a few more minutes because of caching.
				</p>
			{:else if deploy?.state === 'failed' && deploy.current}
				<p class="big bad">The website update failed.</p>
				<p class="muted">Your data is safe on GitHub. Open the details to see what went wrong.</p>
			{:else}
				<p class="big">Nothing to publish — everything is already sent.</p>
			{/if}
			<p class="row">
				<a class="btn" href={data.siteUrl} target="_blank" rel="noreferrer">Open the website ↗</a>
				{#if deploy?.url}<a class="btn" href={deploy.url} target="_blank" rel="noreferrer"
						>Deployment details ↗</a
					>
				{:else if data.git.actionsUrl}<a
						class="btn"
						href={data.git.actionsUrl}
						target="_blank"
						rel="noreferrer">Deployment details ↗</a
					>{/if}
			</p>
		</section>
	{/if}

	{#if data.validation.warnings.length}
		<details class="panel">
			<summary>Notes ({data.validation.warnings.length})</summary>
			<ul class="warnings">
				{#each data.validation.warnings as w, i (i)}<li>{w}</li>{/each}
			</ul>
		</details>
	{/if}
{/if}

<style>
	.intro {
		max-width: 65ch;
	}

	.alert,
	.ok {
		padding: var(--sp-3) var(--sp-4);
		border-radius: var(--radius-md);
		white-space: pre-wrap;
		font-size: var(--fs-sm);
	}

	.alert {
		border: 1px solid rgb(255 84 104 / 0.45);
		background: var(--c-danger-soft);
	}

	.ok {
		border: 1px solid rgb(61 220 132 / 0.4);
		background: var(--c-live-soft);
	}

	.steps {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(11rem, 1fr));
		gap: var(--sp-3);
	}

	.steps li {
		display: flex;
		align-items: center;
		gap: var(--sp-3);
		padding: var(--sp-3) var(--sp-4);
		border: var(--border);
		border-radius: var(--radius-md);
		background: var(--c-surface);
		color: var(--c-text-muted);
	}

	.steps div {
		display: grid;
		font-size: var(--fs-sm);
	}

	.steps strong {
		color: var(--c-text);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.icon {
		display: grid;
		place-items: center;
		width: 2rem;
		height: 2rem;
		border-radius: 50%;
		border: var(--border-strong);
		font-weight: 800;
		flex: none;
	}

	.done .icon {
		background: var(--c-live);
		border-color: var(--c-live);
		color: #04140b;
	}

	.todo {
		border-color: var(--c-gold-line) !important;
	}

	.todo .icon {
		background: var(--c-gold);
		border-color: var(--c-gold);
		color: var(--c-on-gold);
	}

	.running .icon {
		border-color: var(--c-info);
		color: var(--c-info);
		animation: spin 1.2s linear infinite;
	}

	.bad .icon {
		background: var(--c-danger);
		border-color: var(--c-danger);
		color: #1a0408;
	}

	.problems {
		border-color: rgb(255 84 104 / 0.45);
	}

	.problems ul {
		color: var(--c-danger);
	}

	ul {
		margin: 0;
		padding-left: 1.2rem;
		font-size: var(--fs-sm);
	}

	.changes {
		list-style: none;
		padding: 0;
		display: grid;
		gap: var(--sp-1);
		max-height: 20rem;
		overflow: auto;
	}

	.huge {
		justify-self: start;
		font-size: var(--fs-lg);
		padding: 0.9rem 1.8rem;
	}

	.big {
		font-family: var(--font-display);
		font-size: var(--fs-xl);
		font-weight: 700;
	}

	.big.bad {
		color: var(--c-danger);
	}

	.warnings {
		color: var(--c-text-muted);
		margin-top: var(--sp-3);
	}

	summary {
		cursor: pointer;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}
</style>
