import { spawnSync } from 'node:child_process'
import { copyFile, mkdir, readdir, rm } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { checkModuleDirectory } from './check-mp-weixin-modules.mjs'
import { checkWxssDirectory } from './check-mp-weixin-wxss.mjs'

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url))
const projectRoot = path.resolve(scriptDirectory, '..')
const stagingDirectory = path.resolve(projectRoot, 'dist/.mp-weixin-staging')
const publishedDirectory = path.resolve(projectRoot, 'dist/build/mp-weixin')
const PRESERVED_LOCAL_FILES = new Set(['project.private.config.json'])

function normalized(filePath) {
  return filePath.replaceAll('\\', '/')
}

async function filesUnder(directory, root = directory) {
  const entries = await readdir(directory, { withFileTypes: true })
  const files = []
  for (const entry of entries) {
    const entryPath = path.join(directory, entry.name)
    if (entry.isDirectory()) files.push(...await filesUnder(entryPath, root))
    else if (entry.isFile()) files.push(normalized(path.relative(root, entryPath)))
  }
  return files
}

export function orderJavaScriptModules(filePaths, graph) {
  const knownFiles = new Set(filePaths)
  const javascriptFiles = filePaths.filter((filePath) => filePath.endsWith('.js')).sort()
  const visited = new Set()
  const visiting = new Set()
  const ordered = []

  function visit(filePath) {
    if (visited.has(filePath)) return
    if (visiting.has(filePath)) return
    visiting.add(filePath)
    for (const dependency of graph.get(filePath) ?? []) {
      if (knownFiles.has(dependency)) visit(dependency)
    }
    visiting.delete(filePath)
    visited.add(filePath)
    ordered.push(filePath)
  }

  for (const filePath of javascriptFiles) visit(filePath)
  return ordered
}

export function publicationPlan(filePaths, graph) {
  const files = [...new Set(filePaths.map(normalized))]
  const startupFiles = ['app.json', 'project.config.json'].filter((filePath) => files.includes(filePath))
  const javascriptFiles = orderJavaScriptModules(files, graph)
  const planned = new Set([...startupFiles, ...javascriptFiles])
  const remainingFiles = files.filter((filePath) => !planned.has(filePath)).sort()
  const appReload = files.includes('app.json') ? ['app.json'] : []
  return [...startupFiles, ...javascriptFiles, ...remainingFiles, ...appReload]
}

function buildInStagingDirectory() {
  const executable = path.resolve(
    projectRoot,
    process.platform === 'win32' ? 'node_modules/.bin/uni.cmd' : 'node_modules/.bin/uni',
  )
  const result = spawnSync(
    executable,
    ['build', '-p', 'mp-weixin', '--outDir', stagingDirectory],
    {
      cwd: projectRoot,
      env: process.env,
      shell: process.platform === 'win32',
      stdio: 'inherit',
    },
  )
  if (result.error) throw result.error
  if (result.status !== 0) throw new Error(`uni mp-weixin staging build exited with ${result.status ?? 'unknown status'}`)
}

async function copyPublishedFile(relativePath) {
  const source = path.resolve(stagingDirectory, relativePath)
  const destination = path.resolve(publishedDirectory, relativePath)
  await mkdir(path.dirname(destination), { recursive: true })
  await copyFile(source, destination)
}

async function removeStalePublishedFiles(stagingFiles) {
  const expected = new Set(stagingFiles)
  let publishedFiles = []
  try {
    publishedFiles = await filesUnder(publishedDirectory)
  } catch (error) {
    if (error?.code !== 'ENOENT') throw error
  }
  for (const relativePath of publishedFiles) {
    if (expected.has(relativePath) || PRESERVED_LOCAL_FILES.has(relativePath)) continue
    await rm(path.resolve(publishedDirectory, relativePath), { force: true })
  }
}

async function publishCompleteBuild(stagingFiles, graph) {
  await mkdir(publishedDirectory, { recursive: true })
  const plan = publicationPlan(stagingFiles, graph)
  for (const relativePath of plan) await copyPublishedFile(relativePath)
  await removeStalePublishedFiles(stagingFiles)
}

async function main() {
  await rm(stagingDirectory, { recursive: true, force: true })
  buildInStagingDirectory()

  const wxss = await checkWxssDirectory(stagingDirectory)
  if (wxss.findings.length) throw new Error(`staging WXSS check found ${wxss.findings.length} incompatible rule(s)`)
  const modules = await checkModuleDirectory(stagingDirectory)
  if (modules.missing.length) throw new Error(`staging module check found ${modules.missing.length} missing dependency(ies)`)

  const stagingFiles = await filesUnder(stagingDirectory)
  await publishCompleteBuild(stagingFiles, modules.graph)
  await rm(stagingDirectory, { recursive: true, force: true })

  const loginClosure = modules.entryClosures.get('pages/login/index.js') ?? []
  console.log(
    `微信构建已安全发布：${stagingFiles.length} 个文件，登录页依赖闭包 ${loginClosure.length} 个模块；app.json 已在依赖发布完成后触发重载。`,
  )
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : ''
if (invokedPath === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(`微信安全构建失败：${error instanceof Error ? error.message : String(error)}`)
    process.exitCode = 1
  })
}
