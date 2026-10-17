import path from 'node:path';

// The admin app lives in <data repo>/admin and is started from that folder.
const repo = path.resolve(process.env.UG_REPO ?? path.join(process.cwd(), '..'));
const defaultDb = path.join(repo, 'database', 'ultimate-gamer.sqlite');
const db = path.resolve(repo, process.env.UG_DB ?? defaultDb);

export const config = {
	repo,
	db,
	out: path.resolve(repo, process.env.UG_OUT ?? 'data'),
	python: process.env.UG_PYTHON ?? 'python3',
	siteUrl: process.env.UG_SITE_URL ?? 'https://the-ultimate-gamer.github.io/',
	/** True when working on a database other than the authoritative one (e.g. the demo). */
	isAlternateDb: db !== defaultDb,
	/** Git commit/push is only offered for the authoritative database. */
	canPublish: db === defaultDb
};
