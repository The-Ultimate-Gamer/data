import { config } from '#lib/server/config.ts';
import { isUnpublished, status } from '#lib/server/git.ts';
import { demoPresent } from '#lib/server/repo.ts';
import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = async () => {
	const git = config.canPublish ? await status() : null;
	return {
		dbPath: config.db,
		isAlternateDb: config.isAlternateDb,
		canPublish: config.canPublish,
		siteUrl: config.siteUrl,
		unpublished: git ? isUnpublished(git) : false,
		demo: demoPresent()
	};
};
