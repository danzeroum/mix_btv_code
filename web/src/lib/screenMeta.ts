import type { ScreenId } from '../types/domain'

export interface ScreenMeta {
  kicker: string
  title: string
  note: string
}

export const SCREEN_META: Record<ScreenId, ScreenMeta> = {
  onboarding: { kicker: 'setup', title: 'Primeiros passos', note: 'instalar · chaves de API · doctor' },
  sessao: { kicker: 'forge run/chat', title: 'Sessão de código', note: 'transcript ao vivo' },
  permissao: { kicker: 'forge-core', title: 'Permissão interativa', note: 'não contornável' },
  modelo: { kicker: 'config', title: 'Modelo, agente & autonomia', note: 'tier · perfil · nível' },
  prompts: { kicker: '/prompt', title: 'Biblioteca de prompts', note: 'geradores · salvos' },
  squad: { kicker: 'forge squad', title: 'Squad ao vivo', note: 'consenso ponderado · HITL' },
  designer: { kicker: 'conceito · fase 4+', title: 'Squad Designer', note: 'arrastar · conectar · salvar' },
  sugestoes: { kicker: 'roadmap', title: 'Sugestões de interação', note: 'próximas telas' },
  telemetria: { kicker: '127.0.0.1:7878', title: 'Telemetria', note: 'atualiza a cada 5s' },
  ledger: { kicker: 'hash-chain', title: 'Ledger / Auditoria', note: 'append-only · Nada Fake' },
  verify: { kicker: '/verify', title: 'Verificação & review por valor', note: 'gate > 0.70' },
  providers: { kicker: 'gateway LLM', title: 'Providers & rate limits', note: 'keys só no Rust' },
  skills: { kicker: 'vetting · MCP', title: 'Skills, MCP & permissões', note: 'sidecar health' },
}
