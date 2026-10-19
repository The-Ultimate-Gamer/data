<script lang="ts">
	import { enhance } from '$app/forms';
	import { resolve } from '$app/paths';
	import { keep } from '#lib/forms.ts';
	import Feedback from '#lib/components/Feedback.svelte';
	import ResultsEntry from '#lib/components/ResultsEntry.svelte';
	import Standings from '#lib/components/Standings.svelte';
	import type { PageProps } from './$types';

	let { data, form }: PageProps = $props();

	const t = $derived(data.tournament);
	const completed = $derived(!!data.round?.completed);
	const gameName = $derived(Object.fromEntries(data.games.map((g) => [g.id, g.name])));
	const playerName = $derived(Object.fromEntries(data.participants.map((p) => [p.id, p.name])));
	const inCategory = $derived(
		data.games.filter((g) => g.categories.includes(data.category.slug) && g.available)
	);
	const otherGames = $derived(data.games.filter((g) => !inCategory.includes(g)));
	const previousWinnerId = $derived(
		data.participants.find((p) => p.slug === data.previousWinner?.slug)?.id ?? null
	);
	const nominateLeft = $derived(
		Object.fromEntries(
			data.participants.map((p) => [p.id, t.nominate_tokens - (data.usage.nominate[p.id] ?? 0)])
		)
	);
	const doubleDownLeft = $derived(
		Object.fromEntries(
			data.participants.map((p) => [
				p.id,
				t.double_down_tokens - (data.usage.doubleDown[p.id] ?? 0)
			])
		)
	);

	let source = $state<'winner' | 'vote' | 'token'>('winner');
	const route = (n: number) =>
		resolve('/tournaments/[slug]/round/[number]', { slug: t.slug, number: String(n) });
</script>

