/** Minimal types for the generated JSON the admin reads back (see the website's docs/data-contract.md). */

export interface Ref {
	slug: string;
	name: string;
}

export interface ExportedStanding {
	rank: number;
	player: Ref;
	points: number;
	isChallenger: boolean;
	challengerSinceGame: number | null;
	isChampion: boolean;
	tokens: { doubleDown: number; nominate: number };
	gamesWon: number;
}

export interface ExportedCategory {
	order: number;
	category: string;
	status: 'completed' | 'active' | 'upcoming';
	game: Ref | null;
	winner: Ref | null;
}

export interface ExportedGame {
	number: number;
	winner: Ref | null;
	newChallengers: Ref[];
	decidedTournament: boolean;
	results: {
		player: Ref;
		placement: number;
		points: number;
		defeatedNemesis: boolean | null;
		doubleDown: boolean;
	}[];
}

export interface ExportedTournament {
	slug: string;
	name: string;
	status: string;
	champion: Ref | null;
	standings: ExportedStanding[];
	categories: ExportedCategory[];
	games: ExportedGame[];
}
