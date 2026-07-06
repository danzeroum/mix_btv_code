import type { ScreenId } from '../types/domain'

export interface ScreenMeta {
  kicker: string
  title: string
  note: string
  /** Cor de destaque da tela (CSS var), usada no kicker e na pílula de chrome da janela (README §4.2). */
  accent: string
  chromeIcon: string
  chromeRight: string
}

export const SCREEN_META: Record<ScreenId, ScreenMeta> = {
  onboarding: { kicker: 'setup', title: 'Primeiros passos', note: 'instalar · chaves de API · doctor', accent: 'var(--amber)', chromeIcon: '✦', chromeRight: 'forge init' },
  sessao: { kicker: 'forge run/chat', title: 'Sessão de código', note: 'transcript ao vivo', accent: 'var(--py)', chromeIcon: '▸', chromeRight: 'terminal' },
  permissao: { kicker: 'forge-core', title: 'Permissão interativa', note: 'não contornável', accent: 'var(--wire)', chromeIcon: '⚿', chromeRight: 'gate' },
  modelo: { kicker: 'config', title: 'Modelo, agente & autonomia', note: 'tier · perfil · nível', accent: 'var(--rust)', chromeIcon: '◑', chromeRight: 'config' },
  prompts: { kicker: '/prompt', title: 'Biblioteca de prompts', note: 'geradores · salvos', accent: 'var(--teal)', chromeIcon: '❯', chromeRight: 'promptforge' },
  squad: { kicker: 'forge squad', title: 'Squad ao vivo', note: 'consenso ponderado · HITL', accent: 'var(--amber)', chromeIcon: '⧉', chromeRight: 'squad' },
  designer: { kicker: 'conceito · fase 4+', title: 'Squad Designer', note: 'arrastar · conectar · salvar', accent: 'var(--wire)', chromeIcon: '⬒', chromeRight: 'designer' },
  sugestoes: { kicker: 'roadmap', title: 'Sugestões de interação', note: 'próximas telas', accent: 'var(--py)', chromeIcon: '✧', chromeRight: 'roadmap' },
  telemetria: { kicker: '127.0.0.1:7878', title: 'Telemetria', note: 'atualiza a cada 5s', accent: 'var(--teal)', chromeIcon: '▦', chromeRight: 'dashboard' },
  mcp: { kicker: '.forge/mcp.toml', title: 'Console MCP', note: 'sondagem ao vivo · política real', accent: 'var(--wire)', chromeIcon: '⎈', chromeRight: 'mcp' },
  modelos: { kicker: 'telemetria', title: 'Uso por modelo', note: 'chamadas · cache · tier', accent: 'var(--teal)', chromeIcon: '◈', chromeRight: 'uso' },
  ledger: { kicker: 'hash-chain', title: 'Ledger / Auditoria', note: 'append-only · Nada Fake', accent: 'var(--ok)', chromeIcon: '⛓', chromeRight: 'auditoria' },
  verify: { kicker: '/verify', title: 'Verificação & review por valor', note: 'gate > 0.70', accent: 'var(--ok)', chromeIcon: '✓', chromeRight: 'verify' },
  providers: { kicker: 'gateway LLM', title: 'Providers & rate limits', note: 'keys só no Rust', accent: 'var(--rust)', chromeIcon: '⇄', chromeRight: 'gateway' },
  skills: { kicker: 'vetting', title: 'Skills & permissões', note: 'sidecar health', accent: 'var(--amber)', chromeIcon: '⬡', chromeRight: 'skills' },
}
