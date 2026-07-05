import { useEffect } from 'react'
import { Modal } from '../../primitives/Modal'
import { Button } from '../../primitives/Button'
import { useAsyncAction } from '../../../hooks/useAsyncAction'
import { useToast } from '../../primitives/Toast'
import { PENDING_PERMISSION, resolvePermission } from '../../../api/permissions'
import type { PermissionDecision } from '../../../types/domain'

export function Permissao() {
  const toast = useToast()
  const resolve = useAsyncAction((decision: PermissionDecision) => resolvePermission(PENDING_PERMISSION.id, decision))

  async function handleDecision(decision: PermissionDecision) {
    try {
      const result = await resolve.run(decision)
      const label = decision === 'allow' ? 'permitido' : decision === 'deny' ? 'negado' : 'sempre permitido p/ bash'
      toast.push('success', `${label} · ledger seq ${result.ledgerSeq}`)
    } catch {
      toast.push('error', 'falha ao resolver permissão')
    }
  }

  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === 's') void handleDecision('allow')
      if (e.key === 'n') void handleDecision('deny')
      if (e.key === 'a') void handleDecision('always')
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  return (
    <div style={{ position: 'relative', height: '100%' }}>
      <div style={{ filter: 'blur(1.5px)', opacity: 0.4 }} className="mono">
        você ▸ rode os testes de usuários
        <br />
        forge ▸ vou rodar a suíte de testes agora.
      </div>

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
            ⚒ {PENDING_PERMISSION.tool}
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
            {PENDING_PERMISSION.scope}
          </div>

          <div className="row" style={{ justifyContent: 'flex-end' }}>
            <Button
              variant="primary"
              style={{ background: 'var(--ok)', color: '#08170c' }}
              onClick={() => void handleDecision('allow')}
            >
              [s] Permitir
            </Button>
            <Button onClick={() => void handleDecision('deny')}>[n] Negar</Button>
            <Button onClick={() => void handleDecision('always')}>[a] Sempre p/ {PENDING_PERMISSION.tool}</Button>
          </div>
        </div>
      </Modal>
    </div>
  )
}
