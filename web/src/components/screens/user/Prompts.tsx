import { useEffect, useState } from 'react'
import { useToast } from '../../primitives/Toast'
import {
  GENERATORS,
  listLibrary,
  removePrompt,
  renderPrompt,
  savePrompt,
  toggleFavorite,
} from '../../../api/prompts'
import type { SavedPrompt } from '../../../types/domain'

export function Prompts() {
  const toast = useToast()
  const [library, setLibrary] = useState<SavedPrompt[]>([])
  const [preview, setPreview] = useState<string | null>(null)
  const [activeGenerator, setActiveGenerator] = useState<string | null>(null)

  useEffect(() => {
    void listLibrary().then(setLibrary)
  }, [])

  async function handleUseGenerator(id: string) {
    setActiveGenerator(id)
    try {
      const rendered = await renderPrompt(id)
      setPreview(rendered)
    } catch {
      toast.push('error', 'falha ao renderizar prompt')
    }
  }

  async function handleSave() {
    if (!activeGenerator) return
    try {
      const entry = await savePrompt(`${activeGenerator} salvo`, activeGenerator)
      setLibrary((prev) => [...prev, entry])
      toast.push('success', 'salvo na biblioteca')
    } catch {
      toast.push('error', 'falha ao salvar prompt')
    }
  }

  async function handleFav(id: string) {
    const updated = await toggleFavorite(id)
    setLibrary((prev) => prev.map((p) => (p.id === id ? updated : p)))
  }

  async function handleRemove(id: string) {
    await removePrompt(id)
    setLibrary((prev) => prev.filter((p) => p.id !== id))
    toast.push('success', 'removido')
  }

  return (
    <div className="grid" style={{ gridTemplateColumns: '1fr 340px' }}>
      <div className="stack">
        <div className="mono" style={{ color: 'var(--muted)' }}>
          &gt; /prompt list
        </div>
        {GENERATORS.map((g) => (
          <button
            key={g.id}
            onClick={() => void handleUseGenerator(g.id)}
            className="row mono"
            style={{
              justifyContent: 'space-between',
              background: 'var(--panel)',
              border: '1px solid var(--line)',
              borderLeft: '3px solid var(--teal)',
              borderRadius: 6,
              padding: '8px 12px',
              color: 'var(--ink)',
            }}
          >
            {g.name}
          </button>
        ))}

        <div className="mono" style={{ color: 'var(--muted)', marginTop: 12 }}>
          &gt; /prompt library
        </div>
        {library.map((p) => (
          <div
            key={p.id}
            className="row mono"
            style={{
              justifyContent: 'space-between',
              background: 'var(--panel)',
              border: '1px solid var(--line)',
              borderLeft: '3px solid var(--wire)',
              borderRadius: 6,
              padding: '8px 12px',
            }}
          >
            <span>
              #{p.id} {p.name} {p.favorite ? '★' : '☆'} [{p.generator}] {p.tags.join(',')}
            </span>
            <span className="row">
              <button onClick={() => void handleFav(p.id)} style={{ background: 'none', border: 'none', color: 'var(--amber)' }}>
                fav
              </button>
              <button onClick={() => void handleUseGenerator(p.generator)} style={{ background: 'none', border: 'none', color: 'var(--py)' }}>
                use
              </button>
              <button onClick={() => void handleRemove(p.id)} style={{ background: 'none', border: 'none', color: 'var(--red)' }}>
                rm
              </button>
            </span>
          </div>
        ))}
      </div>

      <div className="stack">
        <div className="mono" style={{ color: 'var(--muted)' }}>
          &gt; /prompt use {activeGenerator ?? '—'} ★
        </div>
        <pre
          className="mono"
          style={{
            background: '#0a0d12',
            border: '1px solid var(--line)',
            borderRadius: 8,
            padding: 12,
            fontSize: 12,
            minHeight: 120,
            whiteSpace: 'pre-wrap',
          }}
        >
          {preview ?? 'selecione um gerador à esquerda'}
        </pre>
        <div className="row">
          <button onClick={handleSave} style={chip}>
            save
          </button>
          <button onClick={() => activeGenerator && void handleFav(activeGenerator)} style={chip}>
            fav ★
          </button>
          <button
            onClick={() => {
              if (preview) void navigator.clipboard?.writeText(preview).then(() => toast.push('success', 'copiado'))
            }}
            style={chip}
          >
            copiar
          </button>
        </div>
      </div>
    </div>
  )
}

const chip: React.CSSProperties = {
  border: '1px solid var(--line)',
  background: 'transparent',
  color: 'var(--ink)',
  borderRadius: 6,
  padding: '4px 10px',
  fontSize: 12,
}
