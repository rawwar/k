import { ref, watch } from 'vue'
import { useData } from 'vitepress'

export interface ProgressData {
  visited: Record<string, number> // path -> timestamp
  lastVisited: Record<string, string> // track -> path
}

export interface ChapterProgress {
  read: number
  total: number
  subchapters: Array<{ link: string; title: string; visited: boolean }>
}

export interface TrackProgress {
  completedChapters: number
  totalChapters: number
  readSections: number
  totalSections: number
}

const STORAGE_KEY = 'k-learn:progress'

// Matches /project/CHAPTER/SUBCHAPTER or /linear/CHAPTER/SUBCHAPTER
const SUBCHAPTER_RE = /^\/learn\/(project|linear)\/([^/]+)\/([^/]+)$/

const data = ref<ProgressData>({
  visited: {},
  lastVisited: {},
})
let initialized = false

function loadFromStorage(): ProgressData {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw) {
      const parsed = JSON.parse(raw)
      return {
        visited: parsed.visited || {},
        lastVisited: parsed.lastVisited || {},
      }
    }
  } catch {}
  return { visited: {}, lastVisited: {} }
}

function saveToStorage(d: ProgressData) {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(d))
  } catch {}
}

function parseTrack(path: string): string | null {
  if (path.startsWith('/learn/project/')) return 'project'
  if (path.startsWith('/learn/linear/')) return 'linear'
  return null
}

function normalizePath(path: string): string {
  // Remove trailing .html or .md, normalize trailing slashes
  return path.replace(/\.(html|md)$/, '').replace(/\/$/, '')
}

export function useProgress() {
  if (!initialized && typeof window !== 'undefined') {
    data.value = loadFromStorage()
    initialized = true

    watch(data, (val) => {
      saveToStorage(val)
    }, { deep: true })
  }

  function markVisited(rawPath: string) {
    const path = normalizePath(rawPath)
    const match = path.match(SUBCHAPTER_RE)
    if (!match) return // only track subchapter pages

    const track = match[1]
    data.value = {
      visited: { ...data.value.visited, [path]: Date.now() },
      lastVisited: { ...data.value.lastVisited, [track]: path },
    }
  }

  function isVisited(rawPath: string): boolean {
    const path = normalizePath(rawPath)
    return path in data.value.visited
  }

  function getSidebarItems(track: string): Array<{ text: string; link?: string; items?: Array<{ text: string; link: string }> }> {
    try {
      const { theme } = useData()
      const sidebarKey = `/learn/${track}/`
      return theme.value.sidebar?.[sidebarKey] || []
    } catch {
      return []
    }
  }

  function getChapterProgress(track: string, chapterSlug: string): ChapterProgress {
    const chapters = getSidebarItems(track)
    const chapter = chapters.find(c => c.link?.includes(chapterSlug))
    if (!chapter || !chapter.items) {
      return { read: 0, total: 0, subchapters: [] }
    }

    const subchapters = chapter.items.map(item => ({
      link: item.link,
      title: item.text,
      visited: isVisited(item.link),
    }))

    return {
      read: subchapters.filter(s => s.visited).length,
      total: subchapters.length,
      subchapters,
    }
  }

  function getTrackProgress(track: string): TrackProgress {
    const chapters = getSidebarItems(track)
    let completedChapters = 0
    let readSections = 0
    let totalSections = 0

    for (const chapter of chapters) {
      const items = chapter.items || []
      totalSections += items.length
      const readInChapter = items.filter(item => isVisited(item.link)).length
      readSections += readInChapter
      if (items.length > 0 && readInChapter === items.length) {
        completedChapters++
      }
    }

    return {
      completedChapters,
      totalChapters: chapters.length,
      readSections,
      totalSections,
    }
  }

  function getLastVisited(track: string): string | null {
    return data.value.lastVisited[track] || null
  }

  function getLastVisitedTitle(track: string): string | null {
    const path = getLastVisited(track)
    if (!path) return null
    const chapters = getSidebarItems(track)
    for (const chapter of chapters) {
      const item = chapter.items?.find(i => normalizePath(i.link) === path)
      if (item) return item.text
    }
    return null
  }

  function getMostRecentPage(): { path: string; track: string; title: string } | null {
    let latest: { path: string; track: string; title: string; time: number } | null = null

    for (const track of ['project', 'linear']) {
      const path = data.value.lastVisited[track]
      if (!path) continue
      const time = data.value.visited[path] || 0
      if (!latest || time > latest.time) {
        const title = getLastVisitedTitle(track)
        if (title) {
          latest = { path, track, title, time }
        }
      }
    }

    return latest ? { path: latest.path, track: latest.track, title: latest.title } : null
  }

  function hasProgress(): boolean {
    return Object.keys(data.value.visited).length > 0
  }

  return {
    data,
    markVisited,
    isVisited,
    getChapterProgress,
    getTrackProgress,
    getLastVisited,
    getLastVisitedTitle,
    getMostRecentPage,
    hasProgress,
  }
}
