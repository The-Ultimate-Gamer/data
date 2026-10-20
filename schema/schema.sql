-- The Ultimate Gamer — authoritative tournament database (SQLite).
-- Raw facts only: standings, Challenger/champion status, tokens, awards and statistics
-- are derived by ug-admin (admin/src/rules.rs).

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS meta (
	key TEXT PRIMARY KEY,
	value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS players (
	id INTEGER PRIMARY KEY,
	slug TEXT NOT NULL UNIQUE,
	name TEXT NOT NULL,
	initials TEXT,
	avatar_url TEXT CHECK (avatar_url IS NULL OR avatar_url LIKE 'https://%'),
	-- 1 = fictional demo data (scripts/seed.py --add-demo); removed with --remove-demo.
	is_demo INTEGER NOT NULL DEFAULT 0 CHECK (is_demo IN (0, 1)),
	-- Discord account (numeric user id); required for real players, enforced by the admin.
	-- Username, display name and picture (avatars/<slug>.discord.png) are copied from Discord
	-- by `ug-admin discord` / the Players screen, so the website never calls Discord.
	discord_id TEXT CHECK (discord_id IS NULL OR (length(discord_id) BETWEEN 15 AND 21 AND discord_id NOT GLOB '*[^0-9]*')),
	discord_username TEXT,
	discord_display_name TEXT
);

CREATE TABLE IF NOT EXISTS categories (
	slug TEXT PRIMARY KEY,
	name TEXT NOT NULL,
	icon TEXT,
	description TEXT,
	sort_order INTEGER NOT NULL DEFAULT 0
);

-- Award definitions. Recipients are computed from the rules (admin/src/rules.rs), never entered.
CREATE TABLE IF NOT EXISTS awards (
	slug TEXT PRIMARY KEY,
	name TEXT NOT NULL,
	icon TEXT,
	description TEXT,
	scope TEXT NOT NULL DEFAULT 'tournament' CHECK (scope IN ('tournament', 'career')),
	sort_order INTEGER NOT NULL DEFAULT 0
);

-- The game library: played, nominated and never-played games.
CREATE TABLE IF NOT EXISTS games (
	id INTEGER PRIMARY KEY,
	slug TEXT NOT NULL UNIQUE,
	name TEXT NOT NULL,
	description TEXT,
	min_players INTEGER CHECK (min_players IS NULL OR min_players >= 1),
	max_players INTEGER CHECK (max_players IS NULL OR max_players >= 1),
	available INTEGER NOT NULL DEFAULT 1 CHECK (available IN (0, 1)),
	is_demo INTEGER NOT NULL DEFAULT 0 CHECK (is_demo IN (0, 1))
);

CREATE TABLE IF NOT EXISTS game_categories (
	game_id INTEGER NOT NULL REFERENCES games (id) ON DELETE CASCADE,
	category_slug TEXT NOT NULL REFERENCES categories (slug),
	PRIMARY KEY (game_id, category_slug)
);

CREATE TABLE IF NOT EXISTS tournaments (
	id INTEGER PRIMARY KEY,
	slug TEXT NOT NULL UNIQUE,
	name TEXT NOT NULL,
	edition TEXT,
	start_date TEXT,
	end_date TEXT,
	location TEXT,
	status TEXT NOT NULL DEFAULT 'upcoming' CHECK (status IN ('upcoming', 'ongoing', 'completed')),
	challenger_threshold INTEGER NOT NULL CHECK (challenger_threshold > 0),
	double_down_tokens INTEGER NOT NULL DEFAULT 1 CHECK (double_down_tokens >= 0),
	nominate_tokens INTEGER NOT NULL DEFAULT 1 CHECK (nominate_tokens >= 0),
	is_demo INTEGER NOT NULL DEFAULT 0 CHECK (is_demo IN (0, 1))
);

CREATE TABLE IF NOT EXISTS tournament_players (
	tournament_id INTEGER NOT NULL REFERENCES tournaments (id) ON DELETE CASCADE,
	player_id INTEGER NOT NULL REFERENCES players (id),
	PRIMARY KEY (tournament_id, player_id)
);

-- Default base points per placement, used to prefill results (results still store the actual value).
CREATE TABLE IF NOT EXISTS tournament_points (
	tournament_id INTEGER NOT NULL REFERENCES tournaments (id) ON DELETE CASCADE,
	placement INTEGER NOT NULL CHECK (placement >= 1),
	points INTEGER NOT NULL CHECK (points >= 0),
	PRIMARY KEY (tournament_id, placement)
);

-- Category order for a tournament. Rows may be added as categories are announced.
CREATE TABLE IF NOT EXISTS tournament_categories (
	tournament_id INTEGER NOT NULL REFERENCES tournaments (id) ON DELETE CASCADE,
	position INTEGER NOT NULL CHECK (position >= 1),
	category_slug TEXT NOT NULL REFERENCES categories (slug),
	PRIMARY KEY (tournament_id, position)
);

-- One round per category position. game_id is NULL until the game is drawn;
-- completed = 1 once results are final.
CREATE TABLE IF NOT EXISTS rounds (
	tournament_id INTEGER NOT NULL,
	number INTEGER NOT NULL,
	game_id INTEGER REFERENCES games (id),
	played_at TEXT,
	completed INTEGER NOT NULL DEFAULT 0 CHECK (completed IN (0, 1)),
	PRIMARY KEY (tournament_id, number),
	FOREIGN KEY (tournament_id, number)
		REFERENCES tournament_categories (tournament_id, position) ON DELETE CASCADE
);

-- Per-player result of a round. Final points are derived:
-- (base_points + 1 if the assigned Nemesis was beaten) × 2 if Double Down was used.
CREATE TABLE IF NOT EXISTS results (
	tournament_id INTEGER NOT NULL,
	round_number INTEGER NOT NULL,
	player_id INTEGER NOT NULL REFERENCES players (id),
	placement INTEGER NOT NULL CHECK (placement >= 1),
	base_points INTEGER NOT NULL CHECK (base_points >= 0),
	double_down INTEGER NOT NULL DEFAULT 0 CHECK (double_down IN (0, 1)),
	nemesis_player_id INTEGER REFERENCES players (id),
	PRIMARY KEY (tournament_id, round_number, player_id),
	FOREIGN KEY (tournament_id, round_number)
		REFERENCES rounds (tournament_id, number) ON DELETE CASCADE,
	CHECK (nemesis_player_id IS NULL OR nemesis_player_id <> player_id)
);

-- Nemesis pairs announced before a round. Nemeses are public: these are shown on the website
-- while the round is upcoming. Several players may share a Nemesis and pairs need not be mutual.
-- results.nemesis_player_id stays the record of the Nemesis a player actually had.
CREATE TABLE IF NOT EXISTS nemesis_assignments (
	tournament_id INTEGER NOT NULL,
	round_number INTEGER NOT NULL,
	player_id INTEGER NOT NULL REFERENCES players (id),
	nemesis_player_id INTEGER NOT NULL REFERENCES players (id),
	PRIMARY KEY (tournament_id, round_number, player_id),
	FOREIGN KEY (tournament_id, round_number)
		REFERENCES rounds (tournament_id, number) ON DELETE CASCADE,
	CHECK (nemesis_player_id <> player_id)
);

-- Nominations for the game of a round (recorded, not performed, by this project).
CREATE TABLE IF NOT EXISTS nominations (
	id INTEGER PRIMARY KEY,
	tournament_id INTEGER NOT NULL,
	round_number INTEGER NOT NULL,
	game_id INTEGER NOT NULL REFERENCES games (id),
	player_id INTEGER REFERENCES players (id),
	source TEXT NOT NULL CHECK (source IN ('winner', 'vote', 'token')),
	entries INTEGER NOT NULL DEFAULT 1 CHECK (entries >= 1),
	FOREIGN KEY (tournament_id, round_number)
		REFERENCES rounds (tournament_id, number) ON DELETE CASCADE,
	CHECK (source = 'vote' OR player_id IS NOT NULL)
);

CREATE INDEX IF NOT EXISTS idx_results_player ON results (player_id);
CREATE INDEX IF NOT EXISTS idx_nominations_round ON nominations (tournament_id, round_number);
