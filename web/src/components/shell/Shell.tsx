import { useRef } from 'react'
import { useAppState } from '../../state/AppContext'
import { useTheme } from '../../state/useTheme'
import { Topbar } from './Topbar'
import { Sidebar } from './Sidebar'
import { SCREEN_META } from '../../lib/screenMeta'
import { SCREEN_COMPONENTS } from '../../lib/screenComponents'

const ADMIN_SURFACE_SCREENS = new Set(['telemetria', 'ledger', 'verify', 'providers', 'skills', 'sugestoes'])

export function Shell() {
  const rootRef = useRef<HTMLDivElement | null>(null)
  const { screen, theme, accent } = useAppState()
  useTheme(rootRef, theme, accent)

  const meta = SCREEN_META[screen]
  const ScreenComponent = SCREEN_COMPONENTS[screen]
  const stageClass = ADMIN_SURFACE_SCREENS.has(screen) ? 'surf' : 'term'

  return (
    <div id="forge-root" ref={rootRef}>
      <Topbar />
      <div className="forge-body">
        <Sidebar />
        <main className={`forge-stage ${stageClass}`} style={{ position: 'relative' }}>
          <div className="screen-header">
            <div>
              <div className="screen-kicker">{meta.kicker}</div>
              <h1 className="screen-title">{meta.title}</h1>
            </div>
            <div className="screen-note">{meta.note}</div>
          </div>
          <ScreenComponent />
        </main>
      </div>
    </div>
  )
}