<div class="spread">
	<div class="stack head">
		<a href={resolve('/tournaments/[slug]', { slug: t.slug })} class="subtle">← {t.name}</a>
		<h1>Game {data.number} · {data.category.icon} {data.category.name}</h1>
		{#if !completed}
			<p class="subtle">
				① Pick the game · ② Enter the results · ③ Finish game · then publish when you like
			</p>
		{/if}
	</div>
	<nav class="row" aria-label="Games">
		{#if data.number > 1}<a class="btn" href={route(data.number - 1)}>← Game {data.number - 1}</a
			>{/if}
		{#if data.number < data.lastPosition}<a class="btn" href={route(data.number + 1)}
				>Game {data.number + 1} →</a
			>{/if}
	</nav>
</div>

<Feedback {form} />

{#if data.outcome}
	<div class="panel outcome" class:decided={data.outcome.decidedTournament}>
		{#if data.outcome.decidedTournament}
			<p class="big-line">
				👑 {data.outcome.winner?.name} won as a Challenger — the Ultimate Gamer!
			</p>
			<a class="btn primary" href={resolve('/tournaments/[slug]', { slug: t.slug })}
				>Finish the tournament →</a
			>
		{:else}
			<p class="big-line">🏆 {data.outcome.winner?.name} won game {data.number}.</p>
			{#if data.outcome.newChallengers.length}
				<p>
					⚔ New Challenger{data.outcome.newChallengers.length > 1 ? 's' : ''}:
					<strong>{data.outcome.newChallengers.map((p) => p.name).join(', ')}</strong>
				</p>
			{/if}
			{#if data.number < data.lastPosition && !data.champion}
				<a class="btn primary" href={route(data.number + 1)}>Next: game {data.number + 1} →</a>
			{/if}
		{/if}
	</div>
{/if}

<div class="cols">
	<section class="panel">
		<h2>1 · Game selection</h2>
		{#if data.nominations.length}
			<table>
				<tbody>
					{#each data.nominations as n (n.id)}
						<tr class:picked={data.round?.game_id === n.game_id}>
							<td><strong>{gameName[n.game_id]}</strong></td>
							<td class="subtle">
								{n.source === 'winner'
									? 'Winner’s pick'
									: n.source === 'vote'
										? 'Vote'
										: 'Nominate token'}
								{#if n.player_id}· {playerName[n.player_id]}{/if}
								{#if n.entries > 1}×{n.entries}{/if}
							</td>
							<td>
								{#if !completed}
									<form method="POST" action="?/unnominate" use:enhance={keep}>
										<input type="hidden" name="id" value={n.id} />
										<button class="small danger" aria-label="Remove nomination">✕</button>
									</form>
								{/if}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{:else}
			<p class="muted">No nominations recorded yet (optional).</p>
		{/if}

		{#if !completed}
			<form method="POST" action="?/nominate" use:enhance={keep} class="row">
				<label
					>Game
					<select name="game" required>
						<optgroup label="{data.category.name} games">
							{#each inCategory as g (g.id)}<option value={g.id}>{g.name}</option>{/each}
						</optgroup>
						<optgroup label="Other games">
							{#each otherGames as g (g.id)}<option value={g.id}>{g.name}</option>{/each}
						</optgroup>
					</select>
				</label>
				<label
					>Source
					<select name="source" bind:value={source}>
						<option value="winner">Winner’s pick</option>
						<option value="vote">Vote</option>
						<option value="token">Nominate token</option>
					</select>
				</label>
				{#if source !== 'vote'}
					<label
						>Player
						<select name="player" required>
							{#each data.participants as p (p.id)}
								<option
									value={p.id}
									selected={source === 'winner' && p.id === previousWinnerId}
									disabled={source === 'token' && nominateLeft[p.id] <= 0}
									>{p.name}{source === 'token' ? ` (${nominateLeft[p.id]} left)` : ''}</option
								>
							{/each}
						</select>
					</label>
				{/if}
				<label>Entries <input type="number" name="entries" min="1" value="1" /></label>
				<button>Add nomination</button>
			</form>
			{#if data.nominations.length}
				<form method="POST" action="?/draw" use:enhance={keep}>
					<button class="primary">🎲 Draw at random</button>
				</form>
			{/if}
		{/if}

		<form method="POST" action="?/setGame" use:enhance={keep} class="row">
			<label
				>Game being played
				<select
					name="game"
					value={data.round?.game_id ?? ''}
					disabled={completed}
					onchange={(e) => e.currentTarget.form?.requestSubmit()}
				>
					<option value="">— not decided —</option>
					<optgroup label="{data.category.name} games">
						{#each inCategory as g (g.id)}<option value={g.id}>{g.name}</option>{/each}
					</optgroup>
					<optgroup label="Other games">
						{#each otherGames as g (g.id)}<option value={g.id}>{g.name}</option>{/each}
					</optgroup>
				</select>
			</label>
			{#if !completed}<button>Set game</button>{/if}
		</form>
		<p class="hint">
			Missing a game? Add it on the <a href={resolve('/games')}>Games</a> page. Draws you do in person
			can be recorded with “Set game”.
		</p>
	</section>

	<section class="panel">
		<h2>Current standings</h2>
		<Standings
			standings={data.standings}
			highlight={data.outcome?.newChallengers.map((p) => p.slug) ?? []}
		/>
	</section>
</div>

{#if !completed && data.participants.length >= 2}
	<section class="panel">
		<div class="spread">
			<h2>Nemeses for this game</h2>
			<form method="POST" action="?/drawNemeses" use:enhance={keep}>
				<button class="small">🎲 Draw at random</button>
			</form>
		</div>
		<p class="hint">
			Nemeses are public: the pairs are shown on the website until the game is played. Anyone but
			yourself can be drawn, so two players can share a Nemesis and pairs don’t have to be mutual.
			They are copied into the results below.
		</p>
		<form method="POST" action="?/nemeses" use:enhance={keep} class="stack">
			<div class="row">
				{#each data.participants as p (p.id)}
					<label
						>{p.name}
						<select name="nemesis_{p.id}" value={data.nemeses[p.id] ?? ''}>
							<option value="">—</option>
							{#each data.participants.filter((o) => o.id !== p.id) as o (o.id)}
								<option value={o.id}>{o.name}</option>
							{/each}
						</select>
					</label>
				{/each}
			</div>
			<div class="row"><button>Save Nemeses</button></div>
		</form>
	</section>
{/if}

<section class="panel">
	<div class="spread">
		<h2>2 · Results</h2>
		{#if completed}
			<form method="POST" action="?/reopen" use:enhance={keep}>
				<button class="small">Reopen to edit</button>
			</form>
		{/if}
	</div>
	{#if data.participants.length < 2}
		<p class="muted">Register at least two players for this tournament first.</p>
	{:else}
		{#key `${data.number}-${completed}-${JSON.stringify(data.nemeses)}`}
			<ResultsEntry
				participants={data.participants}
				results={data.results}
				nemeses={data.nemeses}
				points={data.points}
				{doubleDownLeft}
				{completed}
				hasGame={!!data.round?.game_id}
			/>
		{/key}
	{/if}
</section>

<style>
	.head {
		gap: var(--sp-2);
	}

	.outcome {
		border-color: rgb(61 220 132 / 0.4);
		background: var(--c-live-soft);
	}

	.outcome.decided {
		border-color: var(--c-gold-line);
		background: var(--c-gold-soft);
	}

	.big-line {
		font-family: var(--font-display);
		font-size: var(--fs-xl);
		font-weight: 700;
	}

	tr.picked td {
		background: var(--c-gold-soft);
	}
</style>
