interface WorkspacePageHeaderProps {
  title: string;
  description: string;
}

export function WorkspacePageHeader({ title, description }: WorkspacePageHeaderProps) {
  return (
    <header className="space-y-2">
      <h1 className="text-xl font-semibold tracking-tight">{title}</h1>
      <p className="max-w-2xl text-sm leading-relaxed text-muted-foreground">{description}</p>
    </header>
  );
}
