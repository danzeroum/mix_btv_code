import { useEffect, useState } from 'react'
import { Card } from '../../primitives/Card'
import { Badge } from '../../primitives/Badge'
import { Button } from '../../primitives/Button'
import { Modal } from '../../primitives/Modal'
import { AsyncStatus } from '../../primitives/AsyncStatus'
import { useAsyncAction } from '../../../hooks/useAsyncAction'
import { useToast } from '../../primitives/Toast'
import { fetchSkills } from '../../../api/skills'
import { fetchMatrix, listRules, revokeRule, setRule } from '../../../api/permissions'
import type {
  AgentProfile,
  PermissionMatrixDecision,
  PermissionMatrixRow,
  PermissionRuleRecord,
  SkillEntry,
} from '../../../types/domain'

const SKILL_COLOR: Record<SkillEntry['status'], string> = {
  aprovado: 'var(--ok)',
  bloqueado: 'var(--red)',
  em_analise: 'var(--amber)',
}

const DECISION_COLOR: Record<PermissionMatrixDecision, string> = {
  allow: 'var(--ok)',
  ask: 'var(--amber)',
  deny: 'var(--red)',
}

const NEXT_DECISION: Record<PermissionMatrixDecision, PermissionMatrixDecision> = {
  allow: 'ask',
  ask: 'deny',
  deny: 'allow',
}

interface PendingCellChange {
  tool: string
  profile: AgentProfile
  from: PermissionMatrixDecision
  to: PermissionMatrixDecision
}

async function loadPermissions(): Promise<{ matrix: PermissionMatrixRow[]; rules: PermissionRuleRecord[] }> {
  const [matrix, rules] = await Promise.all([fetchMatrix(), listRules()])
  return { matrix, rules }
}

