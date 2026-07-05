import { useAppState } from '../../state/AppContext'
import { PersonaToggle } from './PersonaToggle'
import { ThemeSwitcher } from './ThemeSwitcher'
import { AccentSwitcher } from './AccentSwitcher'

export function Topbar() {
  const { persona } = useAppState()
  const healthLabel = persona === 'user' ? 'sidecar saudável' : 'localhost · offline-first'

  return (
    <header
      style={{
        position: 'sticky',
        top: 0,
        zIndex: 20,
        borderBottom: '1px solid var(--line)',
        background: 'var(--bg2)',
        display: 'flex',
        alignItems: 'center',
        gap: 20,
        padding: '10px 20px',
      }}
    >
      <div className="row" style={{ fontWeight: 700 }}>
        <span
          style={{
            width: 26,
            height: 26,
            borderRadius: 7,
            background: 'linear-gradient(135deg, var(--rust), var(--amber))',
            display: 'inline-block',
          }}
        />
        Forge
      </div>

      <PersonaToggle />

      <div style={{ flex: 1 }} />

      <ThemeSwitcher />
      <AccentSwitcher />

      <div className="row" style={{ fontSize: 12, color: 'var(--muted)' }}>
        <span className="pulse-dot" />
        {healthLabel}
        <span style={{ color: 'var(--faint)' }}>· danzeroum/mix_btv_code</span>
      </div>
    </header>
  )
}
