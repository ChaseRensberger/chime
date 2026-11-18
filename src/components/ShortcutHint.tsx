export function ShortcutHint({ label }: { label: string }) {
  return (
    <span className="inline-flex items-center gap-1" role="group" aria-label={label}>
      <kbd>{label}</kbd>
    </span>
  );
}
