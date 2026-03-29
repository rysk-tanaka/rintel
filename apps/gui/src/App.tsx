import { ChatView } from "./components/ChatView";
import { SessionSidebar } from "./components/SessionSidebar";
import { StatusBar } from "./components/StatusBar";
import { useAi } from "./hooks/useAi";
import { useSessions } from "./hooks/useSessions";

function App() {
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
			<div className="app-body">
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
			</div>
			<StatusBar available={available} />
		</div>
	);
}

export default App;
