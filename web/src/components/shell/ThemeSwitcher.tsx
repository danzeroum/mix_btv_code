import { useAppDispatch, useAppState } from '../../state/AppContext'
import { THEME_LIST } from '../../styles/themes'

export function ThemeSwitcher() {
  const { theme } = useAppState()
  const dispatch = useAppDispatch()

  return (
    <div className="row" role="group" aria-label="Tema">
      {THEME_LIST.map((t) => {
        const active = t.key === theme
        return (
          <button
            key={t.key}
            title={t.label}
            onClick={() => dispatch({ type: 'SET_THEME', theme: t.key })}
            style={{
              fontSize: 11,
              padding: '4px 9px',
              borderRadius: 6,
              border: `1px solid ${active ? 'var(--ink)' : 'var(--line)'}`,
              background: active ? 'var(--panel2)' : 'transparent',
              color: active ? 'var(--ink)' : 'var(--muted)',
            }}
          >
            {t.label}
          </button>
        )
      })}
    </div>
  )
}
