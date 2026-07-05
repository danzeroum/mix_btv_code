import { useAppDispatch, useAppState } from '../../state/AppContext'
import { NAV_BY_PERSONA } from '../../lib/nav'

export function Sidebar() {
  const { persona, screen } = useAppState()
  const dispatch = useAppDispatch()
  const items = NAV_BY_PERSONA[persona]
  const heading = persona === 'user' ? 'Superfícies do usuário' : 'Painéis de administração'

  return (
    <nav
      style={{
        width: 262,
        flexShrink: 0,
        background: 'var(--bg2)',
        borderRight: '1px solid var(--line)',
        padding: '16px 10px',
        display: 'flex',
        flexDirection: 'column',
        gap: 4,
      }}
    >
      <div
        style={{
          fontSize: 11,
          textTransform: 'uppercase',
          letterSpacing: '.06em',
          color: 'var(--faint)',
          padding: '4px 10px 8px',
        }}
      >
        {heading}
      </div>

      {items.map((item) => {
        const active = item.id === screen
        return (
          <button
            key={item.id}
            onClick={() => dispatch({ type: 'SET_SCREEN', screen: item.id })}
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: 10,
              textAlign: 'left',
              padding: '8px 10px',
              borderRadius: 7,
              border: 'none',
              background: active ? 'var(--panel2)' : 'transparent',
              boxShadow: active ? 'inset 2px 0 0 var(--rust)' : 'none',
              color: active ? 'var(--ink)' : 'var(--muted)',
            }}
          >
            <span style={{ width: 22, textAlign: 'center' }}>{item.icon}</span>
            <span style={{ flex: 1 }}>
              <div style={{ fontSize: 13, fontWeight: 600 }}>{item.label}</div>
              <div style={{ fontSize: 11, color: 'var(--faint)' }}>{item.hint}</div>
            </span>
          </button>
        )
      })}

      <div style={{ flex: 1 }} />

      <div
        style={{
          fontSize: 11,
          color: 'var(--faint)',
          border: '1px solid var(--line)',
          borderRadius: 8,
          padding: 10,
          margin: '8px 4px',
        }}
      >
        <strong style={{ color: 'var(--muted)' }}>regra de fronteira</strong>
        <br />
        Rust: disco/rede/processo/segredo. Python: raciocínio de agente. Keys só no Rust.
      </div>
    </nav>
  )
}