export function Skills() {
  const toast = useToast()
  const skillsState = useAsyncAction(fetchSkills)
  const permsState = useAsyncAction(loadPermissions)
  const [revokingId, setRevokingId] = useState<number | null>(null)
  const [pendingChange, setPendingChange] = useState<PendingCellChange | null>(null)
  const [confirming, setConfirming] = useState(false)

  useEffect(() => {
    void skillsState.run()
    void permsState.run()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  async function handleConfirmChange() {
    if (!pendingChange) return
    const { tool, profile, to } = pendingChange
    setConfirming(true)
    try {
      await setRule(profile, tool, to)
      await permsState.run()
      toast.push('success', `${tool} (${profile}) → ${to}`)
      setPendingChange(null)
    } catch {
      toast.push('error', `falha ao gravar regra para ${tool}`)
    } finally {
      setConfirming(false)
    }
  }

  async function handleRevoke(id: number) {
    setRevokingId(id)
    try {
      await revokeRule(id)
      await permsState.run()
      toast.push('success', 'regra revogada')
    } catch {
      toast.push('error', 'falha ao revogar regra')
    } finally {
      setRevokingId(null)
    }
  }

  return (
    <div style={{ position: 'relative', minHeight: '100%' }}>
      <div className="grid" style={{ gridTemplateColumns: '1.1fr 1fr' }}>
        <div className="stack">
          <Card>
            <div className="row" style={{ justifyContent: 'space-between' }}>
              <strong>Skill-vetter</strong>
              <button
                onClick={() => void skillsState.run()}
                disabled={skillsState.state.status === 'loading'}
                style={arrowBtn}
              >
                {skillsState.state.status === 'loading' ? 're-vetando…' : 're-vetar'}
              </button>
            </div>
            <div style={{ marginTop: 8 }}>
              <AsyncStatus state={skillsState.state} onRetry={() => void skillsState.run()}>
                {(skills) => (
                  <div className="stack">
                    {skills.map((s) => (
                      <div key={s.id} className="row" style={{ justifyContent: 'space-between' }}>
                        <span>
                          <strong>{s.id}</strong>
                          <div style={{ fontSize: 11, color: 'var(--faint)' }}>{s.detail}</div>
                        </span>
                        <Badge color={SKILL_COLOR[s.status]}>{s.status}</Badge>
                      </div>
                    ))}
                  </div>
                )}
              </AsyncStatus>
            </div>
          </Card>
        </div>

        <div className="stack">
          <Card>
            <strong>Política de permissões</strong>
            <div style={{ marginTop: 8 }}>
              <AsyncStatus state={permsState.state} onRetry={() => void permsState.run()}>
                {({ matrix, rules }) => (
                  <>
                    <table style={{ width: '100%', fontSize: 12, borderCollapse: 'collapse' }}>
                      <thead>
                        <tr>
                          <th style={{ textAlign: 'left' }}>ferramenta</th>
                          <th>build</th>
                          <th>plan</th>
                        </tr>
                      </thead>
                      <tbody>
                        {matrix.map((row) => (
                          <tr key={row.tool}>
                            <td style={{ padding: '4px 0' }}>{row.tool}</td>
                            {(['build', 'plan'] as const).map((profile) => (
                              <td key={profile} style={{ textAlign: 'center' }}>
                                <button
                                  onClick={() =>
                                    setPendingChange({
                                      tool: row.tool,
                                      profile,
                                      from: row[profile],
                                      to: NEXT_DECISION[row[profile]],
                                    })
                                  }
                                  style={{
                                    ...arrowBtn,
                                    color: DECISION_COLOR[row[profile]],
                                    borderColor: DECISION_COLOR[row[profile]],
                                  }}
                                >
                                  {row[profile]}
                                </button>
                              </td>
                            ))}
                          </tr>
                        ))}
                      </tbody>
                    </table>

                    <div style={{ marginTop: 12 }}>
                      <div style={{ fontSize: 11, color: 'var(--faint)', marginBottom: 4 }}>REGRAS ATIVAS (OVERRIDES)</div>
                      {rules.length === 0 ? (
                        <div style={{ fontSize: 11, color: 'var(--faint)' }}>nenhum override persistido</div>
                      ) : (
                        <div className="stack">
                          {rules.map((r) => (
                            <div key={r.id} className="row" style={{ justifyContent: 'space-between', fontSize: 11 }}>
                              <span className="mono">
                                {r.profile} · {r.tool}
                                {r.scope_prefix ? ` · ${r.scope_prefix}` : ''} →{' '}
                                <span style={{ color: DECISION_COLOR[r.decision] }}>{r.decision}</span>
                              </span>
                              <button
                                onClick={() => void handleRevoke(r.id)}
                                disabled={revokingId === r.id}
                                style={arrowBtn}
                              >
                                {revokingId === r.id ? 'revogando…' : 'revogar'}
                              </button>
                            </div>
                          ))}
                        </div>
                      )}
                    </div>
                  </>
                )}
              </AsyncStatus>
            </div>
          </Card>

          <Card>
            <strong>Saúde do sidecar</strong>
            <div className="row" style={{ fontSize: 13, color: 'var(--ok)', marginTop: 8 }}>
              <span className="pulse-dot" /> forge-squadd saudável · gRPC/UDS · fallback squad
            </div>
          </Card>
        </div>
      </div>

      {pendingChange && (
        <Modal width={380}>
          <div style={{ padding: 16 }}>
            <strong>Confirmar mudança de permissão</strong>
            <div
              className="mono"
              style={{
                fontSize: 12,
                background: '#0a0d12',
                border: '1px solid var(--line)',
                borderRadius: 6,
                padding: 8,
                margin: '10px 0',
              }}
            >
              ferramenta: {pendingChange.tool}
              <br />
              perfil: {pendingChange.profile}
              <br />
              escopo: qualquer (regra de matriz, sem prefixo)
              <br />
              {pendingChange.from} → {pendingChange.to}
            </div>
            <div className="row" style={{ justifyContent: 'flex-end' }}>
              <Button onClick={() => setPendingChange(null)} disabled={confirming}>
                cancelar
              </Button>
              <Button variant="primary" onClick={() => void handleConfirmChange()} disabled={confirming}>
                {confirming ? 'gravando…' : 'confirmar'}
              </Button>
            </div>
          </div>
        </Modal>
      )}
    </div>
  )
}

const arrowBtn: React.CSSProperties = {
  border: '1px solid var(--line)',
  background: 'transparent',
  color: 'var(--ink)',
  borderRadius: 5,
  fontSize: 11,
  padding: '2px 8px',
}
