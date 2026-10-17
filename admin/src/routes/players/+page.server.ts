import * as repo from '#lib/server/repo.ts';
import { httpsUrl, int, mutate, optStr, str } from '#lib/server/forms.ts';
import type { Actions, PageServerLoad } from './$types';

export const load: PageServerLoad = () => ({ players: repo.listPlayers() });

export const actions: Actions = {
	create: async ({ request }) => {
		const form = await request.formData();
		return mutate('Player added.', () =>
			repo.createPlayer({
				name: str(form, 'name'),
				slug: optStr(form, 'slug') ?? undefined,
				initials: optStr(form, 'initials'),
				avatar_url: httpsUrl(optStr(form, 'avatar_url'))
			})
		);
	},
	update: async ({ request }) => {
		const form = await request.formData();
		return mutate('Player updated.', () =>
			repo.updatePlayer(int(form, 'id'), {
				name: str(form, 'name'),
				initials: optStr(form, 'initials'),
				avatar_url: httpsUrl(optStr(form, 'avatar_url'))
			})
		);
	},
	delete: async ({ request }) => {
		const form = await request.formData();
		return mutate('Player deleted.', () => repo.deletePlayer(int(form, 'id')));
	}
};
