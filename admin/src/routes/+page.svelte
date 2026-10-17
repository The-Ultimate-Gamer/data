<script lang="ts">
	import { resolve } from '$app/paths';
	import { statusLabel } from '#lib/forms.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();

	const nextIndex = $derived(data.steps.findIndex((s) => !s.done));
</script>

<h1>Dashboard</h1>

{#if data.active.length}
	<section class="stack">
		{#each data.active as t (t.id)}
			<div class="panel spread live">
				<div>
					<p class="row">
						<span class="badge {t.status}">{statusLabel[t.status]}</span>
						{#if t.is_demo}<span class="badge">Demo</span>{/if}
					</p>
					<h2>{t.name}</h2>
					<p class="subtle">{t.players} players · {t.rounds_done}/{t.positions} games played</p>
				</div>
				<div class="row">
					<a class="btn" href={resolve('/tournaments/[slug]', { slug: t.slug })}>Manage</a>
					{#if t.nextRound && t.status === 'ongoing'}
						<a
							class="btn primary big"
							href={resolve('/tournaments/[slug]/round/[number]', {
								slug: t.slug,
								number: String(t.nextRound)
							})}>▶ Enter game {t.nextRound}</a
						>
					{/if}
				</div>
			</div>
		{/each}
	</section>
{/if}

<section class="panel">
	<div class="spread">
		<h2>{nextIndex === -1 ? '✓ All set' : 'Getting started'}</h2>
		<span class="subtle">{data.steps.filter((s) => s.done).length} of {data.steps.length} done</span
		>
	</div>
	<ol class="checklist">
		{#each data.steps as step, i (step.title)}
			<li class:done={step.done} class:next={i === nextIndex}>
				<span class="mark">{step.done ? '✓' : i + 1}</span>
				<div class="text">
					<strong>{step.title}</strong>
					<span class="subtle">{step.detail}</span>
				</div>
				{#if i === nextIndex}
					<a class="btn primary" href={step.href}>{step.action} →</a>
				{:else}
					<a class="btn quiet" href={step.href}>{step.done ? 'Open' : step.action}</a>
				{/if}
			</li>
		{/each}
	</ol>
	<p class="hint">
		During the LAN you mostly live on the game screen: open it with “Enter game”, record the result,
		press Finish game, and publish whenever you want the website updated.
	</p>
</section>

{#if !data.hasReal}
	<section class="panel demo">
		<div>
			<h2>{data.demo ? '🎭 Demo data is on' : 'Want to show it off first?'}</h2>
			<p class="muted">
				{data.demo
					? 'Fictional players and tournaments are loaded. Publish to show them on the website; remove them before the real tournament.'
					: 'Load a fictional demo tournament to show your friends what the website will look like — removable with one click.'}
			</p>
		</div>
		<a class="btn" href={resolve('/demo')}>{data.demo ? 'Manage demo data' : 'Add demo data'}</a>
	</section>
{/if}

<style>
	.live {
		border-color: rgb(61 220 132 / 0.35);
	}

	.big {
		padding: 0.8rem 1.4rem;
		font-size: var(--fs-md);
	}

	.checklist {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: var(--sp-2);
	}

	.checklist li {
		display: grid;
		grid-template-columns: auto 1fr auto;
		align-items: center;
		gap: var(--sp-3);
		padding: var(--sp-3) var(--sp-4);
		border: var(--border);
		border-radius: var(--radius-md);
	}

	.checklist li.next {
		border-color: var(--c-gold-line);
		background: var(--c-gold-soft);
	}

	.checklist li.done .text strong {
		color: var(--c-text-muted);
	}

	.mark {
		display: grid;
		place-items: center;
		width: 1.8rem;
		height: 1.8rem;
		border-radius: 50%;
		border: var(--border-strong);
		font-weight: 800;
		font-size: var(--fs-sm);
	}

	.done .mark {
		background: var(--c-live);
		border-color: var(--c-live);
		color: #04140b;
	}

	.next .mark {
		background: var(--c-gold);
		border-color: var(--c-gold);
		color: var(--c-on-gold);
	}

	.text {
		display: grid;
	}

	.quiet {
		background: transparent;
	}

	.demo {
		grid-template-columns: 1fr auto;
		align-items: center;
	}
</style>
