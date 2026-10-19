/** Data access for the admin UI. Stores facts only; derived values come from the exported JSON. */
import { all, get, run, transaction } from './db.ts';

export interface Player {
	id: number;
	slug: string;
	name: string;
	initials: string | null;
	avatar_url: string | null;
	is_demo: number;
}

export interface Game {
	id: number;
	slug: string;
	name: string;
	description: string | null;
	min_players: number | null;
	max_players: number | null;
	available: number;
	is_demo: number;
	categories: string[];
}

export interface Category {
	slug: string;
	name: string;
	icon: string | null;
}

export interface Award {
	slug: string;
	name: string;
	icon: string | null;
	scope: string;
	description: string | null;
}

export interface Tournament {
	id: number;
	slug: string;
	name: string;
	edition: string | null;
	start_date: string | null;
	end_date: string | null;
	location: string | null;
	status: 'upcoming' | 'ongoing' | 'completed';
	challenger_threshold: number;
	double_down_tokens: number;
	nominate_tokens: number;
	is_demo: number;
}

export interface Round {
	tournament_id: number;
	number: number;
	game_id: number | null;
	played_at: string | null;
	completed: number;
}

export interface ResultRow {
	player_id: number;
	placement: number;
	base_points: number;
	double_down: number;
	nemesis_player_id: number | null;
}

export interface Nomination {
	id: number;
	game_id: number;
	player_id: number | null;
	source: 'winner' | 'vote' | 'token';
	entries: number;
}

export class UserError extends Error {}

const SLUG_RE = /^[a-z0-9]+(?:-[a-z0-9]+)*$/;

export function slugify(text: string): string {
	return text
		.normalize('NFKD')
		.replace(/[\u0300-\u036f]/g, '')
		.toLowerCase()
		.replace(/[^a-z0-9]+/g, '-')
		.replace(/^-+|-+$/g, '');
}

export function checkSlug(slug: string): string {
	if (!SLUG_RE.test(slug))
		throw new UserError(`"${slug}" is not a valid slug (lowercase letters, digits and dashes).`);
	return slug;
}

const NOUN = { players: 'player', games: 'game', tournaments: 'tournament' } as const;

/** Change a URL slug. Website links follow automatically; old bookmarks/shared links stop working. */
export function changeSlug(table: keyof typeof NOUN, id: number, slug: string) {
	checkSlug(slug);
	if (get(`SELECT 1 FROM ${table} WHERE slug = ? AND id <> ?`, slug, id)) {
		throw new UserError(`Another ${NOUN[table]} already uses the slug "${slug}".`);
	}
	run(`UPDATE ${table} SET slug = ? WHERE id = ?`, slug, id);
}

// ------------------------------------------------------------------- demo

export function demoPresent(): boolean {
	return !!get(
		`SELECT 1 WHERE EXISTS (SELECT 1 FROM players WHERE is_demo = 1)
		   OR EXISTS (SELECT 1 FROM games WHERE is_demo = 1)
		   OR EXISTS (SELECT 1 FROM tournaments WHERE is_demo = 1)`
	);
}

// ---------------------------------------------------------------- reference

export const listCategories = () =>
	all<Category>('SELECT slug, name, icon FROM categories ORDER BY sort_order, slug');

export const listAwards = () =>
	all<Award>('SELECT slug, name, icon, scope, description FROM awards ORDER BY sort_order, slug');

// ------------------------------------------------------------------ players

export const listPlayers = () => all<Player>('SELECT * FROM players ORDER BY name COLLATE NOCASE');

export function createPlayer(p: {
	name: string;
	slug?: string;
	initials?: string | null;
	avatar_url?: string | null;
}) {
	const slug = checkSlug(p.slug || slugify(p.name));
	if (get('SELECT 1 FROM players WHERE slug = ?', slug))
		throw new UserError(`A player with slug "${slug}" already exists.`);
	run(
		'INSERT INTO players (slug, name, initials, avatar_url) VALUES (?, ?, ?, ?)',
		slug,
		p.name,
		p.initials || null,
		p.avatar_url || null
	);
	return slug;
}

export function updatePlayer(
	id: number,
	p: { name: string; slug?: string | null; initials?: string | null; avatar_url?: string | null }
) {
	transaction(() => {
		if (p.slug) changeSlug('players', id, p.slug);
		run(
			'UPDATE players SET name = ?, initials = ?, avatar_url = ? WHERE id = ?',
			p.name,
			p.initials || null,
			p.avatar_url || null,
			id
		);
	});
}

