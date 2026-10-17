<script lang="ts">
	import { enhance } from '$app/forms';
	import { resolve } from '$app/paths';
	import { keep, statusLabel } from '#lib/forms.ts';
	import Feedback from '#lib/components/Feedback.svelte';
	import Standings from '#lib/components/Standings.svelte';
	import type { PageProps } from './$types';

	let { data, form }: PageProps = $props();

	const t = $derived(data.tournament);
	const cat = $derived(Object.fromEntries(data.categories.map((c) => [c.slug, c])));
	const playerName = $derived(Object.fromEntries(data.players.map((p) => [p.id, p.name])));
	const awardName = $derived(
		Object.fromEntries(data.awards.map((a) => [a.slug, `${a.icon ?? ''} ${a.name}`]))
	);
	const roundAt = $derived(Object.fromEntries(data.rounds.map((r) => [r.number, r])));
	const exportedCat = $derived(
		Object.fromEntries((data.exported?.categories ?? []).map((c) => [c.order, c]))
	);
	const nextPosition = $derived(data.positions.find((p) => !roundAt[p.position]?.completed));
	const champion = $derived(data.exported?.champion ?? null);
	const participants = $derived(data.players.filter((p) => data.participantIds.includes(p.id)));
</script>

<div class="spread">
	<div class="stack head">
		<a href={resolve('/tournaments')} class="subtle">← Tournaments</a>
		<h1>{t.name}</h1>
		<p class="row">
			<span class="badge {t.status}">{statusLabel[t.status]}</span>
			<span class="subtle"
				>Challenger at <strong class="num">{t.challenger_threshold}</strong> pts</span
			>
			<span class="subtle">{participants.length} players</span>
		</p>
	</div>
	{#if nextPosition && !champion && t.status === 'ongoing'}
		<a
			class="btn primary big"
			href={resolve('/tournaments/[slug]/round/[number]', {
				slug: t.slug,
				number: String(nextPosition.position)
			})}
		>
			▶ Enter game {nextPosition.position}: {cat[nextPosition.category_slug]?.icon}
			{cat[nextPosition.category_slug]?.name}
		</a>
	{/if}
</div>

<Feedback {form} />

{#if champion && t.status !== 'completed'}
	<div class="panel champion">
		<p><strong>👑 {champion.name} won as a Challenger and is the Ultimate Gamer!</strong></p>
		<p class="muted">Mark the tournament as finished, then hand out the awards below.</p>
		<form method="POST" action="?/status" use:enhance>
			<input type="hidden" name="status" value="completed" />
			<button class="primary">Mark tournament completed</button>
		</form>
	</div>
{:else if t.status === 'upcoming'}
	{@const hasPlayers = participants.length >= 2}
	{@const hasCategories = data.positions.length >= 1}
	<section class="panel setup">
		<h2>Set up this tournament</h2>
		<ol class="setup-steps">
			<li class:done={hasPlayers}>
				<span class="mark">{hasPlayers ? '✓' : '1'}</span>
				<a href="#players">Tick the players taking part</a>
				<span class="subtle">{participants.length} selected</span>
			</li>
			<li class:done={hasCategories}>
				<span class="mark">{hasCategories ? '✓' : '2'}</span>
				<a href="#categories">Add the categories in the order they’ll be played</a>
				<span class="subtle">{data.positions.length} added · you can add more later</span>
			</li>
			<li>
				<span class="mark">3</span>
				<span>Start the tournament — it becomes the live tournament on the website</span>
			</li>
		</ol>
		<form method="POST" action="?/status" use:enhance>
			<input type="hidden" name="status" value="ongoing" />
			<button class="primary" disabled={!hasPlayers || !hasCategories}>▶ Start tournament</button>
		</form>
	</section>
{/if}

<div class="cols">
	<section class="panel">
		<h2>Standings</h2>
		<Standings standings={data.exported?.standings ?? []} />
	</section>

	<section class="panel" id="categories">
		<h2>Games &amp; category order</h2>
		<p class="hint">One game per category, played top to bottom.</p>
		{#if data.positions.length}
			<table>
				<tbody>
					{#each data.positions as p, i (p.position)}
						{@const r = roundAt[p.position]}
						{@const ex = exportedCat[p.position]}
						<tr>
							<td class="num">{p.position}</td>
							<td>{cat[p.category_slug]?.icon} {cat[p.category_slug]?.name ?? p.category_slug}</td>
							<td>
								{#if r?.completed}
									<span class="badge completed">Done</span>
									{#if ex?.winner}<span class="subtle">🏆 {ex.winner.name}</span>{/if}
								{:else if r}
									<span class="badge live">In progress</span>
								{:else}
									<span class="subtle">Upcoming</span>
								{/if}
								{#if ex?.game}<div class="subtle">{ex.game.name}</div>{/if}
							</td>
							<td class="actions">
								<a
									class="btn"
									href={resolve('/tournaments/[slug]/round/[number]', {
										slug: t.slug,
										number: String(p.position)
									})}>{r?.completed ? 'View' : 'Enter'}</a
								>
								{#if !r}
									<form method="POST" action="?/movePosition" use:enhance={keep}>
										<input type="hidden" name="position" value={p.position} />
										<button
											class="small"
											name="up"
											value="1"
											disabled={i === 0 || !!roundAt[p.position - 1]}
											aria-label="Move up">↑</button
										>
										<button
											class="small"
											disabled={i === data.positions.length - 1}
											aria-label="Move down">↓</button
										>
									</form>
									<form method="POST" action="?/removePosition" use:enhance={keep}>
										<input type="hidden" name="position" value={p.position} />
										<button class="small danger" aria-label="Remove">✕</button>
									</form>
								{/if}
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{:else}
			<p class="muted">
				No categories yet. Add them in the order they will be played (you can add more later).
			</p>
		{/if}
		<form method="POST" action="?/addPosition" use:enhance={keep} class="row">
			<label
				>Add category
				<select name="category">
					{#each data.categories as c (c.slug)}<option value={c.slug}>{c.icon} {c.name}</option
						>{/each}
				</select>
			</label>
			<button>Add</button>
		</form>
	</section>
</div>

<div class="cols">
	<section class="panel" id="players">
		<h2>Players in this tournament</h2>
		<form method="POST" action="?/participants" use:enhance={keep} class="stack">
			<div class="checks">
				{#each data.players as p (p.id)}
					<label class="inline"
						><input
							type="checkbox"
							name="player"
							value={p.id}
							checked={data.participantIds.includes(p.id)}
							onchange={(e) => e.currentTarget.form?.requestSubmit()}
						/>
						{p.name}</label
					>
				{/each}
			</div>
			{#if !data.players.length}<p class="muted">
					Add players on the <a href={resolve('/players')}>Players</a> page first.
				</p>{/if}
			<p class="hint">Ticking a box saves straight away.</p>
		</form>
	</section>

	<section class="panel">
		<h2>Settings</h2>
		<form method="POST" action="?/settings" use:enhance={keep} class="stack">
			<div class="row">
				<label>Name <input name="name" value={t.name} required /></label>
				<label>Edition <input name="edition" value={t.edition ?? ''} /></label>
				<label>Location <input name="location" value={t.location ?? ''} /></label>
			</div>
			<label
				>Web address
				<input name="slug" value={t.slug} required pattern="[a-z0-9]+(-[a-z0-9]+)*" />
			</label>
			<p class="hint">
				The tournament page is at <code>/tournaments/{t.slug}</code>. Changing it keeps the
				website’s own links working, but links people already shared will break.
			</p>
			<div class="row">
				<label>Start <input type="date" name="start_date" value={t.start_date ?? ''} /></label>
				<label>End <input type="date" name="end_date" value={t.end_date ?? ''} /></label>
				<label
					>Status
					<select name="status" value={t.status}>
						<option value="upcoming">Upcoming</option>
						<option value="ongoing">Live (ongoing)</option>
						<option value="completed">Completed</option>
					</select>
				</label>
			</div>
			<div class="row">
				<label
					>Challenger threshold <input
						type="number"
						name="challenger_threshold"
						min="1"
						value={t.challenger_threshold}
					/></label
				>
				<label
					>Double Down tokens <input
						type="number"
						name="double_down_tokens"
						min="0"
						value={t.double_down_tokens}
					/></label
				>
				<label
					>Nominate tokens <input
						type="number"
						name="nominate_tokens"
						min="0"
						value={t.nominate_tokens}
					/></label
				>
			</div>
			<div><button class="primary">Save settings</button></div>
		</form>
		<form method="POST" action="?/points" use:enhance={keep} class="row">
			<label
				>Points per placement <input
					name="points"
					value={data.points.join(', ')}
					size="20"
				/></label
			>
			<button>Save</button>
		</form>
	</section>
</div>

<div class="cols">
	<section class="panel">
		<h2>Awards</h2>
		<p class="hint">Award winners are your call — especially when there are ties.</p>
		{#if data.grants.length}
			<table>
				<tbody>
					{#each data.grants as g (g.id)}
						<tr>
							<td>{awardName[g.award_slug] ?? g.award_slug}</td>
							<td><strong>{playerName[g.player_id]}</strong></td>
							<td class="subtle">{g.detail ?? ''}</td>
							<td>
								<form method="POST" action="?/revoke" use:enhance={keep}>
									<input type="hidden" name="id" value={g.id} />
									<button class="small danger" aria-label="Remove award">✕</button>
								</form>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{/if}
		<form method="POST" action="?/grant" use:enhance class="row">
			<label
				>Award
				<select name="award">
					{#each data.awards as a (a.slug)}<option value={a.slug} title={a.description}
							>{a.icon} {a.name}</option
						>{/each}
				</select>
			</label>
			<label
				>Player
				<select name="player" required>
					{#each participants as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
				</select>
			</label>
			<label>Detail <input name="detail" placeholder="e.g. 4 Nemesis wins" /></label>
			<button>Grant</button>
		</form>
	</section>

	<section class="panel">
		<h2>Records</h2>
		{#if data.records.length}
			<table>
				<tbody>
					{#each data.records as r (r.position)}
						<tr>
							<td>{r.label}</td>
							<td class="num">{r.value}</td>
							<td>{r.player_id ? playerName[r.player_id] : ''}</td>
							<td>
								<form method="POST" action="?/removeRecord" use:enhance={keep}>
									<input type="hidden" name="position" value={r.position} />
									<button class="small danger" aria-label="Remove record">✕</button>
								</form>
							</td>
						</tr>
					{/each}
				</tbody>
			</table>
		{/if}
		<form method="POST" action="?/addRecord" use:enhance class="row">
			<label>Label <input name="label" required placeholder="Biggest comeback" /></label>
			<label>Value <input name="value" required size="8" /></label>
			<label
				>Player
				<select name="player">
					<option value="">—</option>
					{#each participants as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
				</select>
			</label>
			<button>Add</button>
		</form>
	</section>
</div>

<style>
	.head {
		gap: var(--sp-2);
	}

	.big {
		font-size: var(--fs-md);
		padding: 0.8rem 1.2rem;
	}

	.champion {
		border-color: var(--c-gold-line);
		background: var(--c-gold-soft);
	}

	.setup {
		border-color: var(--c-gold-line);
	}

	.setup-steps {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: var(--sp-2);
	}

	.setup-steps li {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--sp-3);
	}

	.setup-steps .mark {
		display: grid;
		place-items: center;
		width: 1.6rem;
		height: 1.6rem;
		border-radius: 50%;
		border: var(--border-strong);
		font-size: var(--fs-xs);
		font-weight: 800;
	}

	.setup-steps .done .mark {
		background: var(--c-live);
		border-color: var(--c-live);
		color: #04140b;
	}

	section[id] {
		scroll-margin-top: var(--sp-5);
	}

	.actions {
		display: flex;
		gap: var(--sp-1);
		justify-content: flex-end;
		align-items: center;
	}

	.actions form {
		display: flex;
		gap: 0.2rem;
	}

	.checks {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(9rem, 1fr));
		gap: var(--sp-2);
	}
</style>
