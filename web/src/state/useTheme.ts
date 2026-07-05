import { useEffect } from 'react'
import { THEMES, type ThemeId } from '../styles/themes'

/** Aplica as ~18 CSS custom properties do tema em #forge-root, sobrepondo
 * --rust com `accent` quando presente. Roda a cada troca de theme/accent —
 * a restauração inicial (antes do primeiro paint) é feita pelo lazy
 * initializer do reducer em AppContext, não aqui.
 */
export function useTheme(rootRef: React.RefObject<HTMLElement | null>, theme: ThemeId, accent: string | null) {
  useEffect(() => {
    const el = rootRef.current
    if (!el) return
    const palette = THEMES[theme]
    for (const [key, value] of Object.entries(palette)) {
      el.style.setProperty(`--${key}`, value)
    }
    if (accent) {
      el.style.setProperty('--rust', accent)
    }
  }, [rootRef, theme, accent])
}
