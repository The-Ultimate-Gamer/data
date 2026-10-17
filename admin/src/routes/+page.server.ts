import { config } from '#lib/server/config.ts';
import { isUnpublished, status } from '#lib/server/git.ts';
import * as repo from '#lib/server/repo.ts';
import type { PageServerLoad } from './$types';

export interface Step {
	title: string;
	detail: string;
	done: boolean;
	href: string;
	action: string;
}

export const load: PageServerLoad = async () => {
	const tournaments = repo.listTournaments();
	const realTournaments = tournaments.filter((t) => !t.is_demo);
	const players = repo.listPlayers();
	const games = repo.listGames();

	const withNext = (t: (typeof tournaments)[number]) => {
		const done = new Set(
			repo
				.rounds(t.id)
				.filter((r) => r.completed)
				.map((r) => r.number)
		);
		const next = repo.positions(t.id).find((p) => !done.has(p.position));
		return { ...t, nextRound: next?.position ?? null };
	};
	const active = tournaments.filter((t) => t.status !== 'completed').map(withNext);

	// The tournament the checklist is about: the newest real one that isn't finished, else the newest real one.
	const focus = realTournaments.find((t) => t.status !== 'completed') ?? realTournaments[0] ?? null;
	const focusNext = focus ? withNext(focus) : null;
	const git = config.canPublish ? await status() : null;
	const realPlayers = players.filter((p) => !p.is_demo).length;
	const realGames = games.filter((g) => !g.is_demo).length;

	const steps: Step[] = [
		{
			title: 'Add the players',
			detail: `${realPlayers} player${realPlayers === 1 ? '' : 's'} added`,
			done: realPlayers >= 2,
			href: '/players',
			action: 'Add players'
		},
		{
			title: 'Build the game library',
			detail: `${realGames} game${realGames === 1 ? '' : 's'}, each tagged with categories`,
			done: realGames > 0,
			href: '/games',
			action: 'Add games'
		},
		{
			title: 'Create the tournament',
			detail: focus ? focus.name : 'Name, date, Challenger threshold and points per placement',
			done: !!focus,
			href: '/tournaments',
			action: 'Create tournament'
		},
		{
			title: 'Choose who plays and the category order',
			detail: focus
				? `${focus.players} players · ${focus.positions} categories`
				: 'On the tournament page',
			done: !!focus && focus.players >= 2 && focus.positions >= 1,
			href: focus ? `/tournaments/${focus.slug}` : '/tournaments',
			action: 'Set up tournament'
		},
		{
			title: 'Start the tournament',
			detail: 'Makes it the live tournament on the website’s homepage',
			done: !!focus && focus.status !== 'upcoming',
			href: focus ? `/tournaments/${focus.slug}` : '/tournaments',
			action: 'Go live'
		},
		{
			title: 'Record each game',
			detail: focus
				? `${focus.rounds_done} of ${focus.positions} played`
				: 'Tap players in finishing order, then Finish game',
			done: !!focus && (focus.status === 'completed' || focus.rounds_done > 0),
			href:
				focusNext?.nextRound != null
					? `/tournaments/${focusNext.slug}/round/${focusNext.nextRound}`
					: focus
						? `/tournaments/${focus.slug}`
						: '/tournaments',
			action: focusNext?.nextRound != null ? `Enter game ${focusNext.nextRound}` : 'Open tournament'
		},
		{
			title: 'Publish',
			detail: 'Send your changes to the website',
			done: !!git && !isUnpublished(git),
			href: '/publish',
			action: 'Publish'
		}
	];

	return { steps, active, demo: repo.demoPresent(), hasReal: realTournaments.length > 0 };
};
