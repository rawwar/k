import { defineConfig } from 'vitepress'
import container from 'markdown-it-container'
import type Token from 'markdown-it/lib/token.mjs'
import fs from 'node:fs'
import path from 'node:path'

function titleFromSlug(slug: string): string {
  return slug
    .replace(/^\d+-/, '')
    .replace(/-/g, ' ')
    .replace(/\b\w/g, c => c.toUpperCase())
}

function readTitle(filePath: string, fallback: string): string {
  if (!fs.existsSync(filePath)) return fallback
  const content = fs.readFileSync(filePath, 'utf-8')
  const match = content.match(/^title:\s*(.+)$/m)
  return match ? match[1].trim() : fallback
}

function getSubchapters(trackDir: string, chapterDir: string): { text: string; link: string }[] {
  const fullPath = path.resolve(__dirname, '..', trackDir, chapterDir)
  if (!fs.existsSync(fullPath)) return []

  return fs.readdirSync(fullPath)
    .filter(f => f.endsWith('.md') && f !== 'index.md')
    .sort()
    .map(f => {
      const slug = f.replace('.md', '')
      const title = readTitle(path.join(fullPath, f), titleFromSlug(slug))
      return { text: title, link: `/${trackDir}/${chapterDir}/${slug}` }
    })
}

function buildSidebar(trackDir: string) {
  const fullPath = path.resolve(__dirname, '..', trackDir)
  if (!fs.existsSync(fullPath)) return []

  return fs.readdirSync(fullPath)
    .filter(d => {
      const stat = fs.statSync(path.join(fullPath, d))
      return stat.isDirectory()
    })
    .sort()
    .map(chapterDir => {
      const indexPath = path.join(fullPath, chapterDir, 'index.md')
      const title = readTitle(indexPath, titleFromSlug(chapterDir))
      return {
        text: title,
        collapsed: true,
        link: `/${trackDir}/${chapterDir}/`,
        items: getSubchapters(trackDir, chapterDir)
      }
    })
}

export default defineConfig({
  title: 'Build a CLI Coding Agent',
  description: 'Learn how to build a CLI coding agent from scratch in Rust',

  head: [
    ['link', { rel: 'icon', type: 'image/svg+xml', href: '/favicon.svg' }]
  ],

  markdown: {
    theme: 'catppuccin-mocha',
    config: (md) => {
      // Register custom containers: ::: python and ::: wild
      // VitePress only handles built-in types (tip, warning, danger, info, details).
      // Custom types need explicit registration to render as styled callout boxes.
      for (const [type, defaultTitle] of [
        ['python', 'Coming from Python'],
        ['wild', 'In the Wild'],
      ] as const) {
        md.use(container, type, {
          render(tokens: Token[], idx: number) {
            if (tokens[idx].nesting === 1) {
              const title = tokens[idx].info.trim().slice(type.length).trim() || defaultTitle
              return `<div class="custom-block ${type}"><p class="custom-block-title">${title}</p>\n`
            }
            return '</div>\n'
          },
        })
      }
    },
  },

  themeConfig: {
    nav: [
      { text: 'Home', link: '/' },
      { text: 'Project Track', link: '/project/' },
      { text: 'Linear Track', link: '/linear/' }
    ],

    sidebar: {
      '/project/': buildSidebar('project'),
      '/linear/': buildSidebar('linear')
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com' }
    ],

    search: {
      provider: 'local'
    },

    outline: {
      level: [2, 3]
    }
  }
})
