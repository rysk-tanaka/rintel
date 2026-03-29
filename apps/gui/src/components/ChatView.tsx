import { useCallback, useEffect, useRef, useState } from "react";
import type { Message, SessionDetail } from "../types";
import { FileDropZone } from "./FileDropZone";
import { MessageBubble } from "./MessageBubble";

interface ChatViewProps {
	session: SessionDetail;
	sending: boolean;
	onSendMessage: (
		sessionId: string,
		message: string,
	) => Promise<{ content: string; messages: Message[] }>;
	onSessionUpdate: () => void;
}

export function ChatView({
	session,
	sending,
	onSendMessage,
	onSessionUpdate,
}: ChatViewProps) {
	const [input, setInput] = useState("");
	const [messages, setMessages] = useState<Message[]>(session.messages);
	const messagesEndRef = useRef<HTMLDivElement>(null);

	useEffect(() => {
		setMessages(session.messages);
	}, [session.messages]);

	useEffect(() => {
		messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
	}, [messages]);

	const handleSubmit = useCallback(
		async (e: React.FormEvent) => {
			e.preventDefault();
			const text = input.trim();
			if (!text || sending) return;

			setInput("");
			setMessages((prev) => [
				...prev,
				{ role: "user", content: text, timestamp: new Date().toISOString() },
			]);

			const res = await onSendMessage(session.id, text);
			setMessages(res.messages);
		},
		[input, sending, session.id, onSendMessage],
	);

	return (
		<main className="chat-view">
			<div className="chat-messages">
				{messages.map((msg, i) => (
					<MessageBubble key={`${session.id}-${i}`} message={msg} />
				))}
				{sending && (
					<div className="message message-assistant">
						<div className="message-role">AI</div>
						<div className="message-content thinking">Thinking...</div>
					</div>
				)}
				<div ref={messagesEndRef} />
			</div>
			<div className="chat-input-area">
				<FileDropZone
					sessionId={session.id}
					fileContexts={session.file_contexts}
					onFileAdded={onSessionUpdate}
				/>
				<form className="chat-form" onSubmit={handleSubmit}>
					<input
						type="text"
						className="chat-input"
						value={input}
						onChange={(e) => setInput(e.target.value)}
						placeholder="Type a message..."
						disabled={sending}
					/>
					<button
						type="submit"
						className="btn-send"
						disabled={sending || !input.trim()}
					>
						Send
					</button>
				</form>
			</div>
		</main>
	);
}
