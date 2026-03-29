import { formatTime } from "../formatTime";
import type { Message } from "../types";

interface MessageBubbleProps {
	message: Message;
}

export function MessageBubble({ message }: MessageBubbleProps) {
	const isUser = message.role === "user";
	return (
		<div className={`message ${isUser ? "message-user" : "message-assistant"}`}>
			<div className="message-header">
				<span className="message-role">{isUser ? "You" : "AI"}</span>
				<span className="message-time">{formatTime(message.timestamp)}</span>
			</div>
			<div className="message-content">{message.content}</div>
		</div>
	);
}
