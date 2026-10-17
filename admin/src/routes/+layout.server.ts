import { config } from '#lib/server/config.ts';
import { status } from '#lib/server/git.ts';
import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = async () => {
	const git = config.canPublish ? await status() : null;
	return {
		dbPath: config.db,
		isAlternateDb: config.isAlternateDb,
		canPublish: config.canPublish,
		pendingChanges: git ? git.changes.length + git.ahead : 0
	};
};
