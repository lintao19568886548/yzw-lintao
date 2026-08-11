import { readFile, readdir } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const REQUIRE_PATTERN = /\brequire\s*\(\s*(['"])([^'"\r\n]+)\1\s*\)/g
const MODULE_EXTENSIONS = new Set(['.js', '.json'])
const LOGIN_ENTRY = 'pages/login/index.js'

function normalized(filePath) {
  return filePath.replaceAll('\\', '/')
}

export function extractRelativeRequires(source) {
  const requests = []
  for (const match of source.matchAll(REQUIRE_PATTERN)) {
    const request = match[2]
    if (request?.startsWith('./') || request?.startsWith('../')) {
      requests.push({ request, index: match.index ?? 0 })
    }
  }
  return requests
}

function requireCandidates(importer, request) {
  const cleanRequest = request.split(/[?#]/u, 1)[0] ?? request
  const target = path.posix.normalize(path.posix.join(path.posix.dirname(importer), cleanRequest))
  if (path.posix.extname(target)) return [target]
  return [target, `${target}.js`, `${target}.json`, `${target}/index.js`, `${target}/index.json`]
}

export function analyzeModuleSources(moduleSources, entryPaths = [LOGIN_ENTRY]) {
  const sources = new Map(
    [...moduleSources].map(([filePath, source]) => [normalized(filePath), source]),
  )
  const knownFiles = new Set(sources.keys())
  const graph = new Map()
  const missing = []
  let relativeRequireCount = 0

  for (const [importer, source] of sources) {
    if (path.posix.extname(importer) !== '.js') continue
    const dependencies = []
    for (const { request, index } of extractRelativeRequires(source)) {
      relativeRequireCount += 1
      const candidates = requireCandidates(importer, request)
      const resolved = candidates.find((candidate) => knownFiles.has(candidate))
      if (resolved) dependencies.push(resolved)
      else missing.push({ importer, request, index, candidates })
    }
    graph.set(importer, dependencies)
  }

  const entryClosures = new Map()
  for (const rawEntry of entryPaths) {
    const entry = normalized(rawEntry)
    if (!knownFiles.has(entry)) {
      missing.push({ importer: '<entry>', request: entry, index: 0, candidates: [entry] })
      entryClosures.set(entry, [])
      continue
    }
    const visited = new Set()
    const pending = [entry]
    while (pending.length) {
      const current = pending.pop()
      if (!current || visited.has(current)) continue
      visited.add(current)
      for (const dependency of graph.get(current) ?? []) pending.push(dependency)
    }
    entryClosures.set(entry, [...visited].sort())
  }

  return {
    fileCount: sources.size,
    jsFileCount: [...sources.keys()].filter((filePath) => filePath.endsWith('.js')).length,
    relativeRequireCount,
    missing,
    graph,
    entryClosures,
  }
}

async function moduleFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true })
  const files = []
  for (const entry of entries) {
    const entryPath = path.join(directory, entry.name)
    if (entry.isDirectory()) files.push(...await moduleFiles(entryPath))
    else if (entry.isFile() && MODULE_EXTENSIONS.has(path.extname(entry.name))) files.push(entryPath)
  }
  return files
}

export async function checkModuleDirectory(directory, entryPaths = [LOGIN_ENTRY]) {
  const sources = new Map()
  for (const filePath of await moduleFiles(directory)) {
    sources.set(normalized(path.relative(directory, filePath)), await readFile(filePath, 'utf8'))
  }
  return analyzeModuleSources(sources, entryPaths)
}

async function main() {
  const scriptDirectory = path.dirname(fileURLToPath(import.meta.url))
  const projectRoot = path.resolve(scriptDirectory, '..')
  const targetDirectory = path.resolve(projectRoot, process.argv[2] ?? 'dist/build/mp-weixin')
  const result = await checkModuleDirectory(targetDirectory)

  if (result.missing.length) {
    console.error(`微信模块依赖检查失败：发现 ${result.missing.length} 个悬空本地依赖。`)
    for (const finding of result.missing) {
      console.error(`${finding.importer} -> ${finding.request}`)
      console.error(`  已检查：${finding.candidates.join(', ')}`)
    }
    process.exitCode = 1
    return
  }

  const loginClosure = result.entryClosures.get(LOGIN_ENTRY) ?? []
  console.log(
    `微信模块依赖检查通过：扫描 ${result.jsFileCount} 个 JS、解析 ${result.relativeRequireCount} 个相对 require，登录页依赖闭包 ${loginClosure.length} 个模块。`,
  )
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : ''
if (invokedPath === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(`微信模块依赖检查无法完成：${error instanceof Error ? error.message : String(error)}`)
    process.exitCode = 1
  })
}
