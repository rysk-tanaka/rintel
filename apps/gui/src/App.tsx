import { useState } from "react";
import { ChatView } from "./components/ChatView";
import { ClaudeCodeView } from "./components/claude-code/ClaudeCodeView";
import { SessionSidebar } from "./components/SessionSidebar";
import { StatusBar } from "./components/StatusBar";
import { useAi } from "./hooks/useAi";
import { useSessions } from "./hooks/useSessions";

type AppView = "chat" | "claude-code";

function App() {
	const [currentView, setCurrentView] = useState<AppView>("chat");
	const { available, sending, sendMessage } = useAi();
	const {
		sessions,
		activeSession,
		createSession,
		selectSession,
		deleteSession,
	} = useSessions();

	return (
		<div className="app">
			<div className="tab-bar">
				<button
					type="button"
					className={`tab-btn ${currentView === "chat" ? "active" : ""}`}
					onClick={() => setCurrentView("chat")}
				>
					Chat
				</button>
				<button
					type="button"
					className={`tab-btn ${currentView === "claude-code" ? "active" : ""}`}
					onClick={() => setCurrentView("claude-code")}
				>
					Claude Code
				</button>
			</div>
			<div className="app-body">
				{currentView === "chat" ? (
					<>
						<SessionSidebar
							sessions={sessions}
							activeId={activeSession?.id ?? null}
							onSelect={selectSession}
							onCreate={() => createSession()}
							onDelete={deleteSession}
						/>
						<div className="main-area">
							{activeSession ? (
								<ChatView
									session={activeSession}
									sending={sending}
									onSendMessage={sendMessage}
									onSessionUpdate={async () => {
										if (activeSession) {
											await selectSession(activeSession.id);
										}
									}}
								/>
							) : (
								<div className="empty-state">
									<p>Select a session or create a new one to start chatting.</p>
								</div>
							)}
						</div>
					</>
				) : (
					<ClaudeCodeView />
				)}
			</div>
			<StatusBar available={available} />
		</div>
	);
}

export default App;
