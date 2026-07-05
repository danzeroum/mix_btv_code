import { useState } from 'react'
import { Card } from '../../primitives/Card'
import { useToast } from '../../primitives/Toast'
import { useAppDispatch, useAppState } from '../../../state/AppContext'
import { AUTONOMY_LEVELS, MODEL_TIERS, selectAgentProfile, selectAutonomy, selectTier } from '../../../api/models'
import type { AutonomyLevel } from '../../../types/domain'

export function Modelo() {
  const toast = useToast()
  const dispatch = useAppDispatch()
  const { modelTier: tier, agentProfile: agent } = useAppState()
  const [autonomy, setAutonomy] = useState<AutonomyLevel>('interativo')

  return (
    <div className="grid grid-2">
      <div className="stack">
        <div style={{ fontSize: 11, color: 'var(--faint)' }}>MODEL TIER</div>
        {MODEL_TIERS.map((t) => (
          <Card key={t.id} accentBorder={t.id === tier ? 'var(--rust)' : undefined}>
            <button
              onClick={() => {
                dispatch({ type: 'SET_MODEL_TIER', tier: t.id })
                void selectTier(t.id).then(() => toast.push('success', `tier ${t.id} selecionado`))
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
                  void selectAgentProfile(p).then(() => toast.push('success', `agente ${p} ativo`))
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
        <div className="stack">
          {AUTONOMY_LEVELS.map((lvl) => (
            <button
              key={lvl.id}
              disabled={!lvl.enabled}
              onClick={() => {
                setAutonomy(lvl.id)
                void selectAutonomy(lvl.id).then(() => toast.push('success', `autonomia: ${lvl.label}`))
              }}
              className="row"
              style={{
                justifyContent: 'space-between',
                background: 'transparent',
                border: '1px solid var(--line)',
                borderRadius: 7,
                padding: '8px 10px',
                color: lvl.enabled ? 'var(--ink)' : 'var(--faint)',
                opacity: lvl.enabled ? 1 : 0.6,
              }}
            >
              <span>{lvl.label}</span>
              {lvl.id === autonomy && <span style={{ color: 'var(--rust)' }}>●</span>}
            </button>
          ))}
        </div>

        <div style={{ fontSize: 11, color: 'var(--faint)', marginTop: 8 }}>
          janela 200k · cache on · compaction ~75% tier-gated
        </div>
      </div>
    </div>
  )
}
