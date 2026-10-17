import * as repo from '#lib/server/repo.ts';
import { validate } from '#lib/server/scripts.ts';
import type { PageServerLoad } from './$types';

export const load: PageServerLoad = async () => {
	const tournaments = repo.listTournaments();
	const live = tournaments
		.filter((t) => t.status !== 'completed')
		.map((t) => {
			const done = new Set(
				repo
					.rounds(t.id)
					.filter((r) => r.completed)
					.map((r) => r.number)
			);
			const next = repo.positions(t.id).find((p) => !done.has(p.position));
			return { ...t, nextRound: next?.position ?? null };
		});
	const validation = await validate();
	return {
		live,
		counts: {
			tournaments: tournaments.length,
			players: repo.listPlayers().length,
			games: repo.listGames().length
		},
		errors: validation.errors.length,
		warnings: validation.warnings.length
	};
};
