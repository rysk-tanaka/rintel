import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useRef, useState } from "react";
import type {
	ClaudeProject,
	ClaudeSessionDetail,
	ClaudeSessionSummary,
} from "../types";

export function useClaudeCode() {
	const [projects, setProjects] = useState<ClaudeProject[]>([]);
	const [selectedProject, setSelectedProject] = useState<string | null>(null);
	const [sessions, setSessions] = useState<ClaudeSessionSummary[]>([]);
	const [selectedSession, setSelectedSession] =
		useState<ClaudeSessionDetail | null>(null);
	const [loading, setLoading] = useState(false);

	const projectRequestRef = useRef(0);
	const sessionRequestRef = useRef(0);

	const loadProjects = useCallback(async () => {
		try {
			const list = await invoke<ClaudeProject[]>("list_claude_projects");
			setProjects(list);
		} catch (e) {
			console.error("Failed to load projects:", e);
			setProjects([]);
		}
	}, []);

	useEffect(() => {
		loadProjects();
	}, [loadProjects]);

	const selectProject = useCallback(async (dirName: string) => {
		const requestId = ++projectRequestRef.current;
		++sessionRequestRef.current;
		setSelectedProject(dirName);
		setSelectedSession(null);
		setSessions([]);
		setLoading(true);
		try {
			const list = await invoke<ClaudeSessionSummary[]>(
				"list_claude_sessions",
				{ projectDir: dirName },
			);
			if (projectRequestRef.current !== requestId) return;
			setSessions(list);
		} catch (e) {
			console.error("Failed to load sessions:", e);
			if (projectRequestRef.current === requestId) {
				setSessions([]);
			}
		} finally {
			if (projectRequestRef.current === requestId) {
				setLoading(false);
			}
		}
	}, []);

	const selectSession = useCallback(
		async (sessionId: string) => {
			if (!selectedProject) return;
			const requestId = ++sessionRequestRef.current;
			setLoading(true);
			try {
				const detail = await invoke<ClaudeSessionDetail>(
					"get_claude_session",
					{ projectDir: selectedProject, sessionId },
				);
				if (sessionRequestRef.current !== requestId) return;
				setSelectedSession(detail);
			} catch (e) {
				console.error("Failed to load session:", e);
				if (sessionRequestRef.current === requestId) {
					setSelectedSession(null);
				}
			} finally {
				if (sessionRequestRef.current === requestId) {
					setLoading(false);
				}
			}
		},
		[selectedProject],
	);

	const goBack = useCallback(() => {
		if (selectedSession) {
			setSelectedSession(null);
		} else if (selectedProject) {
			setSelectedProject(null);
			setSessions([]);
		}
	}, [selectedProject, selectedSession]);

	return {
		projects,
		selectedProject,
		sessions,
		selectedSession,
		loading,
		selectProject,
		selectSession,
		goBack,
	};
}
