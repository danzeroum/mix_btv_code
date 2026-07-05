import type { ReactNode } from 'react'
import { LITERAL_COLORS } from '../../styles/themes'

/** A "janela" que emoldura cada tela (README §4.2) — semáforos + pílula de
 * chrome central + rótulo à direita. Cores dos semáforos são literais
 * intencionais que não trocam com o tema (README §8.3).
 */
export function WindowChrome({
  icon,
  title,
  right,
  children,
}: {
  icon: string
  title: string
  right: string
  children: ReactNode
}) {
  return (
    <div
      style={{
        border: '1px solid var(--line2)',
        borderRadius: 12,
        boxShadow: '0 24px 60px -20px #000a',
        minHeight: 560,
        overflow: 'hidden',
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      <div
        className="row"
        style={{
          background: 'var(--bg2)',
          borderBottom: '1px solid var(--line)',
          padding: '8px 12px',
          justifyContent: 'space-between',
        }}
      >
        <div className="row">
          {LITERAL_COLORS.trafficLights.map((c) => (
            <span key={c} style={{ width: 11, height: 11, borderRadius: '50%', background: c, display: 'inline-block' }} />
          ))}
        </div>
        <div
          className="row mono"
          style={{
            background: 'var(--panel)',
            border: '1px solid var(--line)',
            borderRadius: 999,
            padding: '2px 12px',
            fontSize: 11.5,
            color: 'var(--muted)',
          }}
        >
          <span>{icon}</span>
          <span>{title}</span>
        </div>
        <div className="mono" style={{ fontSize: 11, color: 'var(--faint)' }}>
          {right}
        </div>
      </div>
      <div style={{ flex: 1, overflow: 'auto' }}>{children}</div>
    </div>
  )
}
