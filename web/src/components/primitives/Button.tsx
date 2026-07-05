import type { ButtonHTMLAttributes } from 'react'

type Variant = 'primary' | 'ghost' | 'danger'

const VARIANT_STYLE: Record<Variant, React.CSSProperties> = {
  primary: {
    background: 'linear-gradient(135deg, var(--rust), var(--amber))',
    color: '#1a1205',
    border: 'none',
  },
  ghost: {
    background: 'transparent',
    color: 'var(--ink)',
    border: '1px solid var(--line)',
  },
  danger: {
    background: 'transparent',
    color: 'var(--red)',
    border: '1px solid var(--red)',
  },
}

export function Button({
  variant = 'ghost',
  style,
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: Variant }) {
  return (
    <button
      {...rest}
      style={{
        borderRadius: 8,
        padding: '7px 14px',
        fontSize: 13,
        fontWeight: 600,
        ...VARIANT_STYLE[variant],
        ...style,
      }}
    />
  )
}
