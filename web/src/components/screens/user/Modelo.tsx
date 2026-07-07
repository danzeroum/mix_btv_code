import { Card } from '../../primitives/Card'
import { useToast } from '../../primitives/Toast'
import { useAppDispatch, useAppState } from '../../../state/AppContext'
import { AUTONOMY_LEVELS, MODEL_TIERS } from '../../../api/models'

export function Modelo() {
  const toast = useToast()
  const dispatch = useAppDispatch()
  const { modelTier: tier, agentProfile: agent } = useAppState()

  return (
    <div className="grid grid-2">
      <div className="stack">
        <div style={{ fontSize: 11, color: 'var(--faint)' }}>MODEL TIER</div>
        {MODEL_TIERS.map((t) => (
          <Card key={t.id} accentBorder={t.id === tier ? 'var(--rust)' : undefined}>
            <button
              onClick={() => {
                dispatch({ type: 'SET_MODEL_TIER', tier: t.id })
                toast.push('success', `tier ${t.id} selecionado — aplica à próxima mensagem enviada`)
              }}
              style={{ background: 'transparent', border: 'none', textAlign: 'left', width: '100%', color: 'var(--ink)' }}
            >
              <div className="row" style={{ justifyContent: 'space-between' }}>
                <strong>{t.id}</strong>
                {t.id === tier && <span style={{ color: 'var(--rust)', fontSize: 12 }}>selecionado</span>}
              </div>
              <div style={{ fontSize: 12, color: 'var(--muted)' }}>{t.models}</div>
              {t.label && <div style={{ fontSize: 11, color: 'var(--faint)' }}>{t.label}</div>}
            </button>
          </Card>
        ))}
      </div>

      <div className="stack">
        <div style={{ fontSize: 11, color: 'var(--faint)' }}>PERFIL DE AGENTE</div>
        <div className="row">
          {(['build', 'plan'] as const).map((p) => (
            <Card key={p} accentBorder={p === agent ? 'var(--ok)' : undefined} style={{ flex: 1 }}>
              <button
                onClick={() => {
                  dispatch({ type: 'SET_AGENT_PROFILE', profile: p })
                  toast.push('success', `agente ${p} ativo — aplica à próxima mensagem enviada`)
                }}
                style={{ background: 'transparent', border: 'none', width: '100%', textAlign: 'left', color: 'var(--ink)' }}
              >
                <strong>{p}</strong>
                <div style={{ fontSize: 11, color: 'var(--muted)' }}>{p === 'build' ? 'ativo' : 'somente leitura'}</div>
              </button>
            </Card>
          ))}
        </div>

        <div style={{ fontSize: 11, color: 'var(--faint)', marginTop: 8 }}>NÍVEL DE AUTONOMIA</div>
        {/* Fase 7 Onda 13: informativo, não um seletor — `max_autonomy_level`
            (SquadTask) é ignorado ponta-a-ponta pelo orquestrador hoje (ADR
            0021); wire-lo até aqui seria fabricar um efeito que não existe.
            A autonomia real é decidida por agente via `agent_trust_scores`
            (`ProgressiveAutonomyManager`, hitl.py), não por um teto de tarefa. */}
        <div className="stack">
          {AUTONOMY_LEVELS.map((lvl) => (
            <div
              key={lvl.id}
              className="stack"
              style={{
                border: '1px solid var(--line)',
                borderRadius: 7,
                padding: '8px 10px',
                color: 'var(--faint)',
              }}
            >
              <span style={{ color: 'var(--muted)' }}>{lvl.label}</span>
              <span style={{ fontSize: 11 }}>{lvl.detail}</span>
            </div>
          ))}
        </div>
        <p style={{ fontSize: 11, color: 'var(--faint)' }}>
          não aplicado pelo orquestrador ainda — descope explícito (ADR 0021), não um controle de verdade.
        </p>

        <div style={{ fontSize: 11, color: 'var(--faint)', marginTop: 8 }}>
          janela 200k · cache on · compaction ~75% tier-gated
        </div>
      </div>
    </div>
  )
}
