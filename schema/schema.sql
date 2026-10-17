-- The Ultimate Gamer — authoritative tournament database (SQLite).
-- Raw facts only: standings, Challenger/champion status, tokens and statistics
-- are derived by scripts/export_json.py.

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
	avatar_url TEXT CHECK (avatar_url IS NULL OR avatar_url LIKE 'https://%')
);

CREATE TABLE IF NOT EXISTS categories (
	slug TEXT PRIMARY KEY,
	name TEXT NOT NULL,
	icon TEXT,
	description TEXT,
	sort_order INTEGER NOT NULL DEFAULT 0
);

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
	available INTEGER NOT NULL DEFAULT 1 CHECK (available IN (0, 1))
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
	nominate_tokens INTEGER NOT NULL DEFAULT 1 CHECK (nominate_tokens >= 0)
);

CREATE TABLE IF NOT EXISTS tournament_players (
	tournament_id INTEGER NOT NULL REFERENCES tournaments (id) ON DELETE CASCADE,
	player_id INTEGER NOT NULL REFERENCES players (id),
	PRIMARY KEY (tournament_id, player_id)
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

-- Authoritative award recipients, entered by the organisers (ties resolved by them).
CREATE TABLE IF NOT EXISTS award_grants (
	id INTEGER PRIMARY KEY,
	award_slug TEXT NOT NULL REFERENCES awards (slug),
	player_id INTEGER NOT NULL REFERENCES players (id),
	tournament_id INTEGER REFERENCES tournaments (id) ON DELETE CASCADE,
	detail TEXT
);

CREATE TABLE IF NOT EXISTS tournament_records (
	tournament_id INTEGER NOT NULL REFERENCES tournaments (id) ON DELETE CASCADE,
	position INTEGER NOT NULL,
	label TEXT NOT NULL,
	value TEXT NOT NULL,
	player_id INTEGER REFERENCES players (id),
	PRIMARY KEY (tournament_id, position)
);

CREATE INDEX IF NOT EXISTS idx_results_player ON results (player_id);
CREATE INDEX IF NOT EXISTS idx_grants_player ON award_grants (player_id);
CREATE INDEX IF NOT EXISTS idx_nominations_round ON nominations (tournament_id, round_number);
