/** メッセージの時刻を HH:MM 形式で表示 */
export function formatTime(isoString: string): string {
	const date = new Date(isoString);
	return date.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

/** セッション一覧の時刻表示（当日: HH:MM、今年: MM/DD HH:MM、それ以外: YYYY/MM/DD） */
export function formatSessionTime(isoString: string): string {
	const date = new Date(isoString);
	const now = new Date();

	const sameDay =
		date.getFullYear() === now.getFullYear() &&
		date.getMonth() === now.getMonth() &&
		date.getDate() === now.getDate();

	if (sameDay) {
		return date.toLocaleTimeString([], {
			hour: "2-digit",
			minute: "2-digit",
		});
	}

	if (date.getFullYear() === now.getFullYear()) {
		return date.toLocaleDateString([], {
			month: "2-digit",
			day: "2-digit",
			hour: "2-digit",
			minute: "2-digit",
		});
	}

	return date.toLocaleDateString([], {
		year: "numeric",
		month: "2-digit",
		day: "2-digit",
	});
}
