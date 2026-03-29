import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import type { SessionDetail, SessionInfo, SessionSummary } from "../types";

export function useSessions() {
	const [sessions, setSessions] = useState<SessionSummary[]>([]);
	const [activeSession, setActiveSession] = useState<SessionDetail | null>(
		null,
	);

	const refresh = useCallback(async () => {
		const list = await invoke<SessionSummary[]>("list_sessions");
		setSessions(list);
	}, []);

	useEffect(() => {
		refresh();
	}, [refresh]);

	const createSession = useCallback(
		async (systemPrompt?: string) => {
			const info = await invoke<SessionInfo>("create_session", {
				systemPrompt: systemPrompt ?? null,
			});
			await refresh();
			const detail = await invoke<SessionDetail>("load_session", {
				id: info.id,
			});
			setActiveSession(detail);
			return detail;
		},
		[refresh],
	);

	const selectSession = useCallback(async (id: string) => {
		const detail = await invoke<SessionDetail>("load_session", { id });
		setActiveSession(detail);
	}, []);

	const deleteSession = useCallback(
		async (id: string) => {
			await invoke("delete_session", { id });
			if (activeSession?.id === id) {
				setActiveSession(null);
			}
			await refresh();
		},
		[activeSession, refresh],
	);

	const cleanupSessions = useCallback(async () => {
		const count = await invoke<number>("cleanup_sessions");
		await refresh();
		return count;
	}, [refresh]);

	return {
		sessions,
		activeSession,
		setActiveSession,
		createSession,
		selectSession,
		deleteSession,
		cleanupSessions,
		refresh,
	};
}
