export function ProgressBar({ value, color = 'var(--rust)' }: { value: number; color?: string }) {
  const pct = Math.max(0, Math.min(1, value)) * 100
  return (
    <div style={{ background: 'var(--line)', borderRadius: 999, height: 6, overflow: 'hidden' }}>
      <div
        style={{
          width: `${pct}%`,
          height: '100%',
          background: color,
          transition: 'width .3s ease',
        }}
      />
    </div>
  )
}
