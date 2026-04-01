import type { ClaudeProject } from "../../types";

interface ProjectListProps {
	projects: ClaudeProject[];
	activeDir: string | null;
	onSelect: (dirName: string) => void;
}

export function ProjectList({ projects, activeDir, onSelect }: ProjectListProps) {
	return (
		<div className="cc-project-list">
			<div className="cc-panel-header">Projects</div>
			<div className="cc-list">
				{[...projects]
				.map((p) => {
					const segments = p.decoded_path.split("/").filter(Boolean);
					const label = segments.slice(-2).join("/") || p.dir_name;
					return { ...p, label };
				})
				.sort((a, b) => a.label.localeCompare(b.label))
				.map(({ label, ...p }) => {
					return (
						<button
							key={p.dir_name}
							type="button"
							className={`cc-list-item ${p.dir_name === activeDir ? "active" : ""}`}
							onClick={() => onSelect(p.dir_name)}
							title={p.decoded_path}
						>
							<span className="cc-item-title">{label}</span>
						</button>
					);
				})}
				{projects.length === 0 && (
					<div className="cc-empty">No projects found</div>
				)}
			</div>
		</div>
	);
}