export function deletePlayer(id: number) {
	const used = get<{ n: number }>(
		`SELECT (SELECT COUNT(*) FROM tournament_players WHERE player_id = ?) +
		        (SELECT COUNT(*) FROM award_grants WHERE player_id = ?) AS n`,
		id,
		id
	);
	if (used && used.n > 0)
		throw new UserError('This player has tournament history and cannot be deleted.');
	run('DELETE FROM players WHERE id = ?', id);
}

// -------------------------------------------------------------------- games

export function listGames(): Game[] {
	const games = all<Omit<Game, 'categories'>>('SELECT * FROM games ORDER BY name COLLATE NOCASE');
	const cats = all<{ game_id: number; category_slug: string }>(
		'SELECT game_id, category_slug FROM game_categories'
	);
	return games.map((g) => ({
		...g,
		categories: cats.filter((c) => c.game_id === g.id).map((c) => c.category_slug)
	}));
}

export interface GameInput {
	name: string;
	slug?: string;
	description?: string | null;
	min_players?: number | null;
	max_players?: number | null;
	available: boolean;
	categories: string[];
}

export function saveGame(id: number | null, g: GameInput) {
	return transaction(() => {
		let gameId = id;
		if (gameId === null) {
			const slug = checkSlug(g.slug || slugify(g.name));
			if (get('SELECT 1 FROM games WHERE slug = ?', slug))
				throw new UserError(`A game with slug "${slug}" already exists.`);
			gameId = Number(
				run(
					'INSERT INTO games (slug, name, description, min_players, max_players, available) VALUES (?, ?, ?, ?, ?, ?)',
					slug,
					g.name,
					g.description || null,
					g.min_players ?? null,
					g.max_players ?? null,
					g.available ? 1 : 0
				).lastInsertRowid
			);
		} else {
			if (g.slug) changeSlug('games', gameId, g.slug);
			run(
				'UPDATE games SET name = ?, description = ?, min_players = ?, max_players = ?, available = ? WHERE id = ?',
				g.name,
				g.description || null,
				g.min_players ?? null,
				g.max_players ?? null,
				g.available ? 1 : 0,
				gameId
			);
			run('DELETE FROM game_categories WHERE game_id = ?', gameId);
		}
		for (const c of g.categories)
			run('INSERT INTO game_categories (game_id, category_slug) VALUES (?, ?)', gameId, c);
		return gameId;
	});
}

// -------------------------------------------------------------- tournaments

export function deleteGame(id: number) {
	const used = get<{ n: number }>(
		`SELECT (SELECT COUNT(*) FROM rounds WHERE game_id = ?) + (SELECT COUNT(*) FROM nominations WHERE game_id = ?) AS n`,
		id,
		id
	);
	if (used && used.n > 0)
		throw new UserError('This game has been played or nominated; mark it unavailable instead.');
	transaction(() => {
		run('DELETE FROM game_categories WHERE game_id = ?', id);
		run('DELETE FROM games WHERE id = ?', id);
	});
}

export const listTournaments = () =>
	all<Tournament & { players: number; rounds_done: number; positions: number }>(
		`SELECT t.*,
		        (SELECT COUNT(*) FROM tournament_players p WHERE p.tournament_id = t.id) AS players,
		        (SELECT COUNT(*) FROM rounds r WHERE r.tournament_id = t.id AND r.completed = 1) AS rounds_done,
		        (SELECT COUNT(*) FROM tournament_categories c WHERE c.tournament_id = t.id) AS positions
		 FROM tournaments t ORDER BY t.start_date IS NULL, t.start_date DESC, t.id DESC`
	);

export function getTournament(slug: string) {
	return get<Tournament>('SELECT * FROM tournaments WHERE slug = ?', slug);
}

export type TournamentInput = Omit<Tournament, 'id' | 'slug' | 'is_demo'>;

