import { useClaudeCode } from "../../hooks/useClaudeCode";
import { ClaudeConversation } from "./ClaudeConversation";
import { ClaudeSessionList } from "./ClaudeSessionList";
import { ProjectList } from "./ProjectList";

export function ClaudeCodeView() {
	const {
		projects,
		selectedProject,
		sessions,
		selectedSession,
		loading,
		selectProject,
		selectSession,
	} = useClaudeCode();

	return (
		<div className="cc-view">
			<ProjectList
				projects={projects}
				activeDir={selectedProject}
				onSelect={selectProject}
			/>
			{selectedProject && (
				<ClaudeSessionList
					sessions={sessions}
					activeId={selectedSession?.session_id ?? null}
					onSelect={selectSession}
				/>
			)}
			<div className="cc-main">
				{loading && <div className="cc-loading">Loading...</div>}
				{!loading && selectedSession && (
					<ClaudeConversation session={selectedSession} />
				)}
				{!loading && !selectedSession && (
					<div className="empty-state">
						<p>Select a project and session to view the conversation.</p>
					</div>
				)}
			</div>
		</div>
	);
}
