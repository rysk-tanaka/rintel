import { formatSessionTime } from "../formatTime";
import type { SessionSummary } from "../types";

interface SessionSidebarProps {
	sessions: SessionSummary[];
	activeId: string | null;
	onSelect: (id: string) => void;
	onCreate: () => void;
	onDelete: (id: string) => void;
}

export function SessionSidebar({
	sessions,
	activeId,
	onSelect,
	onCreate,
	onDelete,
}: SessionSidebarProps) {
	return (
		<aside className="sidebar">
			<button type="button" className="btn-new-session" onClick={onCreate}>
				+ New Session
			</button>
			<div className="session-list">
				{sessions.map((s) => (
					<div
						key={s.id}
						className={`session-item ${s.id === activeId ? "active" : ""} ${s.expired ? "expired" : ""}`}
					>
						<button
							type="button"
							className="session-item-select"
							onClick={() => onSelect(s.id)}
						>
							<span className="session-title">
								{s.title ?? s.id.slice(0, 8)}
							</span>
							<span className="session-meta">
								{s.message_count} msgs · {formatSessionTime(s.last_active)}
							</span>
						</button>
						<button
							type="button"
							className="session-item-delete"
							onClick={(e) => {
								e.stopPropagation();
								onDelete(s.id);
							}}
							aria-label="Delete session"
						>
							x
						</button>
					</div>
				))}
			</div>
		</aside>
	);
}
