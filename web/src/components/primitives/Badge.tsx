export function Badge({ children, color = 'var(--muted)' }: { children: React.ReactNode; color?: string }) {
  return (
    <span
      style={{
        display: 'inline-block',
        fontSize: 11,
        fontWeight: 600,
        padding: '2px 8px',
        borderRadius: 999,
        border: `1px solid ${color}`,
        color,
      }}
    >
      {children}
    </span>
  )
}
