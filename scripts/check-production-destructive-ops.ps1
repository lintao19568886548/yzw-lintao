[CmdletBinding()]
param(
    [string]$RepositoryRoot = (Split-Path -Parent $PSScriptRoot),
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workflowPath = '.github/workflows/deploy-production.yml'
$remoteScriptPath = 'deploy/scripts/remote_deploy.sh'
$nginxPath = 'deploy/nginx/yizu-furong.org.conf'

$forbiddenRules = @(
    @{ Id = 'production-push-trigger'; Path = $workflowPath; Pattern = '(?m)^\s{2}push\s*:' },
    @{ Id = 'database-clear-flag'; Path = $workflowPath; Pattern = '(?i)--(?:clear-database|delete-data)\b' },
    @{ Id = 'database-clear-flag'; Path = $remoteScriptPath; Pattern = '(?i)--(?:clear-database|delete-data)\b' },
    @{ Id = 'legacy-delete-data-variable'; Path = $workflowPath; Pattern = 'YIZU_SPACETIMEDB_DELETE_DATA' },
    @{ Id = 'legacy-delete-data-variable'; Path = $remoteScriptPath; Pattern = 'YIZU_SPACETIMEDB_DELETE_DATA' },
    @{ Id = 'legacy-force-publish-variable'; Path = $workflowPath; Pattern = 'YIZU_SPACETIMEDB_FORCE_PUBLISH' },
    @{ Id = 'legacy-force-publish-variable'; Path = $remoteScriptPath; Pattern = 'YIZU_SPACETIMEDB_FORCE_PUBLISH' },
    @{ Id = 'arbitrary-publish-command'; Path = $workflowPath; Pattern = 'YIZU_SPACETIMEDB_PUBLISH_CMD' },
    @{ Id = 'arbitrary-publish-command'; Path = $remoteScriptPath; Pattern = 'YIZU_SPACETIMEDB_PUBLISH_CMD' },
    @{ Id = 'dynamic-shell-command'; Path = $workflowPath; Pattern = '(?i)\bbash\s+-lc\b' },
    @{ Id = 'dynamic-shell-command'; Path = $remoteScriptPath; Pattern = '(?i)\bbash\s+-lc\b' },
    @{ Id = 'database-delete-command'; Path = $workflowPath; Pattern = '(?i)\bspacetime\s+delete\b' },
    @{ Id = 'database-delete-command'; Path = $remoteScriptPath; Pattern = '(?i)\bspacetime\s+delete\b' },
    @{ Id = 'recursive-remove'; Path = $workflowPath; Pattern = '(?i)\brm\s+-[a-z]*r[a-z]*\b' },
    @{ Id = 'recursive-remove'; Path = $remoteScriptPath; Pattern = '(?i)\brm\s+-[a-z]*r[a-z]*\b' },
    @{ Id = 'container-volume-delete'; Path = $workflowPath; Pattern = '(?i)\bdocker\s+(?:compose\s+down\b[^\r\n]*\s-v\b|volume\s+rm\b)' },
    @{ Id = 'container-volume-delete'; Path = $remoteScriptPath; Pattern = '(?i)\bdocker\s+(?:compose\s+down\b[^\r\n]*\s-v\b|volume\s+rm\b)' }
)

$requiredRules = @(
    @{ Id = 'manual-production-trigger'; Path = $workflowPath; Pattern = '(?m)^\s{2}workflow_dispatch\s*:' },
    @{ Id = 'build-must-pass-before-deploy'; Path = $workflowPath; Pattern = '(?m)^\s{4}needs:\s*build\s*$' },
    @{ Id = 'protected-production-context'; Path = $workflowPath; Pattern = '(?m)^\s{4}environment:\s*production\s*$' },
    @{ Id = 'fixed-production-marker'; Path = $workflowPath; Pattern = 'YIZU_DEPLOY_ENVIRONMENT(?:=|=")production' },
    @{ Id = 'destructive-scan-in-build'; Path = $workflowPath; Pattern = 'check-production-destructive-ops\.ps1' },
    @{ Id = 'fail-closed-environment-case'; Path = $remoteScriptPath; Pattern = 'case\s+"\$DEPLOY_ENVIRONMENT"\s+in' },
    @{ Id = 'direct-production-rejection'; Path = $remoteScriptPath; Pattern = 'if\s+\[\s+"\$DEPLOY_ENVIRONMENT"\s+=\s+"production"\s+\]' },
    @{ Id = 'fixed-publish-toggle'; Path = $remoteScriptPath; Pattern = 'YIZU_SPACETIMEDB_PUBLISH_ENABLED' },
    @{ Id = 'public-publish-and-delete-map'; Path = $nginxPath; Pattern = 'map\s+"\$request_method:\$uri"\s+\$yizu_block_destructive_spacetime_operation' },
    @{ Id = 'public-database-delete-deny'; Path = $nginxPath; Pattern = 'DELETE:/v1/database/' },
    @{ Id = 'public-module-publish-deny'; Path = $nginxPath; Pattern = '/v1/database/\[\^/\]\+/publish' },
    @{ Id = 'all-public-proxies-enforce-deny'; Path = $nginxPath; Pattern = '(?s)(?:if\s*\(\$yizu_block_destructive_spacetime_operation\)\s*\{\s*return\s+403;\s*\}.*){2}' }
)

function Invoke-ProductionDestructiveScan {
    param([Parameter(Mandatory)][string]$Root)

    $contentByPath = @{}
    $missingFiles = [System.Collections.Generic.List[string]]::new()

    foreach ($relativePath in @($workflowPath, $remoteScriptPath, $nginxPath)) {
        $fullPath = Join-Path $Root $relativePath
        if (-not (Test-Path -LiteralPath $fullPath -PathType Leaf)) {
            $missingFiles.Add("missing-required-file:$relativePath")
            continue
        }
        $contentByPath[$relativePath] = Get-Content -LiteralPath $fullPath -Raw
    }

    $failures = [System.Collections.Generic.List[string]]::new()
    foreach ($failure in @(Test-ProductionDestructiveContent -ContentByPath $contentByPath)) {
        $failures.Add($failure)
    }
    foreach ($missingFile in $missingFiles) {
        $failures.Add($missingFile)
    }
    return $failures
}

function Test-ProductionDestructiveContent {
    param([Parameter(Mandatory)][hashtable]$ContentByPath)

    $failures = [System.Collections.Generic.List[string]]::new()

    foreach ($rule in $forbiddenRules) {
        if ($ContentByPath.ContainsKey($rule.Path) -and
            $ContentByPath[$rule.Path] -match $rule.Pattern) {
            $failures.Add("forbidden:$($rule.Id):$($rule.Path)")
        }
    }

    foreach ($rule in $requiredRules) {
        if (-not $ContentByPath.ContainsKey($rule.Path) -or
            $ContentByPath[$rule.Path] -notmatch $rule.Pattern) {
            $failures.Add("missing-control:$($rule.Id):$($rule.Path)")
        }
    }

    return $failures
}

function Assert-ScanFailsWith {
    param(
        [Parameter(Mandatory)][hashtable]$ContentByPath,
        [Parameter(Mandatory)][string]$ExpectedRule
    )

    $result = @(Test-ProductionDestructiveContent -ContentByPath $ContentByPath)
    if (-not ($result | Where-Object { $_ -like "*:${ExpectedRule}:*" })) {
        throw "Negative self-test did not detect rule: $ExpectedRule"
    }
}

function Invoke-SelfTest {
    $fixture = @{
        $workflowPath = @'
on:
  workflow_dispatch:
jobs:
  build:
    steps:
      - run: ./scripts/check-production-destructive-ops.ps1
  deploy:
    needs: build
    environment: production
    steps:
      - run: export YIZU_DEPLOY_ENVIRONMENT="production"
'@
        $remoteScriptPath = @'
DEPLOY_ENVIRONMENT="${YIZU_DEPLOY_ENVIRONMENT:-}"
SPACETIME_PUBLISH_ENABLED="${YIZU_SPACETIMEDB_PUBLISH_ENABLED:-false}"
case "$DEPLOY_ENVIRONMENT" in
  production | test | local) ;;
  *) exit 1 ;;
esac
if [ "$DEPLOY_ENVIRONMENT" = "production" ]; then
  exit 0
fi
'@
        $nginxPath = @'
map "$request_method:$uri" $yizu_block_destructive_spacetime_operation {
  default 0;
  "~^DELETE:/v1/database/" 1;
  "~^[A-Z]+:/v1/database/[^/]+/publish(?:/|$)" 1;
}
if ($yizu_block_destructive_spacetime_operation) { return 403; }
if ($yizu_block_destructive_spacetime_operation) { return 403; }
'@
    }

    $safeResult = @(Test-ProductionDestructiveContent -ContentByPath $fixture)
    if ($safeResult.Count -ne 0) {
        throw "Safe self-test fixture was rejected by $($safeResult.Count) rule(s)."
    }

    $fixture[$workflowPath] += "`n  push:`n"
    Assert-ScanFailsWith -ContentByPath $fixture -ExpectedRule 'production-push-trigger'

    $fixture[$workflowPath] = $fixture[$workflowPath].Replace("`n  push:`n", "`n")
    $fixture[$remoteScriptPath] += "`n# fixture only: --delete-data`n"
    Assert-ScanFailsWith -ContentByPath $fixture -ExpectedRule 'database-clear-flag'

    $fixture[$remoteScriptPath] = $fixture[$remoteScriptPath].Replace(
        "`n# fixture only: --delete-data`n",
        "`n"
    )
    $fixture[$remoteScriptPath] += "`n# fixture only: bash -lc`n"
    Assert-ScanFailsWith -ContentByPath $fixture -ExpectedRule 'dynamic-shell-command'

    Write-Output 'P0-01 negative self-tests passed.'
}

if ($SelfTest) {
    Invoke-SelfTest
    exit 0
}

$scanFailures = @(Invoke-ProductionDestructiveScan -Root $RepositoryRoot)
if ($scanFailures.Count -gt 0) {
    foreach ($failure in $scanFailures) {
        Write-Error $failure
    }
    exit 1
}

Write-Output 'Production destructive-operation scan passed.'
