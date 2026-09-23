export type PaneSide = 'old' | 'new';
export type AnswerStatus = 'pending' | 'streaming' | 'done' | 'error' | 'cancelled';
export type Answer = {status: AnswerStatus; text: string; error?: string; tools: number; agent?: string};

export type Turn = {message: string; answer?: Answer; prior?: Answer[]}; // prior: earlier answers on this turn, oldest first

export type Question = {
	id?: string; // stable id, set on send
	file: string;
	index: number; // cursor row
	side?: PaneSide; // split view pane of the cursor (unified/browse: 'new')
	line?: number;
	text: string; // cursor line, or selected text (joined with \n) when a selection was active
	message: string;
	// only with a selection: 1-based lines and 1-based inclusive cols
	startLine?: number;
	endLine?: number;
	startCol?: number;
	endCol?: number;
	context?: string[]; // code lines around the anchor (prompt context)
	wide?: string[]; // more surrounding lines (wider prompt context)
	answer?: Answer; // mirrors the LATEST turn's answer
	turns?: Turn[]; // thread: turn 1 = message/answer; set by the controller
	origin?: 'agent'; // comment added by an external responder
	number?: number; // optional order label; ( ) jump between numbered comments
};
