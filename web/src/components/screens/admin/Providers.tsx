import { useState } from 'react'
import { Card } from '../../primitives/Card'
import { ProgressBar } from '../../primitives/ProgressBar'
import { useToast } from '../../primitives/Toast'
import { PROVIDERS, RATE_LIMITS, reorderFallback, setRateLimit, toggleProvider } from '../../../api/providers'
import type { ProviderInfo, RateLimitTier } from '../../../types/domain'

export function Providers() {
  const toast = useToast()
  const [providers, setProviders] = useState<ProviderInfo[]>(PROVIDERS)
  const [limits, setLimits] = useState<RateLimitTier[]>(RATE_LIMITS)

  async function move(index: number, dir: -1 | 1) {
    const order = providers.map((p) => p.id)
    const target = index + dir
    if (target < 0 || target >= order.length) return
    ;[order[index], order[target]] = [order[target], order[index]]
    const updated = await reorderFallback(order)
    setProviders(updated)
  }

  async function handleToggle(id: string) {
    const updated = await toggleProvider(id)
    setProviders(updated)
    toast.push('success', `${id} atualizado`)
  }

  async function handleLimitChange(tier: RateLimitTier['tier'], cap: number) {
    const updated = await setRateLimit(tier, cap)
    setLimits((prev) => prev.map((r) => (r.tier === tier ? updated : r)))
  }

  return (
    <div className="stack">
      <div className="grid grid-2">
        <Card>
          <strong>Gateway LLM · ordem de fallback</strong>
          <div className="stack" style={{ marginTop: 8 }}>
            {providers.map((p, i) => (
              <div key={p.id} className="row" style={{ justifyContent: 'space-between' }}>
                <span className="row">
                  <span style={{ color: p.status === 'ativo' ? 'var(--ok)' : 'var(--muted)' }}>●</span>
                  {p.name}
                  <span style={{ fontSize: 11, color: 'var(--faint)' }}>{p.status}</span>
                </span>
                <span className="row">
                  <button onClick={() => void move(i, -1)} style={arrowBtn}>↑</button>
                  <button onClick={() => void move(i, 1)} style={arrowBtn}>↓</button>
                  <button onClick={() => void handleToggle(p.id)} style={arrowBtn}>
                    {p.status === 'ativo' ? 'desativar' : 'ativar'}
                  </button>
                </span>
              </div>
            ))}
          </div>
          <p style={{ fontSize: 11, color: 'var(--faint)', marginTop: 10 }}>🔑 keys só no Rust</p>
        </Card>

        <Card>
          <strong>Rate limiting tier-gated</strong>
          <div className="stack" style={{ marginTop: 8 }}>
            {limits.map((r) => (
              <div key={r.tier}>
                <div className="row" style={{ justifyContent: 'space-between', fontSize: 12 }}>
                  <span>{r.tier}</span>
                  <span className="mono">
                    {r.used}/{r.cap}
                  </span>
                </div>
                <ProgressBar value={r.used / r.cap} />
                <div className="row" style={{ marginTop: 4 }}>
                  <button onClick={() => void handleLimitChange(r.tier, Math.max(r.used, r.cap - 10))} style={arrowBtn}>
                    −10 cap
                  </button>
                  <button onClick={() => void handleLimitChange(r.tier, r.cap + 10)} style={arrowBtn}>
                    +10 cap
                  </button>
                </div>
              </div>
            ))}
          </div>
          <p style={{ fontSize: 11, color: 'var(--faint)', marginTop: 10 }}>
            hit de cache nunca consome vaga
          </p>
        </Card>
      </div>

      <div style={{ fontSize: 11, color: 'var(--faint)' }}>
        cache hit 41.2% · JCS RFC 8785+sha256 · paridade 50/50 · SSE
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
  padding: '2px 6px',
}