export function createTournament(
	t: TournamentInput & { slug?: string; points: number[]; copyPlayersFrom?: number | null }
) {
	return transaction(() => {
		const slug = checkSlug(t.slug || slugify(t.name));
		if (get('SELECT 1 FROM tournaments WHERE slug = ?', slug))
			throw new UserError(`A tournament with slug "${slug}" already exists.`);
		const id = Number(
			run(
				`INSERT INTO tournaments (slug, name, edition, start_date, end_date, location, status, challenger_threshold,
				 double_down_tokens, nominate_tokens) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
				slug,
				t.name,
				t.edition,
				t.start_date,
				t.end_date,
				t.location,
				t.status,
				t.challenger_threshold,
				t.double_down_tokens,
				t.nominate_tokens
			).lastInsertRowid
		);
		setPoints(id, t.points);
		if (t.copyPlayersFrom) {
			run(
				'INSERT INTO tournament_players SELECT ?, player_id FROM tournament_players WHERE tournament_id = ?',
				id,
				t.copyPlayersFrom
			);
		}
		return slug;
	});
}

export function updateTournament(id: number, t: TournamentInput & { slug?: string | null }) {
	if (t.slug) changeSlug('tournaments', id, t.slug);
	run(
		`UPDATE tournaments SET name = ?, edition = ?, start_date = ?, end_date = ?, location = ?, status = ?,
		 challenger_threshold = ?, double_down_tokens = ?, nominate_tokens = ? WHERE id = ?`,
		t.name,
		t.edition,
		t.start_date,
		t.end_date,
		t.location,
		t.status,
		t.challenger_threshold,
		t.double_down_tokens,
		t.nominate_tokens,
		id
	);
}

export const participants = (tid: number) =>
	all<Player>(
		`SELECT p.* FROM players p JOIN tournament_players tp ON tp.player_id = p.id
		 WHERE tp.tournament_id = ? ORDER BY p.name COLLATE NOCASE`,
		tid
	);

export function setParticipants(tid: number, playerIds: number[]) {
	transaction(() => {
		const current = participants(tid).map((p) => p.id);
		for (const pid of current.filter((id) => !playerIds.includes(id))) {
			const used = get<{ n: number }>(
				`SELECT (SELECT COUNT(*) FROM results WHERE tournament_id = ? AND player_id = ?) +
				        (SELECT COUNT(*) FROM award_grants WHERE tournament_id = ? AND player_id = ?) AS n`,
				tid,
				pid,
				tid,
				pid
			);
			if (used && used.n > 0) {
				const name = get<{ name: string }>('SELECT name FROM players WHERE id = ?', pid)?.name;
				throw new UserError(
					`${name} already has results or awards in this tournament and can't be removed.`
				);
			}
			run('DELETE FROM tournament_players WHERE tournament_id = ? AND player_id = ?', tid, pid);
		}
		for (const pid of playerIds.filter((id) => !current.includes(id))) {
			run('INSERT INTO tournament_players (tournament_id, player_id) VALUES (?, ?)', tid, pid);
		}
	});
}

export const points = (tid: number) =>
	all<{ placement: number; points: number }>(
		'SELECT placement, points FROM tournament_points WHERE tournament_id = ? ORDER BY placement',
		tid
	).map((r) => r.points);

export function setPoints(tid: number, values: number[]) {
	if (values.some((v) => !Number.isInteger(v) || v < 0))
		throw new UserError('Points must be whole numbers ≥ 0.');
	transaction(() => {
		run('DELETE FROM tournament_points WHERE tournament_id = ?', tid);
		values.forEach((v, i) => run('INSERT INTO tournament_points VALUES (?, ?, ?)', tid, i + 1, v));
	});
}

// ------------------------------------------------------- category positions

export const positions = (tid: number) =>
	all<{ position: number; category_slug: string }>(
		'SELECT position, category_slug FROM tournament_categories WHERE tournament_id = ? ORDER BY position',
		tid
	);

export function addPosition(tid: number, category: string) {
	const next =
		get<{ n: number }>(
			'SELECT COALESCE(MAX(position), 0) + 1 AS n FROM tournament_categories WHERE tournament_id = ?',
			tid
		)?.n ?? 1;
	run('INSERT INTO tournament_categories VALUES (?, ?, ?)', tid, next, category);
}

function assertUnplayed(tid: number, position: number) {
	if (get('SELECT 1 FROM rounds WHERE tournament_id = ? AND number = ?', tid, position)) {
		throw new UserError(`Round ${position} has already started; its category can't be changed.`);
	}
}

export function removePosition(tid: number, position: number) {
	transaction(() => {
		assertUnplayed(tid, position);
		run(
			'DELETE FROM tournament_categories WHERE tournament_id = ? AND position = ?',
			tid,
			position
		);
		// Close the gap so positions stay 1..n.
		const later = positions(tid).filter((p) => p.position > position);
		for (const p of later) {
			assertUnplayed(tid, p.position);
			run(
				'UPDATE tournament_categories SET position = ? WHERE tournament_id = ? AND position = ?',
				p.position - 1,
				tid,
				p.position
			);
		}
	});
}

