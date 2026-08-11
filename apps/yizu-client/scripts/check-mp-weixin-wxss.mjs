import { readFile, readdir } from 'node:fs/promises'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const BLOCKED_SELECTOR_RULES = [
  {
    code: 'universal-selector',
    message: '微信 WXSS 不接受裸通配选择器',
    test: hasUniversalSelector,
  },
  {
    code: 'browser-root-selector',
    message: '微信 WXSS 不应包含 html/body 根选择器',
    test: (selector) => /(^|[\s,>+~])(html|body)(?=$|[\s,>+~.#:[\]])/i.test(stripAttributeSelectors(selector)),
  },
  {
    code: 'browser-only-pseudo',
    message: '微信 WXSS 不应包含浏览器专用伪类/伪元素',
    test: (selector) => /:root\b|:focus-visible\b|:has\(|::-webkit-/i.test(selector),
  },
]

function stripComments(source) {
  return source.replace(/\/\*[\s\S]*?\*\//g, (comment) => comment.replace(/[^\n]/g, ' '))
}

function stripAttributeSelectors(selector) {
  return selector.replace(/\[(?:\\.|[^\]"']|"(?:\\.|[^"])*"|'(?:\\.|[^'])*')*\]/g, '[]')
}

function hasUniversalSelector(selector) {
  const clean = stripAttributeSelectors(selector)
    .replace(/"(?:\\.|[^"])*"|'(?:\\.|[^'])*'/g, '')
  return /(^|[\s,>+~(])\*(?=$|[\s,>+~.#:[\]),])/.test(clean)
}

function selectorBlocks(source) {
  const clean = stripComments(source)
  const blocks = []
  let boundary = 0

  for (let index = 0; index < clean.length; index += 1) {
    const character = clean[index]
    if (character === '{') {
      const selector = clean.slice(boundary, index).trim()
      if (selector && !selector.startsWith('@')) {
        const offset = clean.slice(boundary, index).search(/\S/)
        blocks.push({ selector, index: boundary + Math.max(offset, 0) })
      }
      boundary = index + 1
    } else if (character === '}' || character === ';') {
      boundary = index + 1
    }
  }

  return blocks
}

function lineAndColumn(source, index) {
  const before = source.slice(0, index)
  const lines = before.split('\n')
  return { line: lines.length, column: (lines.at(-1)?.length ?? 0) + 1 }
}

export function findIncompatibleWxss(filePath, source) {
  const findings = []
  const clean = stripComments(source)

  for (const match of clean.matchAll(/@supports\b/gi)) {
    const location = lineAndColumn(clean, match.index ?? 0)
    findings.push({
      filePath,
      ...location,
      code: 'supports-at-rule',
      selector: '@supports',
      message: '微信 WXSS 不应包含 @supports',
    })
  }

  for (const block of selectorBlocks(source)) {
    for (const rule of BLOCKED_SELECTOR_RULES) {
      if (!rule.test(block.selector)) continue
      const location = lineAndColumn(source, block.index)
      findings.push({
        filePath,
        ...location,
        code: rule.code,
        selector: block.selector.replace(/\s+/g, ' ').trim(),
        message: rule.message,
      })
    }
  }

  return findings
}

async function wxssFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true })
  const files = []
  for (const entry of entries) {
    const entryPath = path.join(directory, entry.name)
    if (entry.isDirectory()) files.push(...await wxssFiles(entryPath))
    else if (entry.isFile() && entry.name.endsWith('.wxss')) files.push(entryPath)
  }
  return files
}

export async function checkWxssDirectory(directory) {
  const files = await wxssFiles(directory)
  const findings = []
  for (const filePath of files) {
    findings.push(...findIncompatibleWxss(filePath, await readFile(filePath, 'utf8')))
  }
  return { files, findings }
}

async function main() {
  const scriptDirectory = path.dirname(fileURLToPath(import.meta.url))
  const projectRoot = path.resolve(scriptDirectory, '..')
  const targetDirectory = path.resolve(projectRoot, process.argv[2] ?? 'dist/build/mp-weixin')
  const { files, findings } = await checkWxssDirectory(targetDirectory)

  if (findings.length) {
    console.error(`WXSS 兼容检查失败：共 ${findings.length} 处不兼容规则。`)
    for (const finding of findings) {
      console.error(`${path.relative(projectRoot, finding.filePath)}:${finding.line}:${finding.column} [${finding.code}] ${finding.message}`)
      console.error(`  ${finding.selector}`)
    }
    process.exitCode = 1
    return
  }

  console.log(`WXSS 兼容检查通过：已扫描 ${files.length} 个文件。`)
}

const invokedPath = process.argv[1] ? path.resolve(process.argv[1]) : ''
if (invokedPath === fileURLToPath(import.meta.url)) {
  main().catch((error) => {
    console.error(`WXSS 兼容检查无法完成：${error instanceof Error ? error.message : String(error)}`)
    process.exitCode = 1
  })
}
