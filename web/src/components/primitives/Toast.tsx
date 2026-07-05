import { createContext, useCallback, useContext, useRef, useState, type ReactNode } from 'react'

interface ToastItem {
  id: number
  kind: 'success' | 'error'
  message: string
}

interface ToastContextValue {
  push: (kind: ToastItem['kind'], message: string) => void
}

const ToastContext = createContext<ToastContextValue | null>(null)

export function ToastProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<ToastItem[]>([])
  const idRef = useRef(0)

  const push = useCallback((kind: ToastItem['kind'], message: string) => {
    const id = ++idRef.current
    setItems((prev) => [...prev, { id, kind, message }])
    setTimeout(() => {
      setItems((prev) => prev.filter((i) => i.id !== id))
    }, 4000)
  }, [])

  return (
    <ToastContext.Provider value={{ push }}>
      {children}
      <div style={{ position: 'fixed', bottom: 16, right: 16, display: 'flex', flexDirection: 'column', gap: 8, zIndex: 50 }}>
        {items.map((item) => (
          <div
            key={item.id}
            style={{
              background: 'var(--panel2)',
              border: `1px solid ${item.kind === 'error' ? 'var(--red)' : 'var(--ok)'}`,
              color: 'var(--ink)',
              borderRadius: 8,
              padding: '8px 14px',
              fontSize: 13,
              minWidth: 220,
            }}
          >
            {item.kind === 'error' ? '✗ ' : '✓ '}
            {item.message}
          </div>
        ))}
      </div>
    </ToastContext.Provider>
  )
}

export function useToast(): ToastContextValue {
  const ctx = useContext(ToastContext)
  if (!ctx) throw new Error('useToast deve ser usado dentro de <ToastProvider>')
  return ctx
}
