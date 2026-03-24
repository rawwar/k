/**
 * Theme definitions for the reading experience.
 * Each theme provides CSS custom property values for backgrounds, text, brand colors,
 * code blocks, sidebar, and semantic colors (tip/warning/danger).
 */

export interface ThemeColors {
  id: string
  name: string
  mode: 'light' | 'dark'
  colors: {
    bg: string
    bgSoft: string
    bgMute: string
    bgAlt: string
    text1: string
    text2: string
    text3: string
    brand1: string
    brand2: string
    brand3: string
    brandSoft: string
    divider: string
    gutter: string
    sidebarBg: string
    codeBlockBg: string
    tip: string
    tipSoft: string
    warning: string
    warningSoft: string
    danger: string
    dangerSoft: string
  }
}

// ── Dark Themes ──────────────────────────────────────────────────────────────

const catppuccinMocha: ThemeColors = {
  id: 'catppuccin-mocha',
  name: 'Catppuccin Mocha',
  mode: 'dark',
  colors: {
    bg: '#1e1e2e',
    bgSoft: '#181825',
    bgMute: '#313244',
    bgAlt: '#11111b',
    text1: '#cdd6f4',
    text2: '#bac2de',
    text3: '#a6adc8',
    brand1: '#b4befe',
    brand2: '#89b4fa',
    brand3: '#74c7ec',
    brandSoft: 'rgba(180,190,254,0.14)',
    divider: '#313244',
    gutter: '#181825',
    sidebarBg: '#181825',
    codeBlockBg: '#11111b',
    tip: '#a6e3a1',
    tipSoft: 'rgba(166,227,161,0.14)',
    warning: '#f9e2af',
    warningSoft: 'rgba(249,226,175,0.14)',
    danger: '#f38ba8',
    dangerSoft: 'rgba(243,139,168,0.14)',
  },
}

const dracula: ThemeColors = {
  id: 'dracula',
  name: 'Dracula',
  mode: 'dark',
  colors: {
    bg: '#282a36',
    bgSoft: '#21222c',
    bgMute: '#44475a',
    bgAlt: '#191a21',
    text1: '#f8f8f2',
    text2: '#d4d4d4',
    text3: '#b0b0b0',
    brand1: '#bd93f9',
    brand2: '#8be9fd',
    brand3: '#ff79c6',
    brandSoft: 'rgba(189,147,249,0.14)',
    divider: '#44475a',
    gutter: '#21222c',
    sidebarBg: '#21222c',
    codeBlockBg: '#191a21',
    tip: '#50fa7b',
    tipSoft: 'rgba(80,250,123,0.14)',
    warning: '#f1fa8c',
    warningSoft: 'rgba(241,250,140,0.14)',
    danger: '#ff5555',
    dangerSoft: 'rgba(255,85,85,0.14)',
  },
}

const oneDark: ThemeColors = {
  id: 'one-dark',
  name: 'One Dark',
  mode: 'dark',
  colors: {
    bg: '#282c34',
    bgSoft: '#21252b',
    bgMute: '#3e4451',
    bgAlt: '#1b1d23',
    text1: '#abb2bf',
    text2: '#9da5b4',
    text3: '#7f848e',
    brand1: '#61afef',
    brand2: '#c678dd',
    brand3: '#56b6c2',
    brandSoft: 'rgba(97,175,239,0.14)',
    divider: '#3e4451',
    gutter: '#21252b',
    sidebarBg: '#21252b',
    codeBlockBg: '#1b1d23',
    tip: '#98c379',
    tipSoft: 'rgba(152,195,121,0.14)',
    warning: '#e5c07b',
    warningSoft: 'rgba(229,192,123,0.14)',
    danger: '#e06c75',
    dangerSoft: 'rgba(224,108,117,0.14)',
  },
}

const nord: ThemeColors = {
  id: 'nord',
  name: 'Nord',
  mode: 'dark',
  colors: {
    bg: '#2e3440',
    bgSoft: '#292e39',
    bgMute: '#3b4252',
    bgAlt: '#242933',
    text1: '#eceff4',
    text2: '#d8dee9',
    text3: '#9ba3b3',
    brand1: '#88c0d0',
    brand2: '#81a1c1',
    brand3: '#5e81ac',
    brandSoft: 'rgba(136,192,208,0.14)',
    divider: '#3b4252',
    gutter: '#292e39',
    sidebarBg: '#292e39',
    codeBlockBg: '#242933',
    tip: '#a3be8c',
    tipSoft: 'rgba(163,190,140,0.14)',
    warning: '#ebcb8b',
    warningSoft: 'rgba(235,203,139,0.14)',
    danger: '#bf616a',
    dangerSoft: 'rgba(191,97,106,0.14)',
  },
}

