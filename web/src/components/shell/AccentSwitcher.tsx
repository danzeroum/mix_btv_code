import { useAppDispatch, useAppState } from '../../state/AppContext'
import { ACCENTS } from '../../styles/themes'

export function AccentSwitcher() {
  const { accent } = useAppState()
  const dispatch = useAppDispatch()

  return (
    <div className="row" role="group" aria-label="Cor de destaque — cada usuário edita a sua">
      {ACCENTS.map((swatch) => {
        const active = swatch.color === accent
        return (
          <button
            key={swatch.color ?? 'null'}
            title={swatch.label}
            onClick={() => dispatch({ type: 'SET_ACCENT', accent: swatch.color })}
            style={{
              width: 18,
              height: 18,
              borderRadius: '50%',
              border: swatch.color ? `1px dashed transparent` : '1px dashed var(--muted)',
              background: swatch.color ?? 'transparent',
              boxShadow: active ? '0 0 0 2px var(--ink)' : 'none',
              padding: 0,
            }}
          />
        )
      })}
    </div>
  )
}
