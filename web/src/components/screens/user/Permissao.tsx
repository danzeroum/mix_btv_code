import { useEffect, useState } from 'react'
import { Modal } from '../../primitives/Modal'
import { Button } from '../../primitives/Button'
import { useToast } from '../../primitives/Toast'
import { useSession } from '../../../state/SessionContext'
import { useAppState } from '../../../state/AppContext'
import { setRule } from '../../../api/permissions'

export function Permissao() {
  const toast = useToast()
  const { pending, resolvePermission } = useSession()
  const { agentProfile } = useAppState()
  const [grantingAlways, setGrantingAlways] = useState(false)

  async function handleDecision(allow: boolean) {
    try {
      await resolvePermission(allow)
      toast.push('success', allow ? 'permitido' : 'negado')
    } catch {
      toast.push('error', 'falha ao resolver permissão')
    }
  }

  /** "sempre": grava um override persistido (Rule) restrito ao MESMO escopo
   * já mostrado nesta tela — não é um "allow" genérico para o tool inteiro,
   * é a repetição deste pedido específico que passa a ser auto-aprovada.
   * A matriz build/plan×tool (tela Skills) é quem cobre o caso "qualquer
   * escopo". Gravar primeiro, resolver depois: se a gravação falhar, o
   * pedido atual continua pendente em vez de ser aprovado sem rastro.
   */
  async function handleAlways() {
    if (!pending) return
    setGrantingAlways(true)
    try {
      await setRule(agentProfile, pending.tool, 'allow', pending.scope)
      await resolvePermission(true)
      toast.push('success', `"sempre" gravado para ${pending.tool} · ${agentProfile}`)
    } catch {
      toast.push('error', 'falha ao gravar permissão "sempre"')
    } finally {
      setGrantingAlways(false)
    }
  }

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (!pending) return
      if (e.key === 's') void handleDecision(true)
      if (e.key === 'n') void handleDecision(false)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pending])

  if (!pending) {
    return (
      <div className="mono" style={{ padding: 18, color: 'var(--faint)', fontSize: 13 }}>
        nenhum pedido de permissão pendente — envie uma mensagem em Sessão que peça uma ferramenta sob política
        &quot;ask&quot; (ex.: bash) para ver o gate aqui.
      </div>
    )
  }

  return (
    <div style={{ position: 'relative', height: '100%' }}>
      <Modal>
        <div style={{ padding: 18 }}>
          <div className="row" style={{ fontWeight: 600 }}>
            <span
              style={{
                width: 8,
                height: 8,
                borderRadius: '50%',
                background: 'var(--wire)',
                boxShadow: '0 0 8px var(--wire)',
              }}
            />
            Permissão solicitada
          </div>
          <div style={{ fontSize: 11, color: 'var(--faint)', marginBottom: 12 }}>forge-core · não contornável</div>

          <div className="mono" style={{ fontSize: 13, marginBottom: 6 }}>
            ⚒ {pending.tool}
          </div>
          <div
            className="mono"
            style={{
              fontSize: 12,
              background: '#0a0d12',
              border: '1px solid var(--line)',
              borderRadius: 6,
              padding: 8,
              marginBottom: 14,
            }}
          >
            {pending.scope}
          </div>

          <div className="row" style={{ justifyContent: 'flex-end' }}>
            <Button
              variant="primary"
              style={{ background: 'var(--ok)', color: '#08170c' }}
              onClick={() => void handleDecision(true)}
            >
              [s] Permitir
            </Button>
            <Button
              onClick={() => void handleAlways()}
              disabled={grantingAlways}
              title={`grava um override persistido para ${pending.tool} · ${agentProfile} restrito a este escopo`}
            >
              {grantingAlways ? 'gravando…' : 'sempre'}
            </Button>
            <Button onClick={() => void handleDecision(false)}>[n] Negar</Button>
          </div>
        </div>
      </Modal>
    </div>
  )
}
