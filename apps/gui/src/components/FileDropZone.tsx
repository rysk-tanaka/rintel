import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useCallback } from "react";
import type { FileContext, FileContextInfo } from "../types";

interface FileDropZoneProps {
	sessionId: string;
	fileContexts: FileContext[];
	onFileAdded: () => void;
}

export function FileDropZone({
	sessionId,
	fileContexts,
	onFileAdded,
}: FileDropZoneProps) {
	const handleAddFile = useCallback(async () => {
		const path = await open({ multiple: false });
		if (!path) return;

		await invoke<FileContextInfo>("add_file_context", {
			sessionId,
			path,
		});
		onFileAdded();
	}, [sessionId, onFileAdded]);

	return (
		<div className="file-drop-zone">
			{fileContexts.length > 0 && (
				<div className="file-list">
					{fileContexts.map((fc) => (
						<span key={fc.filename} className="file-tag">
							{fc.filename}
						</span>
					))}
				</div>
			)}
			<button type="button" className="btn-add-file" onClick={handleAddFile}>
				+ File
			</button>
		</div>
	);
}