const tokyoNight: ThemeColors = {
  id: 'tokyo-night',
  name: 'Tokyo Night',
  mode: 'dark',
  colors: {
    bg: '#1a1b26',
    bgSoft: '#16161e',
    bgMute: '#292e42',
    bgAlt: '#13131a',
    text1: '#c0caf5',
    text2: '#a9b1d6',
    text3: '#787c99',
    brand1: '#7aa2f7',
    brand2: '#bb9af7',
    brand3: '#7dcfff',
    brandSoft: 'rgba(122,162,247,0.14)',
    divider: '#292e42',
    gutter: '#16161e',
    sidebarBg: '#16161e',
    codeBlockBg: '#13131a',
    tip: '#9ece6a',
    tipSoft: 'rgba(158,206,106,0.14)',
    warning: '#e0af68',
    warningSoft: 'rgba(224,175,104,0.14)',
    danger: '#f7768e',
    dangerSoft: 'rgba(247,118,142,0.14)',
  },
}

const gruvboxDark: ThemeColors = {
  id: 'gruvbox-dark',
  name: 'Gruvbox Dark',
  mode: 'dark',
  colors: {
    bg: '#282828',
    bgSoft: '#1d2021',
    bgMute: '#3c3836',
    bgAlt: '#1d2021',
    text1: '#ebdbb2',
    text2: '#d5c4a1',
    text3: '#a89984',
    brand1: '#fabd2f',
    brand2: '#83a598',
    brand3: '#b8bb26',
    brandSoft: 'rgba(250,189,47,0.14)',
    divider: '#3c3836',
    gutter: '#1d2021',
    sidebarBg: '#1d2021',
    codeBlockBg: '#1d2021',
    tip: '#b8bb26',
    tipSoft: 'rgba(184,187,38,0.14)',
    warning: '#fabd2f',
    warningSoft: 'rgba(250,189,47,0.14)',
    danger: '#fb4934',
    dangerSoft: 'rgba(251,73,52,0.14)',
  },
}

const solarizedDark: ThemeColors = {
  id: 'solarized-dark',
  name: 'Solarized Dark',
  mode: 'dark',
  colors: {
    bg: '#002b36',
    bgSoft: '#073642',
    bgMute: '#0a4050',
    bgAlt: '#001e27',
    text1: '#fdf6e3',
    text2: '#eee8d5',
    text3: '#93a1a1',
    brand1: '#268bd2',
    brand2: '#2aa198',
    brand3: '#6c71c4',
    brandSoft: 'rgba(38,139,210,0.14)',
    divider: '#0a4050',
    gutter: '#073642',
    sidebarBg: '#073642',
    codeBlockBg: '#001e27',
    tip: '#859900',
    tipSoft: 'rgba(133,153,0,0.14)',
    warning: '#b58900',
    warningSoft: 'rgba(181,137,0,0.14)',
    danger: '#dc322f',
    dangerSoft: 'rgba(220,50,47,0.14)',
  },
}

const materialPalenight: ThemeColors = {
  id: 'material-palenight',
  name: 'Material Palenight',
  mode: 'dark',
  colors: {
    bg: '#292d3e',
    bgSoft: '#232635',
    bgMute: '#3a3f58',
    bgAlt: '#1e2132',
    text1: '#a6accd',
    text2: '#959dcb',
    text3: '#717cb4',
    brand1: '#82aaff',
    brand2: '#c792ea',
    brand3: '#89ddff',
    brandSoft: 'rgba(130,170,255,0.14)',
    divider: '#3a3f58',
    gutter: '#232635',
    sidebarBg: '#232635',
    codeBlockBg: '#1e2132',
    tip: '#c3e88d',
    tipSoft: 'rgba(195,232,141,0.14)',
    warning: '#ffcb6b',
    warningSoft: 'rgba(255,203,107,0.14)',
    danger: '#ff5370',
    dangerSoft: 'rgba(255,83,112,0.14)',
  },
}

