import { useAppDispatch, useAppState } from '../../state/AppContext'

export function PersonaToggle() {
  const { persona } = useAppState()
  const dispatch = useAppDispatch()

  return (
    <div className="row" role="group" aria-label="Perfil ativo">
      <button
        onClick={() => dispatch({ type: 'SET_PERSONA', persona: 'user' })}
        style={personaBtnStyle(persona === 'user')}
      >
        ▸ Usuário
      </button>
      <button
        onClick={() => dispatch({ type: 'SET_PERSONA', persona: 'admin' })}
        style={personaBtnStyle(persona === 'admin')}
      >
        ◨ Administrador
      </button>
    </div>
  )
}

function personaBtnStyle(active: boolean): React.CSSProperties {
  return {
    border: '1px solid var(--line)',
    borderRadius: 7,
    padding: '6px 12px',
    fontSize: 13,
    fontWeight: 600,
    background: active ? 'linear-gradient(135deg, var(--rust), var(--amber))' : 'transparent',
    color: active ? '#1a1205' : 'var(--muted)',
  }
}
