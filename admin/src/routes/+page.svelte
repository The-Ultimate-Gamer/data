<script lang="ts">
	import { resolve } from '$app/paths';
	import { statusLabel } from '#lib/forms.ts';
	import type { PageProps } from './$types';

	let { data }: PageProps = $props();
</script>

<h1>Dashboard</h1>

{#if data.live.length}
	<section class="stack">
		{#each data.live as t (t.id)}
			<div class="panel spread live">
				<div>
					<p><span class="badge {t.status}">{statusLabel[t.status]}</span></p>
					<h2>{t.name}</h2>
					<p class="subtle">{t.players} players · {t.rounds_done}/{t.positions} rounds played</p>
				</div>
				<div class="row">
					<a class="btn" href={resolve('/tournaments/[slug]', { slug: t.slug })}>Manage</a>
					{#if t.nextRound}
						<a
							class="btn primary big"
							href={resolve('/tournaments/[slug]/round/[number]', {
								slug: t.slug,
								number: String(t.nextRound)
							})}>▶ Round {t.nextRound}</a
						>
					{/if}
				</div>
			</div>
		{/each}
	</section>
{:else}
	<section class="panel">
		<p class="muted">No upcoming or live tournament.</p>
		<p><a class="btn primary" href={resolve('/tournaments')}>Create a tournament</a></p>
	</section>
{/if}

<div class="cols">
	<a class="panel tile" href={resolve('/tournaments')}
		><span class="n num">{data.counts.tournaments}</span> Tournaments</a
	>
	<a class="panel tile" href={resolve('/players')}
		><span class="n num">{data.counts.players}</span> Players</a
	>
	<a class="panel tile" href={resolve('/games')}
		><span class="n num">{data.counts.games}</span> Games</a
	>
	<a class="panel tile" href={resolve('/publish')}>
		{#if data.errors}<span class="n num err">{data.errors}</span> Validation errors
		{:else}<span class="n ok">✓</span> Data valid{#if data.warnings}
				({data.warnings} warnings){/if}{/if}
	</a>
</div>

<section class="panel">
	<h2>Typical flow</h2>
	<ol class="muted flow">
		<li>
			<strong>Before the LAN:</strong> add players and games, create the tournament, register players,
			set the category order.
		</li>
		<li><strong>Start:</strong> open the tournament and press “Start tournament (go live)”.</li>
		<li>
			<strong>Each game:</strong> open the round, record nominations and the drawn game, tap players in
			finishing order, tick Double Downs and Nemeses, then “Finish game”.
		</li>
		<li><strong>Publish</strong> whenever you want the website to update.</li>
		<li>
			<strong>The end:</strong> when a Challenger wins, mark the tournament completed and grant the awards.
		</li>
	</ol>
</section>

<style>
	.live {
		border-color: rgb(61 220 132 / 0.35);
	}

	.big {
		padding: 0.8rem 1.4rem;
		font-size: var(--fs-md);
	}

	.tile {
		color: var(--c-text);
		font-weight: 600;
		grid-template-columns: auto 1fr;
		align-items: center;
	}

	.tile:hover {
		border-color: var(--c-gold-line);
		color: var(--c-text);
	}

	.n {
		font-size: var(--fs-2xl);
		font-weight: 800;
		color: var(--c-gold);
	}

	.n.err {
		color: var(--c-danger);
	}

	.n.ok {
		color: var(--c-live);
	}

	.flow {
		margin: 0;
		padding-left: 1.2rem;
		display: grid;
		gap: var(--sp-2);
	}
</style>
