interface StatusBarProps {
	available: boolean | null;
}

export function StatusBar({ available }: StatusBarProps) {
	const label =
		available === null
			? "Checking..."
			: available
				? "Apple Intelligence: Available"
				: "Apple Intelligence: Unavailable";
	const className = `status-bar ${available === true ? "available" : available === false ? "unavailable" : ""}`;

	return <div className={className}>{label}</div>;
}