const githubDark: ThemeColors = {
  id: 'github-dark',
  name: 'GitHub Dark',
  mode: 'dark',
  colors: {
    bg: '#0d1117',
    bgSoft: '#161b22',
    bgMute: '#21262d',
    bgAlt: '#010409',
    text1: '#e6edf3',
    text2: '#c9d1d9',
    text3: '#8b949e',
    brand1: '#58a6ff',
    brand2: '#79c0ff',
    brand3: '#a5d6ff',
    brandSoft: 'rgba(88,166,255,0.14)',
    divider: '#21262d',
    gutter: '#161b22',
    sidebarBg: '#161b22',
    codeBlockBg: '#010409',
    tip: '#3fb950',
    tipSoft: 'rgba(63,185,80,0.14)',
    warning: '#d29922',
    warningSoft: 'rgba(210,153,34,0.14)',
    danger: '#f85149',
    dangerSoft: 'rgba(248,81,73,0.14)',
  },
}

const monokaiPro: ThemeColors = {
  id: 'monokai-pro',
  name: 'Monokai Pro',
  mode: 'dark',
  colors: {
    bg: '#2d2a2e',
    bgSoft: '#221f22',
    bgMute: '#403e41',
    bgAlt: '#1a181a',
    text1: '#fcfcfa',
    text2: '#d0d0d0',
    text3: '#939293',
    brand1: '#ffd866',
    brand2: '#ab9df2',
    brand3: '#78dce8',
    brandSoft: 'rgba(255,216,102,0.14)',
    divider: '#403e41',
    gutter: '#221f22',
    sidebarBg: '#221f22',
    codeBlockBg: '#1a181a',
    tip: '#a9dc76',
    tipSoft: 'rgba(169,220,118,0.14)',
    warning: '#ffd866',
    warningSoft: 'rgba(255,216,102,0.14)',
    danger: '#ff6188',
    dangerSoft: 'rgba(255,97,136,0.14)',
  },
}

// ── Light Themes ─────────────────────────────────────────────────────────────

const githubLight: ThemeColors = {
  id: 'github-light',
  name: 'GitHub Light',
  mode: 'light',
  colors: {
    bg: '#ffffff',
    bgSoft: '#f6f8fa',
    bgMute: '#eaeef2',
    bgAlt: '#f0f3f6',
    text1: '#1f2328',
    text2: '#3d4752',
    text3: '#656d76',
    brand1: '#0969da',
    brand2: '#0550ae',
    brand3: '#033d8b',
    brandSoft: 'rgba(9,105,218,0.10)',
    divider: '#d0d7de',
    gutter: '#f6f8fa',
    sidebarBg: '#f6f8fa',
    codeBlockBg: '#f6f8fa',
    tip: '#1a7f37',
    tipSoft: 'rgba(26,127,55,0.10)',
    warning: '#9a6700',
    warningSoft: 'rgba(154,103,0,0.10)',
    danger: '#cf222e',
    dangerSoft: 'rgba(207,34,46,0.10)',
  },
}

const solarizedLight: ThemeColors = {
  id: 'solarized-light',
  name: 'Solarized Light',
  mode: 'light',
  colors: {
    bg: '#fdf6e3',
    bgSoft: '#eee8d5',
    bgMute: '#e4dcc8',
    bgAlt: '#f5efdc',
    text1: '#073642',
    text2: '#586e75',
    text3: '#93a1a1',
    brand1: '#268bd2',
    brand2: '#2aa198',
    brand3: '#6c71c4',
    brandSoft: 'rgba(38,139,210,0.10)',
    divider: '#d6ccb0',
    gutter: '#eee8d5',
    sidebarBg: '#eee8d5',
    codeBlockBg: '#eee8d5',
    tip: '#859900',
    tipSoft: 'rgba(133,153,0,0.10)',
    warning: '#b58900',
    warningSoft: 'rgba(181,137,0,0.10)',
    danger: '#dc322f',
    dangerSoft: 'rgba(220,50,47,0.10)',
  },
}

