import { useEffect, useRef } from "react";
import { formatTime } from "../../formatTime";
import type { ClaudeMessage, ClaudeSessionDetail, ClaudeToolUse } from "../../types";

interface ClaudeConversationProps {
	session: ClaudeSessionDetail;
}

function ToolUseBlock({ tool }: { tool: ClaudeToolUse }) {
	return (
		<details className="cc-tool-use">
			<summary className="cc-tool-use-summary">{tool.name}</summary>
			<pre className="cc-tool-use-detail">{tool.input_preview}</pre>
		</details>
	);
}

function MessageBlock({ message }: { message: ClaudeMessage }) {
	const isUser = message.role === "user";
	return (
		<div className={`message ${isUser ? "message-user" : "message-assistant"}`}>
			<div className="message-header">
				<span className="message-role">{isUser ? "You" : "Claude"}</span>
				{message.timestamp && (
					<span className="message-time">{formatTime(message.timestamp)}</span>
				)}
			</div>
			{message.text_content && (
				<div className="message-content">{message.text_content}</div>
			)}
			{message.tool_uses.length > 0 && (
				<div className="cc-tool-uses">
					{message.tool_uses.map((tool, i) => (
						<ToolUseBlock key={`${message.uuid}-tool-${i}`} tool={tool} />
					))}
				</div>
			)}
		</div>
	);
}

export function ClaudeConversation({ session }: ClaudeConversationProps) {
	const endRef = useRef<HTMLDivElement>(null);

	useEffect(() => {
		endRef.current?.scrollIntoView({ behavior: "smooth" });
	}, [session.session_id]);

	return (
		<div className="cc-conversation">
			<div className="cc-conv-header">
				<span className="cc-conv-title">
					{session.slug ?? session.session_id.slice(0, 8)}
				</span>
				{session.git_branch && (
					<span className="cc-conv-branch">{session.git_branch}</span>
				)}
			</div>
			<div className="cc-messages">
				{session.messages.map((msg, i) => (
					<MessageBlock key={msg.uuid ?? i} message={msg} />
				))}
				<div ref={endRef} />
			</div>
		</div>
	);
}
