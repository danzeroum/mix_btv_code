import type { ComponentType } from 'react'
import type { ScreenId } from '../types/domain'
import { Onboarding } from '../components/screens/user/Onboarding'
import { Sessao } from '../components/screens/user/Sessao'
import { Permissao } from '../components/screens/user/Permissao'
import { Modelo } from '../components/screens/user/Modelo'
import { Prompts } from '../components/screens/user/Prompts'
import { Squad } from '../components/screens/user/Squad'
import { Designer } from '../components/screens/user/Designer/Designer'
import { Sugestoes } from '../components/screens/user/Sugestoes'
import { Telemetria } from '../components/screens/admin/Telemetria'
import { Mcp } from '../components/screens/admin/Mcp'
import { Modelos } from '../components/screens/admin/Modelos'
import { Memoria } from '../components/screens/admin/Memoria'
import { Ledger } from '../components/screens/admin/Ledger'
import { Verify } from '../components/screens/admin/Verify'
import { Providers } from '../components/screens/admin/Providers'
import { Skills } from '../components/screens/admin/Skills'

export const SCREEN_COMPONENTS: Record<ScreenId, ComponentType> = {
  onboarding: Onboarding,
  sessao: Sessao,
  permissao: Permissao,
  modelo: Modelo,
  prompts: Prompts,
  squad: Squad,
  designer: Designer,
  sugestoes: Sugestoes,
  telemetria: Telemetria,
  mcp: Mcp,
  modelos: Modelos,
  memoria: Memoria,
  ledger: Ledger,
  verify: Verify,
  providers: Providers,
  skills: Skills,
}