const catppuccinLatte: ThemeColors = {
  id: 'catppuccin-latte',
  name: 'Catppuccin Latte',
  mode: 'light',
  colors: {
    bg: '#eff1f5',
    bgSoft: '#e6e9ef',
    bgMute: '#dce0e8',
    bgAlt: '#ccd0da',
    text1: '#4c4f69',
    text2: '#5c5f77',
    text3: '#7c7f93',
    brand1: '#7287fd',
    brand2: '#1e66f5',
    brand3: '#209fb5',
    brandSoft: 'rgba(114,135,253,0.10)',
    divider: '#ccd0da',
    gutter: '#e6e9ef',
    sidebarBg: '#e6e9ef',
    codeBlockBg: '#e6e9ef',
    tip: '#40a02b',
    tipSoft: 'rgba(64,160,43,0.10)',
    warning: '#df8e1d',
    warningSoft: 'rgba(223,142,29,0.10)',
    danger: '#d20f39',
    dangerSoft: 'rgba(210,15,57,0.10)',
  },
}

const oneLight: ThemeColors = {
  id: 'one-light',
  name: 'One Light',
  mode: 'light',
  colors: {
    bg: '#fafafa',
    bgSoft: '#f0f0f0',
    bgMute: '#e5e5e6',
    bgAlt: '#eaeaeb',
    text1: '#383a42',
    text2: '#4f525e',
    text3: '#a0a1a7',
    brand1: '#4078f2',
    brand2: '#a626a4',
    brand3: '#0184bc',
    brandSoft: 'rgba(64,120,242,0.10)',
    divider: '#d3d3d5',
    gutter: '#f0f0f0',
    sidebarBg: '#f0f0f0',
    codeBlockBg: '#f0f0f0',
    tip: '#50a14f',
    tipSoft: 'rgba(80,161,79,0.10)',
    warning: '#c18401',
    warningSoft: 'rgba(193,132,1,0.10)',
    danger: '#e45649',
    dangerSoft: 'rgba(228,86,73,0.10)',
  },
}

const nordLight: ThemeColors = {
  id: 'nord-light',
  name: 'Nord Light',
  mode: 'light',
  colors: {
    bg: '#eceff4',
    bgSoft: '#e5e9f0',
    bgMute: '#d8dee9',
    bgAlt: '#dfe4ed',
    text1: '#2e3440',
    text2: '#3b4252',
    text3: '#6b7994',
    brand1: '#5e81ac',
    brand2: '#81a1c1',
    brand3: '#88c0d0',
    brandSoft: 'rgba(94,129,172,0.10)',
    divider: '#c8ced9',
    gutter: '#e5e9f0',
    sidebarBg: '#e5e9f0',
    codeBlockBg: '#e5e9f0',
    tip: '#a3be8c',
    tipSoft: 'rgba(163,190,140,0.10)',
    warning: '#ebcb8b',
    warningSoft: 'rgba(235,203,139,0.15)',
    danger: '#bf616a',
    dangerSoft: 'rgba(191,97,106,0.10)',
  },
}

const gruvboxLight: ThemeColors = {
  id: 'gruvbox-light',
  name: 'Gruvbox Light',
  mode: 'light',
  colors: {
    bg: '#fbf1c7',
    bgSoft: '#f2e5bc',
    bgMute: '#ebdbb2',
    bgAlt: '#f9f0c3',
    text1: '#3c3836',
    text2: '#504945',
    text3: '#7c6f64',
    brand1: '#076678',
    brand2: '#427b58',
    brand3: '#8f3f71',
    brandSoft: 'rgba(7,102,120,0.10)',
    divider: '#d5c4a1',
    gutter: '#f2e5bc',
    sidebarBg: '#f2e5bc',
    codeBlockBg: '#f2e5bc',
    tip: '#79740e',
    tipSoft: 'rgba(121,116,14,0.10)',
    warning: '#b57614',
    warningSoft: 'rgba(181,118,20,0.10)',
    danger: '#9d0006',
    dangerSoft: 'rgba(157,0,6,0.10)',
  },
}

const rosePineDawn: ThemeColors = {
  id: 'rose-pine-dawn',
  name: 'Rosé Pine Dawn',
  mode: 'light',
  colors: {
    bg: '#faf4ed',
    bgSoft: '#f2e9e1',
    bgMute: '#e8ddd5',
    bgAlt: '#f4ede8',
    text1: '#575279',
    text2: '#6e6a86',
    text3: '#9893a5',
    brand1: '#907aa9',
    brand2: '#d7827e',
    brand3: '#286983',
    brandSoft: 'rgba(144,122,169,0.10)',
    divider: '#dfdad9',
    gutter: '#f2e9e1',
    sidebarBg: '#f2e9e1',
    codeBlockBg: '#f2e9e1',
    tip: '#56949f',
    tipSoft: 'rgba(86,148,159,0.10)',
    warning: '#ea9d34',
    warningSoft: 'rgba(234,157,52,0.10)',
    danger: '#b4637a',
    dangerSoft: 'rgba(180,99,122,0.10)',
  },
}

