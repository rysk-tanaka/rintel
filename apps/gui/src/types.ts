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
