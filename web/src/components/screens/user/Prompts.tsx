import { useEffect, useState } from 'react'
import { Card } from '../../primitives/Card'
import { Button } from '../../primitives/Button'
import { AsyncStatus } from '../../primitives/AsyncStatus'
import { useAsyncAction } from '../../../hooks/useAsyncAction'
import { useToast } from '../../primitives/Toast'
import {
  listGenerators,
  listLibrary,
  removePrompt,
  renderPrompt,
  savePrompt,
  toggleFavorite,
  type GeneratorInfo,
  type SavedPrompt,
} from '../../../api/prompts'

async function loadPromptsScreen(): Promise<{ generators: GeneratorInfo[]; library: SavedPrompt[] }> {
  const [generators, library] = await Promise.all([listGenerators(), listLibrary()])
  return { generators, library }
}

export function Prompts() {
  const toast = useToast()
  const screenState = useAsyncAction(loadPromptsScreen)
  const [library, setLibrary] = useState<SavedPrompt[]>([])
  const [activeGenerator, setActiveGenerator] = useState<GeneratorInfo | null>(null)
  const [fieldValues, setFieldValues] = useState<Record<string, string>>({})
  const [preview, setPreview] = useState<string | null>(null)
  const [rendering, setRendering] = useState(false)
  const [saveName, setSaveName] = useState('')
  const [saveTags, setSaveTags] = useState('')
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    void screenState.run().then((result) => setLibrary(result.library))
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  function handleSelectGenerator(g: GeneratorInfo) {
    setActiveGenerator(g)
    setFieldValues(Object.fromEntries(g.fields.map((f) => [f.name, ''])))
    setPreview(null)
    setSaveName('')
    setSaveTags('')
  }

  function handleUseSaved(p: SavedPrompt, generators: GeneratorInfo[]) {
    const generator = generators.find((g) => g.name === p.generator)
    if (generator) {
      setActiveGenerator(generator)
      setFieldValues(Object.fromEntries(generator.fields.map((f) => [f.name, String(p.fields[f.name] ?? '')])))
    } else {
      // Gerador não existe mais no sidecar (renomeado/removido) — ainda
      // mostra o prompt salvo, só sem campos editáveis para re-renderizar.
      setActiveGenerator({ name: p.generator, category: '', fields: [] })
      setFieldValues({})
    }
    setPreview(p.rendered)
    setSaveName(p.name)
    setSaveTags(p.tags.join(', '))
  }

  async function handleRender() {
    if (!activeGenerator) return
    setRendering(true)
    try {
      const rendered = await renderPrompt(activeGenerator.name, fieldValues)
      setPreview(rendered)
    } catch {
      toast.push('error', 'falha ao renderizar prompt')
    } finally {
      setRendering(false)
    }
  }

  async function handleSave() {
    if (!activeGenerator || !preview || !saveName.trim()) return
    setSaving(true)
    try {
      const tags = saveTags
        .split(',')
        .map((t) => t.trim())
        .filter(Boolean)
      const entry = await savePrompt({
        name: saveName.trim(),
        generator: activeGenerator.name,
        fields: fieldValues,
        rendered: preview,
        tags,
      })
      setLibrary((prev) => [entry, ...prev])
      toast.push('success', 'salvo na biblioteca')
    } catch {
      toast.push('error', 'falha ao salvar prompt')
    } finally {
      setSaving(false)
    }
  }

  async function handleFav(id: number) {
    try {
      const { favorite } = await toggleFavorite(id)
      setLibrary((prev) => prev.map((p) => (p.id === id ? { ...p, favorite } : p)))
    } catch {
      toast.push('error', 'falha ao favoritar prompt')
    }
  }

  async function handleRemove(id: number) {
    try {
      await removePrompt(id)
      setLibrary((prev) => prev.filter((p) => p.id !== id))
      toast.push('success', 'removido')
    } catch {
      toast.push('error', 'falha ao remover prompt')
    }
  }

  return (
    <div className="grid" style={{ gridTemplateColumns: '1fr 360px' }}>
      <div className="stack">
        <div className="mono" style={{ color: 'var(--muted)' }}>
          &gt; /prompt list
        </div>
        <AsyncStatus
          state={screenState.state}
          onRetry={() => void screenState.run().then((r) => setLibrary(r.library))}
        >
          {({ generators }) => (
            <>
              {generators.map((g) => (
                <button
                  key={g.name}
                  onClick={() => handleSelectGenerator(g)}
                  className="row mono"
                  style={{
                    justifyContent: 'space-between',
                    background: activeGenerator?.name === g.name ? 'var(--panel2)' : 'var(--panel)',
                    border: '1px solid var(--line)',
                    borderLeft: '3px solid var(--teal)',
                    borderRadius: 6,
                    padding: '8px 12px',
                    color: 'var(--ink)',
                  }}
                >
                  <span>{g.name}</span>
                  <span style={{ fontSize: 11, color: 'var(--faint)' }}>{g.category}</span>
                </button>
              ))}

              <div className="mono" style={{ color: 'var(--muted)', marginTop: 12 }}>
                &gt; /prompt library
              </div>
              {library.length === 0 && (
                <div className="mono" style={{ color: 'var(--faint)', fontSize: 12 }}>
                  nenhum prompt salvo ainda.
                </div>
              )}
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
                    <button onClick={() => void handleFav(p.id)} style={linkBtn('var(--amber)')}>
                      fav
                    </button>
                    <button onClick={() => handleUseSaved(p, generators)} style={linkBtn('var(--py)')}>
                      use
                    </button>
                    <button onClick={() => void handleRemove(p.id)} style={linkBtn('var(--red)')}>
                      rm
                    </button>
                  </span>
                </div>
              ))}
            </>
          )}
        </AsyncStatus>
      </div>

      <div className="stack">
        <div className="mono" style={{ color: 'var(--muted)' }}>
          &gt; /prompt use {activeGenerator?.name ?? '—'}
        </div>

        {activeGenerator && activeGenerator.fields.length > 0 && (
          <Card>
            <div className="stack">
              {activeGenerator.fields.map((f) => (
                <label key={f.name} className="stack" style={{ gap: 2 }}>
                  <span style={{ fontSize: 11, color: 'var(--muted)' }}>
                    {f.label}
                    {f.required ? ' *' : ''}
                  </span>
                  <input
                    value={fieldValues[f.name] ?? ''}
                    onChange={(e) => setFieldValues((prev) => ({ ...prev, [f.name]: e.target.value }))}
                    placeholder={f.placeholder}
                    style={inputStyle}
                  />
                </label>
              ))}
            </div>
          </Card>
        )}

        <Button onClick={() => void handleRender()} disabled={!activeGenerator || rendering}>
          {rendering ? 'renderizando…' : 'renderizar'}
        </Button>

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
          {preview ?? 'selecione um gerador à esquerda e renderize.'}
        </pre>

        {preview && (
          <Card>
            <div className="stack">
              <input
                value={saveName}
                onChange={(e) => setSaveName(e.target.value)}
                placeholder="nome para salvar"
                style={inputStyle}
              />
              <input
                value={saveTags}
                onChange={(e) => setSaveTags(e.target.value)}
                placeholder="tags (vírgula)"
                style={inputStyle}
              />
              <div className="row">
                <Button onClick={() => void handleSave()} disabled={saving || !saveName.trim()}>
                  {saving ? 'salvando…' : 'salvar'}
                </Button>
                <Button
                  variant="ghost"
                  onClick={() => {
                    void navigator.clipboard?.writeText(preview).then(() => toast.push('success', 'copiado'))
                  }}
                >
                  copiar
                </Button>
              </div>
            </div>
          </Card>
        )}
      </div>
    </div>
  )
}

const inputStyle: React.CSSProperties = {
  background: 'transparent',
  border: '1px solid var(--line)',
  borderRadius: 6,
  color: 'var(--ink)',
  padding: '6px 8px',
  fontSize: 12,
}

function linkBtn(color: string): React.CSSProperties {
  return { background: 'none', border: 'none', color }
}