const materialLight: ThemeColors = {
  id: 'material-light',
  name: 'Material Light',
  mode: 'light',
  colors: {
    bg: '#fafafa',
    bgSoft: '#f0f0f0',
    bgMute: '#e7e7e8',
    bgAlt: '#eaeaeb',
    text1: '#212121',
    text2: '#424242',
    text3: '#757575',
    brand1: '#6182b8',
    brand2: '#7c4dff',
    brand3: '#39adb5',
    brandSoft: 'rgba(97,130,184,0.10)',
    divider: '#d3d3d5',
    gutter: '#f0f0f0',
    sidebarBg: '#f0f0f0',
    codeBlockBg: '#f0f0f0',
    tip: '#91b859',
    tipSoft: 'rgba(145,184,89,0.10)',
    warning: '#f6a434',
    warningSoft: 'rgba(246,164,52,0.10)',
    danger: '#e53935',
    dangerSoft: 'rgba(229,57,53,0.10)',
  },
}

const tokyoNightLight: ThemeColors = {
  id: 'tokyo-night-light',
  name: 'Tokyo Night Light',
  mode: 'light',
  colors: {
    bg: '#d5d6db',
    bgSoft: '#cbccd1',
    bgMute: '#c0c1c7',
    bgAlt: '#cbccd1',
    text1: '#343b58',
    text2: '#4c505e',
    text3: '#8990b3',
    brand1: '#34548a',
    brand2: '#5a4a78',
    brand3: '#166775',
    brandSoft: 'rgba(52,84,138,0.10)',
    divider: '#b4b5bb',
    gutter: '#cbccd1',
    sidebarBg: '#cbccd1',
    codeBlockBg: '#cbccd1',
    tip: '#485e30',
    tipSoft: 'rgba(72,94,48,0.10)',
    warning: '#8f5e15',
    warningSoft: 'rgba(143,94,21,0.10)',
    danger: '#8c4351',
    dangerSoft: 'rgba(140,67,81,0.10)',
  },
}

const ayuLight: ThemeColors = {
  id: 'ayu-light',
  name: 'Ayu Light',
  mode: 'light',
  colors: {
    bg: '#fcfcfc',
    bgSoft: '#f3f4f5',
    bgMute: '#e8e9eb',
    bgAlt: '#f0f1f2',
    text1: '#5c6166',
    text2: '#6b7078',
    text3: '#999da3',
    brand1: '#399ee6',
    brand2: '#a37acc',
    brand3: '#4cbf99',
    brandSoft: 'rgba(57,158,230,0.10)',
    divider: '#d8d9db',
    gutter: '#f3f4f5',
    sidebarBg: '#f3f4f5',
    codeBlockBg: '#f3f4f5',
    tip: '#86b300',
    tipSoft: 'rgba(134,179,0,0.10)',
    warning: '#f2ae49',
    warningSoft: 'rgba(242,174,73,0.10)',
    danger: '#f07171',
    dangerSoft: 'rgba(240,113,113,0.10)',
  },
}

// ── Exports ──────────────────────────────────────────────────────────────────

export const darkThemes: ThemeColors[] = [
  catppuccinMocha,
  dracula,
  oneDark,
  nord,
  tokyoNight,
  gruvboxDark,
  solarizedDark,
  materialPalenight,
  githubDark,
  monokaiPro,
]

export const lightThemes: ThemeColors[] = [
  githubLight,
  solarizedLight,
  catppuccinLatte,
  oneLight,
  nordLight,
  gruvboxLight,
  rosePineDawn,
  materialLight,
  tokyoNightLight,
  ayuLight,
]

export const allThemes: ThemeColors[] = [...darkThemes, ...lightThemes]

export function getThemeById(id: string): ThemeColors | undefined {
  return allThemes.find(t => t.id === id)
}

export const DEFAULT_DARK_THEME = 'catppuccin-mocha'
export const DEFAULT_LIGHT_THEME = 'github-light'
