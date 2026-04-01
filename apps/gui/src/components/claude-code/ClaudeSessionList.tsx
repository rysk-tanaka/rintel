import { formatSessionTime } from "../../formatTime";
import type { ClaudeSessionSummary } from "../../types";

interface ClaudeSessionListProps {
	sessions: ClaudeSessionSummary[];
	activeId: string | null;
	onSelect: (sessionId: string) => void;
}

export function ClaudeSessionList({
	sessions,
	activeId,
	onSelect,
}: ClaudeSessionListProps) {
	return (
		<div className="cc-session-list">
			<div className="cc-panel-header">Sessions</div>
			<div className="cc-list">
				{sessions.map((s) => (
					<button
						key={s.session_id}
						type="button"
						className={`cc-list-item ${s.session_id === activeId ? "active" : ""}`}
						onClick={() => onSelect(s.session_id)}
					>
						<span className="cc-item-title">
							{s.slug ?? s.session_id.slice(0, 8)}
						</span>
						<span className="cc-item-meta">
							{s.timestamp && (
								<>
									{formatSessionTime(s.timestamp)}
									{" · "}
								</>
							)}
							{s.message_count} msgs
						</span>
					</button>
				))}
				{sessions.length === 0 && (
					<div className="cc-empty">No sessions</div>
				)}
			</div>
		</div>
	);
}
