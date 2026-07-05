/** 5 temas renascentistas + 7 amostras de cor de destaque.
 * Valores hex copiados verbatim de docs/design_handoff_forge_telas/README.md §8.3 — não re-derivar.
 * Aplicados via CSS custom properties em #forge-root (ver state/useTheme.ts), nunca via classes.
 */

export type ThemeId = 'default' | 'veneziana' | 'ultramarino' | 'marmore' | 'afresco'

export interface ThemePalette {
  bg: string
  bg2: string
  surf: string
  term: string
  panel: string
  panel2: string
  line: string
  line2: string
  ink: string
  muted: string
  faint: string
  rust: string
  amber: string
  teal: string
  py: string
  wire: string
  ok: string
  red: string
}

export const THEMES: Record<ThemeId, ThemePalette> = {
  default: {
    bg: '#07090d', bg2: '#0b0e13', surf: '#0f1115', term: '#07090d', panel: '#12151c', panel2: '#171b24',
    line: '#242b37', line2: '#2e3644', ink: '#e8ecf3', muted: '#8b95a7', faint: '#5b6474',
    rust: '#f2683c', amber: '#f0a13c', teal: '#2fb8a0', py: '#4d9fff', wire: '#a78bfa', ok: '#43c463', red: '#f2544f',
  },
  veneziana: {
    bg: '#140b0a', bg2: '#1a0e0b', surf: '#1c110e', term: '#170b09', panel: '#231512', panel2: '#2b1a15',
    line: '#3a241d', line2: '#4a2f25', ink: '#f3e7dc', muted: '#c0a189', faint: '#8a6b57',
    rust: '#c8452f', amber: '#d99a2b', teal: '#4f7a52', py: '#7f88ad', wire: '#9b6bb0', ok: '#6b8f4e', red: '#c0392b',
  },
  ultramarino: {
    bg: '#080d1c', bg2: '#0a1122', surf: '#0d1428', term: '#060a18', panel: '#111a33', panel2: '#16203f',
    line: '#22304f', line2: '#2d3d60', ink: '#e6ecf7', muted: '#93a3c4', faint: '#5d6d90',
    rust: '#3f6fd6', amber: '#d8a63a', teal: '#3aa0a6', py: '#5b8def', wire: '#8f7fd6', ok: '#4fae7a', red: '#d65a52',
  },
  marmore: {
    bg: '#e9e3d5', bg2: '#e0d8c6', surf: '#f4efe4', term: '#efe9dd', panel: '#f0eadd', panel2: '#e7e0cf',
    line: '#d6ccb8', line2: '#c7bca2', ink: '#2b251c', muted: '#6f6553', faint: '#9a8f78',
    rust: '#b0532f', amber: '#b3852f', teal: '#4c7550', py: '#3c6ac9', wire: '#7c5896', ok: '#5c7a44', red: '#b0442c',
  },
  afresco: {
    bg: '#efe4cf', bg2: '#e8dcc2', surf: '#f6ecd9', term: '#f3ecdb', panel: '#f2e9d6', panel2: '#ebe0c9',
    line: '#ddceae', line2: '#cdbc98', ink: '#31271b', muted: '#786a52', faint: '#a5967a',
    rust: '#bd5329', amber: '#c2922b', teal: '#57794a', py: '#48699f', wire: '#875f92', ok: '#657f45', red: '#b0402a',
  },
}

export const THEME_LIST: { key: ThemeId; label: string; dark: boolean }[] = [
  { key: 'default', label: 'Forge', dark: true },
  { key: 'veneziana', label: 'Veneziana', dark: true },
  { key: 'ultramarino', label: 'Ultramarino', dark: true },
  { key: 'marmore', label: 'Mármore', dark: false },
  { key: 'afresco', label: 'Afresco', dark: false },
]

export interface AccentSwatch {
  color: string | null
  label: string
}

/** Sobrepõem só `--rust`; primeira entrada (null) = "do tema". */
export const ACCENTS: AccentSwatch[] = [
  { color: null, label: 'do tema' },
  { color: '#3f6fd6', label: 'ultramarino' },
  { color: '#24408f', label: 'lápis-lazúli' },
  { color: '#b1372f', label: 'vermelho veneziano' },
  { color: '#c8972f', label: 'ouro de Ticiano' },
  { color: '#3f6b4f', label: 'verde' },
  { color: '#d8cdb4', label: 'mármore' },
]

/** Literais intencionais que NÃO trocam com tema (README §8.3). */
export const LITERAL_COLORS = {
  codeBlockBg: '#0a0d12',
  trafficLights: ['#ff5f56', '#ffbd2e', '#27c93f'] as const,
  modalOverlay: '#05070aa8',
}

export const THEME_STORAGE_KEY = 'forge_theme'
export const ACCENT_STORAGE_KEY = 'forge_accent'