export function swapPositions(tid: number, a: number, b: number) {
	transaction(() => {
		assertUnplayed(tid, a);
		assertUnplayed(tid, b);
		const list = positions(tid);
		const ca = list.find((p) => p.position === a)?.category_slug;
		const cb = list.find((p) => p.position === b)?.category_slug;
		if (!ca || !cb) return;
		run(
			'UPDATE tournament_categories SET category_slug = ? WHERE tournament_id = ? AND position = ?',
			cb,
			tid,
			a
		);
		run(
			'UPDATE tournament_categories SET category_slug = ? WHERE tournament_id = ? AND position = ?',
			ca,
			tid,
			b
		);
	});
}

// ------------------------------------------------------------------- rounds

export const rounds = (tid: number) =>
	all<Round>('SELECT * FROM rounds WHERE tournament_id = ? ORDER BY number', tid);

export const round = (tid: number, number: number) =>
	get<Round>('SELECT * FROM rounds WHERE tournament_id = ? AND number = ?', tid, number);

function ensureRound(tid: number, number: number) {
	if (
		!get(
			'SELECT 1 FROM tournament_categories WHERE tournament_id = ? AND position = ?',
			tid,
			number
		)
	) {
		throw new UserError(`Round ${number} has no category yet. Add it to the category order first.`);
	}
	run(
		'INSERT OR IGNORE INTO rounds (tournament_id, number, completed) VALUES (?, ?, 0)',
		tid,
		number
	);
}

export const results = (tid: number, number: number) =>
	all<ResultRow>(
		'SELECT player_id, placement, base_points, double_down, nemesis_player_id FROM results WHERE tournament_id = ? AND round_number = ?',
		tid,
		number
	);

export const nominations = (tid: number, number: number) =>
	all<Nomination>(
		'SELECT id, game_id, player_id, source, entries FROM nominations WHERE tournament_id = ? AND round_number = ? ORDER BY id',
		tid,
		number
	);

export function addNomination(tid: number, number: number, n: Omit<Nomination, 'id'>) {
	transaction(() => {
		ensureRound(tid, number);
		if (n.source !== 'vote' && !n.player_id)
			throw new UserError('Winner and token nominations need a player.');
		run(
			'INSERT INTO nominations (tournament_id, round_number, game_id, player_id, source, entries) VALUES (?, ?, ?, ?, ?, ?)',
			tid,
			number,
			n.game_id,
			n.player_id,
			n.source,
			n.entries
		);
	});
}

export function removeNomination(tid: number, id: number) {
	run('DELETE FROM nominations WHERE id = ? AND tournament_id = ?', id, tid);
}

export function setRoundGame(tid: number, number: number, gameId: number | null) {
	transaction(() => {
		ensureRound(tid, number);
		run(
			'UPDATE rounds SET game_id = ? WHERE tournament_id = ? AND number = ?',
			gameId,
			tid,
			number
		);
	});
}

export interface ResultInput {
	player_id: number;
	placement: number;
	base_points: number;
	double_down: boolean;
	nemesis_player_id: number | null;
}

export function saveResults(tid: number, number: number, rows: ResultInput[], finish: boolean) {
	if (finish) {
		if (rows.length < 2) throw new UserError('Enter results for at least two players.');
		const winners = rows.filter((r) => r.placement === 1).length;
		if (winners !== 1)
			throw new UserError(
				`A finished game needs exactly one winner (placement 1); found ${winners}.`
			);
		const current = round(tid, number);
		if (!current?.game_id)
			throw new UserError('Choose which game was played before finishing the round.');
	}
	for (const r of rows) {
		if (!Number.isInteger(r.placement) || r.placement < 1)
			throw new UserError('Every placement must be 1 or higher.');
		if (!Number.isInteger(r.base_points) || r.base_points < 0)
			throw new UserError('Base points must be 0 or higher.');
		if (r.nemesis_player_id === r.player_id)
			throw new UserError('A player cannot be their own Nemesis.');
	}
	transaction(() => {
		ensureRound(tid, number);
		run('DELETE FROM results WHERE tournament_id = ? AND round_number = ?', tid, number);
		for (const r of rows) {
			run(
				'INSERT INTO results VALUES (?, ?, ?, ?, ?, ?, ?)',
				tid,
				number,
				r.player_id,
				r.placement,
				r.base_points,
				r.double_down ? 1 : 0,
				r.nemesis_player_id
			);
		}
		if (finish) {
			run(
				`UPDATE rounds SET completed = 1, played_at = COALESCE(played_at, ?) WHERE tournament_id = ? AND number = ?`,
				new Date().toISOString().replace(/\.\d{3}Z$/, 'Z'),
				tid,
				number
			);
		}
	});
}

