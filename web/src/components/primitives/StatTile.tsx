import { Card } from './Card'

export function StatTile({ value, label }: { value: string; label: string }) {
  return (
    <Card style={{ minWidth: 160 }}>
      <div style={{ fontSize: 26, fontWeight: 700, color: 'var(--ink)' }}>{value}</div>
      <div style={{ fontSize: 12, color: 'var(--muted)', marginTop: 4 }}>{label}</div>
    </Card>
  )
}
