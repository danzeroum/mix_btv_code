import { useState } from 'react'
import { Card } from '../../primitives/Card'
import { Badge } from '../../primitives/Badge'
import { useToast } from '../../primitives/Toast'
import { MCP_SERVERS, PERMISSION_MATRIX, SKILLS, reconnectMcp, togglePermissionCell, vetSkill } from '../../../api/skills'
import type { PermissionMatrixDecision, SkillEntry } from '../../../types/domain'

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

export function Skills() {
  const toast = useToast()
  const [skills, setSkills] = useState(SKILLS)
  const [matrix, setMatrix] = useState(PERMISSION_MATRIX)
  const [mcpServers, setMcpServers] = useState(MCP_SERVERS)
  const [reconnecting, setReconnecting] = useState<string | null>(null)

  async function handleVet(id: string, decision: SkillEntry['status']) {
    const updated = await vetSkill(id, decision)
    setSkills((prev) => prev.map((s) => (s.id === id ? updated : s)))
    toast.push('success', `${id}: ${decision}`)
  }

  async function handleToggleCell(tool: string, profile: 'build' | 'plan') {
    const updated = await togglePermissionCell(tool, profile)
    setMatrix((prev) => prev.map((r) => (r.tool === tool ? updated : r)))
  }

  async function handleReconnect(id: string) {
    setReconnecting(id)
    try {
      const updated = await reconnectMcp(id)
      setMcpServers((prev) => prev.map((s) => (s.id === id ? updated : s)))
      toast.push('success', `${id} reconectado`)
    } catch {
      toast.push('error', `falha ao reconectar ${id}`)
    } finally {
      setReconnecting(null)
    }
  }

  return (
    <div className="grid" style={{ gridTemplateColumns: '1.1fr 1fr' }}>
      <div className="stack">
        <Card>
          <strong>Skill-vetter</strong>
          <div className="stack" style={{ marginTop: 8 }}>
            {skills.map((s) => (
              <div key={s.id} className="row" style={{ justifyContent: 'space-between' }}>
                <span>
                  <strong>{s.id}</strong>
                  <div style={{ fontSize: 11, color: 'var(--faint)' }}>{s.detail}</div>
                </span>
                <span className="row">
                  <Badge color={SKILL_COLOR[s.status]}>{s.status}</Badge>
                  {s.status !== 'aprovado' && (
                    <button onClick={() => void handleVet(s.id, 'aprovado')} style={arrowBtn}>
                      aprovar
                    </button>
                  )}
                  {s.status !== 'bloqueado' && (
                    <button onClick={() => void handleVet(s.id, 'bloqueado')} style={arrowBtn}>
                      bloquear
                    </button>
                  )}
                </span>
              </div>
            ))}
          </div>
        </Card>

        <Card>
          <strong>Servidores MCP</strong>
          <div className="stack" style={{ marginTop: 8 }}>
            {mcpServers.map((s) => (
              <div key={s.id} className="row" style={{ justifyContent: 'space-between' }}>
                <span>{s.id}</span>
                <span className="row">
                  <span style={{ color: s.status === 'ok' ? 'var(--ok)' : 'var(--amber)' }}>{s.status}</span>
                  {s.status !== 'ok' && (
                    <button onClick={() => void handleReconnect(s.id)} disabled={reconnecting === s.id} style={arrowBtn}>
                      {reconnecting === s.id ? 'reconectando…' : 'reconectar'}
                    </button>
                  )}
                </span>
              </div>
            ))}
          </div>
        </Card>
      </div>

      <div className="stack">
        <Card>
          <strong>Política de permissões</strong>
          <table style={{ width: '100%', marginTop: 8, fontSize: 12, borderCollapse: 'collapse' }}>
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
                        onClick={() => void handleToggleCell(row.tool, profile)}
                        style={{ ...arrowBtn, color: DECISION_COLOR[row[profile]], borderColor: DECISION_COLOR[row[profile]] }}
                      >
                        {row[profile]}
                      </button>
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </Card>

        <Card>
          <strong>Saúde do sidecar</strong>
          <div className="row" style={{ fontSize: 13, color: 'var(--ok)', marginTop: 8 }}>
            <span className="pulse-dot" /> forge-squadd saudável · gRPC/UDS · fallback squad
          </div>
        </Card>
      </div>
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
