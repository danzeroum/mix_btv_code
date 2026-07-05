import type { CSSProperties, ReactNode } from 'react'

export function Card({
  children,
  accentBorder,
  style,
}: {
  children: ReactNode
  accentBorder?: string
  style?: CSSProperties
}) {
  return (
    <div
      style={{
        background: 'var(--panel)',
        border: `1px solid ${accentBorder ?? 'var(--line)'}`,
        borderRadius: 11,
        padding: 16,
        ...style,
      }}
    >
      {children}
    </div>
  )
}
