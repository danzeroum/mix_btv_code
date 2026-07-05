import { createContext, useContext, useReducer, type Dispatch, type ReactNode } from 'react'
import type { AgentProfile, ModelTierId, Persona, ScreenId } from '../types/domain'
import { ACCENT_STORAGE_KEY, THEME_STORAGE_KEY, type ThemeId } from '../styles/themes'
import { DEFAULT_SCREEN, screenBelongsToPersona } from '../lib/nav'

export interface AppState {
  persona: Persona
  screen: ScreenId
  theme: ThemeId
  accent: string | null
  /** Selecionados na tela `modelo`; a tela `sessao` lê daqui para o cabeçalho da sessão (README §6 "modelo"). */
  modelTier: ModelTierId
  agentProfile: AgentProfile
}

export type AppAction =
  | { type: 'SET_PERSONA'; persona: Persona }
  | { type: 'SET_SCREEN'; screen: ScreenId }
  | { type: 'SET_THEME'; theme: ThemeId }
  | { type: 'SET_ACCENT'; accent: string | null }
  | { type: 'SET_MODEL_TIER'; tier: ModelTierId }
  | { type: 'SET_AGENT_PROFILE'; profile: AgentProfile }

function readPersisted(): Pick<AppState, 'theme' | 'accent'> {
  try {
    const theme = (localStorage.getItem(THEME_STORAGE_KEY) as ThemeId | null) ?? 'default'
    const accent = localStorage.getItem(ACCENT_STORAGE_KEY)
    return { theme, accent: accent || null }
  } catch {
    // localStorage indisponível (modo privado/sandbox) — degrada para o padrão sem quebrar o app.
    return { theme: 'default', accent: null }
  }
}

function initState(): AppState {
  const { theme, accent } = readPersisted()
  return { persona: 'user', screen: DEFAULT_SCREEN.user, theme, accent, modelTier: 'large', agentProfile: 'build' }
}

function reducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case 'SET_PERSONA': {
      const screen = screenBelongsToPersona(action.persona, state.screen)
        ? state.screen
        : DEFAULT_SCREEN[action.persona]
      return { ...state, persona: action.persona, screen }
    }
    case 'SET_SCREEN':
      return { ...state, screen: action.screen }
    case 'SET_THEME':
      try {
        localStorage.setItem(THEME_STORAGE_KEY, action.theme)
      } catch {
        // ok ignorar — ver readPersisted
      }
      return { ...state, theme: action.theme }
    case 'SET_ACCENT':
      try {
        if (action.accent) localStorage.setItem(ACCENT_STORAGE_KEY, action.accent)
        else localStorage.removeItem(ACCENT_STORAGE_KEY)
      } catch {
        // ok ignorar
      }
      return { ...state, accent: action.accent }
    case 'SET_MODEL_TIER':
      return { ...state, modelTier: action.tier }
    case 'SET_AGENT_PROFILE':
      return { ...state, agentProfile: action.profile }
  }
}

const AppStateContext = createContext<AppState | null>(null)
const AppDispatchContext = createContext<Dispatch<AppAction> | null>(null)

export function AppProvider({ children }: { children: ReactNode }) {
  const [state, dispatch] = useReducer(reducer, undefined, initState)
  return (
    <AppStateContext.Provider value={state}>
      <AppDispatchContext.Provider value={dispatch}>{children}</AppDispatchContext.Provider>
    </AppStateContext.Provider>
  )
}

export function useAppState(): AppState {
  const ctx = useContext(AppStateContext)
  if (!ctx) throw new Error('useAppState deve ser usado dentro de <AppProvider>')
  return ctx
}

export function useAppDispatch(): Dispatch<AppAction> {
  const ctx = useContext(AppDispatchContext)
  if (!ctx) throw new Error('useAppDispatch deve ser usado dentro de <AppProvider>')
  return ctx
}
