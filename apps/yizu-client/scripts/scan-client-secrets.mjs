import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs'
import { resolve, relative } from 'node:path'
import { fileURLToPath } from 'node:url'

const projectRoot = resolve(fileURLToPath(new URL('..', import.meta.url)), '..')
const targetRoot = resolve(projectRoot, process.argv[2] ?? 'dist/build/mp-weixin')
const repositoryRoot = resolve(projectRoot, '../..')

function filesUnder(root) {
  if (!existsSync(root)) return []
  const output = []
  for (const entry of readdirSync(root)) {
    const absolute = resolve(root, entry)
    if (statSync(absolute).isDirectory()) output.push(...filesUnder(absolute))
    else output.push(absolute)
  }
  return output
}

function privateValues() {
  const envFile = resolve(repositoryRoot, '.env')
  if (!existsSync(envFile)) return new Map()
  const allowedNames = [
    'WECHAT_MINIPROGRAM_APP_SECRET', 'SMS_SECRET_KEY', 'BAILIAN_API_KEY',
    'ALIYUN_BAILIAN_KEY', 'ACCESS_TOKEN_SECRET', 'REFRESH_TOKEN_SECRET',
    'DATABASE_URL', 'MYSQL_URL',
  ]
  const values = new Map()
  for (const line of readFileSync(envFile, 'utf8').split(/\r?\n/u)) {
    const match = line.match(/^\s*([A-Z0-9_]+)\s*=\s*(.*?)\s*$/u)
    if (!match?.[1] || !allowedNames.includes(match[1])) continue
    const value = (match[2] ?? '').replace(/^['"]|['"]$/gu, '')
    if (value.length >= 8) values.set(match[1], value)
  }
  return values
}

const files = filesUnder(targetRoot)
const contents = files.map((file) => [file, readFileSync(file, 'utf8')])
const checks = new Map([
  ['SERVER_SECRET_VARIABLE', /(?:WECHAT_MINIPROGRAM_APP_SECRET|SMS_SECRET_KEY|BAILIAN_API_KEY|ALIYUN_BAILIAN_KEY|ACCESS_TOKEN_SECRET|REFRESH_TOKEN_SECRET)/u],
  ['DATABASE_URL', /(?:mysql|postgres(?:ql)?):\/\/[^\s"']+/iu],
  ['PRIVATE_KEY', /-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----/u],
])
for (const [name, value] of privateValues()) checks.set(`LOCAL_${name}`, value)

let failed = false
for (const [secretType, detector] of checks) {
  const hits = contents
    .filter(([, content]) => typeof detector === 'string' ? content.includes(detector) : detector.test(content))
    .map(([file]) => relative(targetRoot, file).replaceAll('\\', '/'))
  if (!hits.length) {
    console.log(`SECRET_TYPE=${secretType}`)
    console.log('FILE=-')
    console.log('RESULT=PASS')
    continue
  }
  failed = true
  for (const file of hits) {
    console.log(`SECRET_TYPE=${secretType}`)
    console.log(`FILE=${file}`)
    console.log('RESULT=FAIL')
  }
}

if (failed) process.exitCode = 1
