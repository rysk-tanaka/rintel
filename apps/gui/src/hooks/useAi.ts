import { invoke } from "@tauri-apps/api/core";
import { useCallback, useEffect, useState } from "react";
import type { Message, SendMessageResponse } from "../types";

export function useAi() {
	const [available, setAvailable] = useState<boolean | null>(null);
	const [sending, setSending] = useState(false);

	useEffect(() => {
		invoke<boolean>("check_ai_availability").then(setAvailable);
	}, []);

	const sendMessage = useCallback(
		async (
			sessionId: string,
			message: string,
		): Promise<{ content: string; messages: Message[] }> => {
			setSending(true);
			try {
				const res = await invoke<SendMessageResponse>("send_message", {
					sessionId,
					message,
				});
				return res;
			} finally {
				setSending(false);
			}
		},
		[],
	);

	return { available, sending, sendMessage };
}