/** Nemesis pairs announced before a round: player id -> Nemesis id. */
export function nemesisAssignments(tid: number, number: number): Record<number, number> {
	const rows = all<{ player_id: number; nemesis_player_id: number }>(
		'SELECT player_id, nemesis_player_id FROM nemesis_assignments WHERE tournament_id = ? AND round_number = ?',
		tid,
		number
	);
	return Object.fromEntries(rows.map((r) => [r.player_id, r.nemesis_player_id]));
}

/** Replace the announced Nemesis pairs of a round. Players without a Nemesis are left out. */
export function setNemesisAssignments(
	tid: number,
	number: number,
	pairs: { player_id: number; nemesis_player_id: number | null }[]
) {
	for (const p of pairs) {
		if (p.nemesis_player_id === p.player_id)
			throw new UserError('A player cannot be their own Nemesis.');
	}
	transaction(() => {
		ensureRound(tid, number);
		run(
			'DELETE FROM nemesis_assignments WHERE tournament_id = ? AND round_number = ?',
			tid,
			number
		);
		for (const p of pairs) {
			if (p.nemesis_player_id === null) continue;
			run(
				'INSERT INTO nemesis_assignments (tournament_id, round_number, player_id, nemesis_player_id) VALUES (?, ?, ?, ?)',
				tid,
				number,
				p.player_id,
				p.nemesis_player_id
			);
		}
	});
}

export function reopenRound(tid: number, number: number) {
	const later = get(
		'SELECT 1 FROM rounds WHERE tournament_id = ? AND number > ? AND completed = 1',
		tid,
		number
	);
	if (later) throw new UserError('Only the most recent finished round can be reopened.');
	run('UPDATE rounds SET completed = 0 WHERE tournament_id = ? AND number = ?', tid, number);
}

export function deleteRound(tid: number, number: number) {
	const r = round(tid, number);
	if (r?.completed) throw new UserError('Reopen the round before deleting it.');
	run('DELETE FROM rounds WHERE tournament_id = ? AND number = ?', tid, number);
}

/** Double Down / Nominate token use per player in this tournament, excluding one round's results. */
export function tokenUsage(tid: number, excludeRound?: number) {
	const dd = all<{ player_id: number; n: number }>(
		`SELECT player_id, SUM(double_down) AS n FROM results WHERE tournament_id = ? AND round_number <> ? GROUP BY player_id`,
		tid,
		excludeRound ?? -1
	);
	const nom = all<{ player_id: number; n: number }>(
		`SELECT player_id, COUNT(*) AS n FROM nominations WHERE tournament_id = ? AND source = 'token' GROUP BY player_id`,
		tid
	);
	return {
		doubleDown: Object.fromEntries(dd.map((r) => [r.player_id, r.n])) as Record<number, number>,
		nominate: Object.fromEntries(nom.map((r) => [r.player_id, r.n])) as Record<number, number>
	};
}

// ------------------------------------------------------------ awards/records

export const grants = (tid: number) =>
	all<{ id: number; award_slug: string; player_id: number; detail: string | null }>(
		'SELECT id, award_slug, player_id, detail FROM award_grants WHERE tournament_id = ? ORDER BY id',
		tid
	);

export function grantAward(tid: number, award: string, playerId: number, detail: string | null) {
	run(
		'INSERT INTO award_grants (award_slug, player_id, tournament_id, detail) VALUES (?, ?, ?, ?)',
		award,
		playerId,
		tid,
		detail
	);
}

export function revokeAward(tid: number, id: number) {
	run('DELETE FROM award_grants WHERE id = ? AND tournament_id = ?', id, tid);
}

export const records = (tid: number) =>
	all<{ position: number; label: string; value: string; player_id: number | null }>(
		'SELECT position, label, value, player_id FROM tournament_records WHERE tournament_id = ? ORDER BY position',
		tid
	);

export function addRecord(tid: number, label: string, value: string, playerId: number | null) {
	const next =
		get<{ n: number }>(
			'SELECT COALESCE(MAX(position), 0) + 1 AS n FROM tournament_records WHERE tournament_id = ?',
			tid
		)?.n ?? 1;
	run('INSERT INTO tournament_records VALUES (?, ?, ?, ?, ?)', tid, next, label, value, playerId);
}

export function removeRecord(tid: number, position: number) {
	run('DELETE FROM tournament_records WHERE tournament_id = ? AND position = ?', tid, position);
}
