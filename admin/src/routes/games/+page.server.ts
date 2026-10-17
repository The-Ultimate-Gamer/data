import * as repo from '#lib/server/repo.ts';
import { bool, int, mutate, optInt, optStr, str } from '#lib/server/forms.ts';
import type { Actions, PageServerLoad } from './$types';

export const load: PageServerLoad = () => ({
	games: repo.listGames(),
	categories: repo.listCategories()
});

function input(form: FormData): repo.GameInput {
	return {
		name: str(form, 'name'),
		slug: optStr(form, 'slug') ?? undefined,
		description: optStr(form, 'description'),
		min_players: optInt(form, 'min_players'),
		max_players: optInt(form, 'max_players'),
		available: bool(form, 'available'),
		categories: form.getAll('categories').map(String)
	};
}

export const actions: Actions = {
	save: async ({ request }) => {
		const form = await request.formData();
		const id = optInt(form, 'id');
		return mutate(id ? 'Game updated.' : 'Game added.', () => repo.saveGame(id, input(form)));
	},
	delete: async ({ request }) => {
		const form = await request.formData();
		return mutate('Game deleted.', () => repo.deleteGame(int(form, 'id')));
	}
};
