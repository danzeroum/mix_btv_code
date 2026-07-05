import type { ReactNode } from 'react'
import { LITERAL_COLORS } from '../../styles/themes'

export function Modal({ children, width = 520 }: { children: ReactNode; width?: number }) {
  return (
    <div
      style={{
        position: 'absolute',
        inset: 0,
        background: LITERAL_COLORS.modalOverlay,
        backdropFilter: 'blur(2px)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 30,
      }}
    >
      <div
        style={{
          width,
          background: 'var(--panel2)',
          border: '1px solid var(--wire)',
          borderRadius: 14,
        }}
      >
        {children}
      </div>
    </div>
  )
}
