export type Role = "user" | "assistant";

export interface Message {
	role: Role;
	content: string;
	timestamp: string;
}

export interface FileContext {
	filename: string;
	content: string;
}

export interface SessionSummary {
	id: string;
	title: string | null;
	last_active: string;
	message_count: number;
	expired: boolean;
}

export interface SessionDetail {
	id: string;
	title: string | null;
	system_prompt: string | null;
	messages: Message[];
	file_contexts: FileContext[];
}

export interface SessionInfo {
	id: string;
}

export interface SendMessageResponse {
	content: string;
	messages: Message[];
}

export interface FileContextInfo {
	filename: string;
	size: number;
}

// Claude Code session viewer types

export interface ClaudeProject {
	dir_name: string;
	decoded_path: string;
}

export interface ClaudeSessionSummary {
	session_id: string;
	slug: string | null;
	timestamp: string | null;
	message_count: number;
}

export interface ClaudeToolUse {
	name: string;
	input_preview: string;
}

export interface ClaudeMessage {
	role: string;
	timestamp: string | null;
	text_content: string;
	tool_uses: ClaudeToolUse[];
	uuid: string | null;
}

export interface ClaudeSessionDetail {
	session_id: string;
	slug: string | null;
	git_branch: string | null;
	messages: ClaudeMessage[];
}
