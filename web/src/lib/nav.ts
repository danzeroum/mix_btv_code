import type { NavItem, Persona, ScreenId } from '../types/domain'

export const USER_NAV: NavItem[] = [
  { id: 'onboarding', icon: '✦', label: 'Primeiros passos', hint: 'instalar · chaves · doctor' },
  { id: 'sessao', icon: '▸', label: 'Sessão de código', hint: 'forge run/chat' },
  { id: 'permissao', icon: '⚿', label: 'Permissão', hint: 'gate interativo' },
  { id: 'modelo', icon: '◑', label: 'Modelo & Agente', hint: 'tier · perfil · autonomia' },
  { id: 'prompts', icon: '❯', label: 'Biblioteca de prompts', hint: '/prompt' },
  { id: 'squad', icon: '⧉', label: 'Squad ao vivo', hint: 'consenso · HITL' },
  { id: 'designer', icon: '⬒', label: 'Squad Designer', hint: 'arrastar · conectar' },
  { id: 'sugestoes', icon: '✧', label: 'Sugestões de interação', hint: 'próximas telas' },
]

export const ADMIN_NAV: NavItem[] = [
  { id: 'telemetria', icon: '▦', label: 'Telemetria', hint: '127.0.0.1:7878' },
  { id: 'mcp', icon: '⎈', label: 'Console MCP', hint: 'servidores · tools · política' },
  { id: 'modelos', icon: '◈', label: 'Uso por modelo', hint: 'chamadas · cache · tier' },
  { id: 'memoria', icon: '⌗', label: 'Memória do squad', hint: 'mapa · busca léxica' },
  { id: 'ledger', icon: '⛓', label: 'Ledger / Auditoria', hint: 'hash-chain append-only' },
  { id: 'verify', icon: '✓', label: 'Verificação & Review', hint: '/verify · value_score' },
  { id: 'providers', icon: '⇄', label: 'Providers & Limites', hint: 'fallback · rate limit' },
  { id: 'skills', icon: '⬡', label: 'Skills & Permissões', hint: 'vetting' },
]

export const NAV_BY_PERSONA: Record<Persona, NavItem[]> = {
  user: USER_NAV,
  admin: ADMIN_NAV,
}

export const DEFAULT_SCREEN: Record<Persona, ScreenId> = {
  user: 'sessao',
  admin: 'telemetria',
}

export function screenBelongsToPersona(persona: Persona, screen: ScreenId): boolean {
  return NAV_BY_PERSONA[persona].some((n) => n.id === screen)
}
