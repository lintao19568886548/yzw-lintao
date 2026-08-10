[CmdletBinding()]
param(
    [string]$RepositoryRoot = (Split-Path -Parent $PSScriptRoot),
    [switch]$SelfTest,
    [switch]$RouteMatrixTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$expectedProductionWorkflow = '.github/workflows/deploy-production.yml'
$expectedRemoteScript = 'deploy/scripts/remote_deploy.sh'
$nginxPath = 'deploy/nginx/yizu-furong.org.conf'
$nginxMarker = 'P0-01-PRODUCTION-MANAGEMENT-ROUTE-DENY'

$forbiddenProductionPatterns = @(
    @{ Id = 'database-delete-data-mode'; Pattern = '(?i)--delete-data(?!\s*=\s*never\b)' },
    @{ Id = 'database-clear-flag'; Pattern = '(?i)--clear-database\b' },
    @{ Id = 'database-delete-command'; Pattern = '(?i)\bspacetime(?:\.exe)?\s+delete\b' },
    @{ Id = 'legacy-delete-data-variable'; Pattern = 'YIZU_SPACETIMEDB_DELETE_DATA' },
    @{ Id = 'legacy-force-publish-variable'; Pattern = 'YIZU_SPACETIMEDB_FORCE_PUBLISH' },
    @{ Id = 'arbitrary-publish-command'; Pattern = '(?i)\b[A-Z0-9_]*SPACETIMEDB[A-Z0-9_]*PUBLISH_CMD\b' },
    @{ Id = 'dynamic-eval'; Pattern = '(?im)(?:^|[;&|]\s*)eval\s+' },
    @{ Id = 'dynamic-shell-command'; Pattern = '(?i)\b(?:ba)?sh\s+-(?:c|lc)\b' },
    @{ Id = 'recursive-remove'; Pattern = '(?i)\brm\s+(?:--[^\s]+\s+)*-[a-z]*r[a-z]*f?[a-z]*\b' },
    @{ Id = 'container-volume-delete'; Pattern = '(?i)\bdocker\s+(?:compose\s+down\b[^\r\n]*\s-v\b|volume\s+rm\b)' },
    @{ Id = 'unsafe-yes-category'; Pattern = '(?i)--yes=(?:all|delete-data|break-clients)\b' },
    @{ Id = 'break-clients-flag'; Pattern = '(?i)--break-clients\b' }
)

function ConvertTo-RepositoryPath {
    param(
        [Parameter(Mandatory)][string]$Root,
        [Parameter(Mandatory)][string]$FullPath
    )

    return [System.IO.Path]::GetRelativePath($Root, $FullPath).Replace('\', '/')
}

function Get-WorkflowTriggers {
    param([Parameter(Mandatory)][string]$Content)

    $lines = @($Content -split "`r?`n")
    $triggers = [System.Collections.Generic.List[string]]::new()

    for ($index = 0; $index -lt $lines.Count; $index++) {
        $line = $lines[$index]
        $match = [regex]::Match(
            $line,
            '^(?<indent>\s*)["'']?on["'']?\s*:\s*(?<value>.*?)(?:\s+#.*)?$',
            [System.Text.RegularExpressions.RegexOptions]::IgnoreCase
        )
        if (-not $match.Success) {
            continue
        }

        $baseIndent = $match.Groups['indent'].Value.Length
        $value = $match.Groups['value'].Value.Trim()
        if ($value.Length -gt 0) {
            if ($value.StartsWith('[') -and $value.EndsWith(']')) {
                foreach ($item in $value.Trim('[', ']').Split(',')) {
                    $name = $item.Trim().Trim('"', "'").ToLowerInvariant()
                    if ($name.Length -gt 0) {
                        $triggers.Add($name)
                    }
                }
            } elseif ($value.StartsWith('{') -and $value.EndsWith('}')) {
                foreach ($item in $value.Trim('{', '}').Split(',')) {
                    $name = ($item.Split(':', 2)[0]).Trim().Trim('"', "'").ToLowerInvariant()
                    if ($name.Length -gt 0) {
                        $triggers.Add($name)
                    }
                }
            } else {
                $name = $value.Trim('"', "'").ToLowerInvariant()
                if ($name.Length -gt 0) {
                    $triggers.Add($name)
                }
            }
            break
        }

        $children = [System.Collections.Generic.List[object]]::new()
        for ($childIndex = $index + 1; $childIndex -lt $lines.Count; $childIndex++) {
            $childLine = $lines[$childIndex]
            if ($childLine -match '^\s*(?:#.*)?$') {
                continue
            }
            $childMatch = [regex]::Match($childLine, '^(?<indent>\s*)(?<body>.+)$')
            $childIndent = $childMatch.Groups['indent'].Value.Length
            if ($childIndent -le $baseIndent) {
                break
            }
            $keyMatch = [regex]::Match(
                $childMatch.Groups['body'].Value,
                '^["'']?(?<key>[A-Za-z0-9_-]+)["'']?\s*:'
            )
            if ($keyMatch.Success) {
                $children.Add([pscustomobject]@{
                    Indent = $childIndent
                    Key = $keyMatch.Groups['key'].Value.ToLowerInvariant()
                })
            }
        }

        if ($children.Count -gt 0) {
            $triggerIndent = ($children | Measure-Object -Property Indent -Minimum).Minimum
            foreach ($child in $children | Where-Object { $_.Indent -eq $triggerIndent }) {
                $triggers.Add($child.Key)
            }
        }
        break
    }

    return @($triggers | Sort-Object -Unique)
}

function Test-IsProductionWorkflow {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][string]$Content
    )

    if ($Path -eq $expectedProductionWorkflow) {
        return $true
    }
    return $Content -match '(?im)^\s*environment\s*:\s*production\s*(?:#.*)?$' -or
        $Content -match 'YIZU_DEPLOY_(?:HOST|USER|PORT)' -or
        $Content -match 'deploy/scripts/[^\s"'']+\.(?:sh|ps1)' -or
        $Content -match '(?i)\bspacetime(?:\.exe)?\s+publish\b' -or
        $Content -match '(?i)\bproduction\s+deploy(?:ment)?\b'
}

function Test-ProductionWorkflowInventory {
    param([Parameter(Mandatory)][hashtable]$WorkflowByPath)

    $failures = [System.Collections.Generic.List[string]]::new()
    $productionPaths = @(
        $WorkflowByPath.Keys |
            Where-Object { Test-IsProductionWorkflow -Path $_ -Content $WorkflowByPath[$_] } |
            Sort-Object
    )

    if (-not $WorkflowByPath.ContainsKey($expectedProductionWorkflow)) {
        $failures.Add("missing-required-file:$expectedProductionWorkflow")
    }
    foreach ($path in $productionPaths) {
        if ($path -ne $expectedProductionWorkflow) {
            $failures.Add("unexpected-production-workflow:$path")
        }
    }
    if ($productionPaths.Count -ne 1 -or $productionPaths[0] -ne $expectedProductionWorkflow) {
        $failures.Add('production-workflow-inventory-mismatch')
    }
    return $failures
}

function Test-ProductionWorkflowTriggerPolicy {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][string]$Content
    )

    $triggers = @(Get-WorkflowTriggers -Content $Content)
    if ($triggers.Count -ne 1 -or $triggers[0] -ne 'workflow_dispatch') {
        return @("production-trigger-not-manual-only:$Path")
    }
    return @()
}

function Test-ForbiddenProductionContent {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][string]$Content
    )

    $failures = [System.Collections.Generic.List[string]]::new()
    foreach ($rule in $forbiddenProductionPatterns) {
        if ($Content -match $rule.Pattern) {
            $failures.Add("forbidden:$($rule.Id):$Path")
        }
    }

    if ($Content -match '(?i)\bspacetime(?:\.exe)?[^\r\n]*\s+publish\b' -or
        $Content -match '(?i)\bpublish\s+"?\$SPACETIME_DB_NAME') {
        foreach ($yesArgument in [regex]::Matches($Content, '(?i)--yes(?:=[^\s\\]+)?')) {
            if ($yesArgument.Value -notin @('--yes=remote', '--yes=skip-login')) {
                $failures.Add("forbidden:unsupported-publish-confirmation:$Path")
            }
        }
        foreach ($required in @(
            @{ Id = 'safe-delete-data-never'; Pattern = '(?i)--delete-data=never\b' },
            @{ Id = 'minimal-yes-remote'; Pattern = '(?i)--yes=remote(?:\s|\\|$)' },
            @{ Id = 'minimal-yes-skip-login'; Pattern = '(?i)--yes=skip-login(?:\s|\\|$)' }
        )) {
            if ($Content -notmatch $required.Pattern) {
                $failures.Add("missing-control:$($required.Id):$Path")
            }
        }
    }
    return $failures
}

function Get-NginxDestructivePatterns {
    param([Parameter(Mandatory)][string]$Content)

    $map = [regex]::Match(
        $Content,
        '(?s)map\s+"\$request_method:\$uri"\s+\$yizu_block_destructive_spacetime_operation\s*\{(?<body>.*?)\}'
    )
    if (-not $map.Success) {
        return @()
    }

    return @(
        [regex]::Matches($map.Groups['body'].Value, '(?m)^\s*"~(?<pattern>[^"]+)"\s+1;\s*$') |
            ForEach-Object { $_.Groups['pattern'].Value }
    )
}

function Test-NginxRouteMatrixContent {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][string]$Content
    )

    $failures = [System.Collections.Generic.List[string]]::new()
    $patterns = @(Get-NginxDestructivePatterns -Content $Content)
    if ($patterns.Count -eq 0) {
        $failures.Add("missing-control:nginx-management-route-map:$Path")
        return $failures
    }

    $blockedRequests = @(
        'POST:/v1/database',
        'POST:/v1/database/',
        'PUT:/v1/database/prod-db',
        'PUT:/v1/database/prod-db?clear=true',
        'DELETE:/v1/database/prod-db',
        'PUT:/v1/database/prod-db/publish'
    )
    $allowedRequests = @(
        'GET:/v1/database/prod-db',
        'GET:/v1/database/prod-db/schema',
        'GET:/v1/database/prod-db/subscribe',
        'POST:/v1/database/prod-db/call/example',
        'POST:/v1/database/prod-db/sql',
        'DELETE:/v1/database/prod-db/route/resource/1',
        'POST:/api/storage/r2/delete-salary-images'
    )

    foreach ($request in $blockedRequests) {
        $nginxKey = $request.Split('?', 2)[0]
        $blocked = $patterns | Where-Object { [regex]::IsMatch($nginxKey, $_) }
        if (-not $blocked) {
            $failures.Add("route-matrix-should-block:$request")
        }
    }
    foreach ($request in $allowedRequests) {
        $nginxKey = $request.Split('?', 2)[0]
        $blocked = $patterns | Where-Object { [regex]::IsMatch($nginxKey, $_) }
        if ($blocked) {
            $failures.Add("route-matrix-should-allow:$request")
        }
    }
    return $failures
}

function Test-NginxConfigControls {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][string]$Content
    )

    $failures = [System.Collections.Generic.List[string]]::new()
    if ($Content -notmatch [regex]::Escape($nginxMarker)) {
        $failures.Add("missing-control:nginx-route-marker:$Path")
    }
    $enforcementCount = [regex]::Matches(
        $Content,
        'if\s*\(\$yizu_block_destructive_spacetime_operation\)\s*\{\s*return\s+403;\s*\}'
    ).Count
    if ($enforcementCount -lt 2) {
        $failures.Add("missing-control:all-public-proxies-enforce-deny:$Path")
    }
    foreach ($failure in @(Test-NginxRouteMatrixContent -Path $Path -Content $Content)) {
        $failures.Add($failure)
    }
    return $failures
}

function Test-RemoteNginxFailClosedControls {
    param(
        [Parameter(Mandatory)][string]$Path,
        [Parameter(Mandatory)][string]$Content
    )

    $failures = [System.Collections.Generic.List[string]]::new()
    $required = @(
        @{ Id = 'production-nginx-cannot-disable'; Pattern = 'DEPLOY_ENVIRONMENT"\s*=\s*"production"\s*\]\s*&&\s*\[\s*"\$ENABLE_NGINX"\s*!=\s*"true"' },
        @{ Id = 'nginx-config-required'; Pattern = '\[\s*!\s+-f\s+"\$NGINX_SOURCE"\s*\]' },
        @{ Id = 'nginx-command-required'; Pattern = 'command\s+-v\s+nginx' },
        @{ Id = 'nginx-supported-config-dir'; Pattern = '/etc/nginx/conf\.d' },
        @{ Id = 'nginx-supported-sites-dir'; Pattern = '/etc/nginx/sites-available' },
        @{ Id = 'nginx-syntax-required'; Pattern = '"\$NGINX_BIN"\s+-t' },
        @{ Id = 'nginx-reload-required'; Pattern = 'systemctl\s+reload\s+nginx' },
        @{ Id = 'nginx-active-required'; Pattern = 'systemctl\s+is-active\s+--quiet\s+nginx' },
        @{ Id = 'nginx-loaded-marker-required'; Pattern = '"\$NGINX_BIN"\s+-T' },
        @{ Id = 'nginx-marker-required'; Pattern = [regex]::Escape($nginxMarker) },
        @{ Id = 'nginx-single-file-backup'; Pattern = 'cp\s+-p\s+"\$NGINX_TARGET"\s+"\$NGINX_BACKUP"' },
        @{ Id = 'nginx-rollback-function'; Pattern = 'rollback_nginx_config\s*\(\)' }
    )
    foreach ($rule in $required) {
        if ($Content -notmatch $rule.Pattern) {
            $failures.Add("missing-control:$($rule.Id):$Path")
        }
    }
    if ($Content -match '(?i)systemctl\s+reload\s+nginx\s*\|\|\s*true') {
        $failures.Add("forbidden:ignored-nginx-reload:$Path")
    }

    $publishIndex = $Content.IndexOf('publish "$SPACETIME_DB_NAME"', [System.StringComparison]::Ordinal)
    $successIndex = $Content.IndexOf('Nginx 管理路由保护检查通过。', [System.StringComparison]::Ordinal)
    if ($publishIndex -lt 0 -or $successIndex -lt 0 -or $successIndex -gt $publishIndex) {
        $failures.Add("missing-control:nginx-gate-before-publish:$Path")
    }
    return $failures
}

function Invoke-ProductionDestructiveScan {
    param([Parameter(Mandatory)][string]$Root)

    $failures = [System.Collections.Generic.List[string]]::new()
    $workflowByPath = @{}
    $workflowDirectory = Join-Path $Root '.github/workflows'
    if (Test-Path -LiteralPath $workflowDirectory -PathType Container) {
        foreach ($file in Get-ChildItem -LiteralPath $workflowDirectory -File |
            Where-Object { $_.Extension -in @('.yml', '.yaml') }) {
            $path = ConvertTo-RepositoryPath -Root $Root -FullPath $file.FullName
            $workflowByPath[$path] = Get-Content -LiteralPath $file.FullName -Raw
        }
    }

    foreach ($failure in @(Test-ProductionWorkflowInventory -WorkflowByPath $workflowByPath)) {
        $failures.Add($failure)
    }

    $productionWorkflowPaths = @(
        $workflowByPath.Keys |
            Where-Object { Test-IsProductionWorkflow -Path $_ -Content $workflowByPath[$_] }
    )
    foreach ($path in $productionWorkflowPaths) {
        foreach ($failure in @(Test-ProductionWorkflowTriggerPolicy -Path $path -Content $workflowByPath[$path])) {
            $failures.Add($failure)
        }
        foreach ($failure in @(Test-ForbiddenProductionContent -Path $path -Content $workflowByPath[$path])) {
            $failures.Add($failure)
        }
    }

    $scriptByPath = @{}
    $deployDirectory = Join-Path $Root 'deploy'
    if (Test-Path -LiteralPath $deployDirectory -PathType Container) {
        foreach ($file in Get-ChildItem -LiteralPath $deployDirectory -Recurse -File |
            Where-Object { $_.Extension -in @('.sh', '.ps1') }) {
            $path = ConvertTo-RepositoryPath -Root $Root -FullPath $file.FullName
            $content = Get-Content -LiteralPath $file.FullName -Raw
            $referenced = $productionWorkflowPaths | Where-Object {
                $workflowByPath[$_] -match [regex]::Escape($path)
            }
            if ($referenced -or
                $content -match 'YIZU_DEPLOY_ENVIRONMENT' -or
                $content -match '(?i)\bspacetime(?:\.exe)?\s+publish\b' -or
                $content -match '(?i)systemctl\s+(?:restart|reload)') {
                $scriptByPath[$path] = $content
            }
        }
    }
    if (-not $scriptByPath.ContainsKey($expectedRemoteScript)) {
        $failures.Add("missing-required-file:$expectedRemoteScript")
    }
    foreach ($path in $scriptByPath.Keys) {
        foreach ($failure in @(Test-ForbiddenProductionContent -Path $path -Content $scriptByPath[$path])) {
            $failures.Add($failure)
        }
    }
    if ($scriptByPath.ContainsKey($expectedRemoteScript)) {
        foreach ($failure in @(Test-RemoteNginxFailClosedControls -Path $expectedRemoteScript -Content $scriptByPath[$expectedRemoteScript])) {
            $failures.Add($failure)
        }
    }

    $fullNginxPath = Join-Path $Root $nginxPath
    if (-not (Test-Path -LiteralPath $fullNginxPath -PathType Leaf)) {
        $failures.Add("missing-required-file:$nginxPath")
    } else {
        $nginxContent = Get-Content -LiteralPath $fullNginxPath -Raw
        foreach ($failure in @(Test-NginxConfigControls -Path $nginxPath -Content $nginxContent)) {
            $failures.Add($failure)
        }
    }

    return @($failures | Sort-Object -Unique)
}

function Assert-NoFailures {
    param(
        [Parameter(Mandatory)][AllowEmptyCollection()][object[]]$Failures,
        [Parameter(Mandatory)][string]$TestName
    )

    if ($Failures.Count -gt 0) {
        throw "$TestName failed with $($Failures.Count) finding(s)."
    }
}

function Invoke-SelfTest {
    $safeWorkflow = @'
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
      - run: deploy/scripts/remote_deploy.sh
'@
    Assert-NoFailures -Failures @(
        Test-ProductionWorkflowTriggerPolicy -Path $expectedProductionWorkflow -Content $safeWorkflow
    ) -TestName 'safe-manual-trigger'

    $unsafeTriggers = @(
        "on:`n  push:`n  workflow_dispatch:",
        "on:`n      push:`n      workflow_dispatch:",
        'on: push',
        'on: [push]',
        'on: [workflow_dispatch, push]'
    )
    foreach ($fixture in $unsafeTriggers) {
        $result = @(Test-ProductionWorkflowTriggerPolicy -Path $expectedProductionWorkflow -Content $fixture)
        if (-not ($result | Where-Object { $_ -like 'production-trigger-not-manual-only:*' })) {
            throw 'push trigger negative test was not rejected.'
        }
    }

    $workflowInventory = @{
        $expectedProductionWorkflow = $safeWorkflow
        '.github/workflows/shadow-production.yaml' = @'
on: push
jobs:
  deploy:
    environment: production
'@
    }
    $inventoryFailures = @(Test-ProductionWorkflowInventory -WorkflowByPath $workflowInventory)
    if (-not ($inventoryFailures | Where-Object { $_ -like 'unexpected-production-workflow:*' })) {
        throw 'secondary production workflow negative test was not rejected.'
    }

    $dangerousArguments = @(
        '--delete-data',
        '--delete-data=always',
        '--delete-data=on-conflict',
        '--clear-database',
        'spacetime delete example',
        'eval "$COMMAND"',
        'sh -c "$COMMAND"',
        'bash -c "$COMMAND"',
        'bash -lc "$COMMAND"',
        'rm -rf "$TARGET"',
        'docker compose down -v',
        'docker volume rm example',
        '--yes=all',
        '--yes=delete-data',
        '--yes=break-clients',
        '--break-clients'
    )
    foreach ($argument in $dangerousArguments) {
        $result = @(Test-ForbiddenProductionContent -Path $expectedRemoteScript -Content $argument)
        if ($result.Count -eq 0) {
            throw 'dangerous publish argument negative test was not rejected.'
        }
    }

    $safePublish = @'
spacetime publish "$SPACETIME_DB_NAME" \
  --delete-data=never \
  --yes=remote \
  --yes=skip-login
'@
    Assert-NoFailures -Failures @(
        Test-ForbiddenProductionContent -Path $expectedRemoteScript -Content $safePublish
    ) -TestName 'minimal-safe-publish-arguments'

    $nginxFixture = @'
# P0-01-PRODUCTION-MANAGEMENT-ROUTE-DENY
map "$request_method:$uri" $yizu_block_destructive_spacetime_operation {
  default 0;
  "~^POST:/v1/database/?$" 1;
  "~^PUT:/v1/database/[^/]+/?$" 1;
  "~^DELETE:/v1/database/[^/]+/?$" 1;
  "~^[A-Z]+:/v1/database/[^/]+/publish(?:/|$)" 1;
}
if ($yizu_block_destructive_spacetime_operation) { return 403; }
if ($yizu_block_destructive_spacetime_operation) { return 403; }
'@
    Assert-NoFailures -Failures @(
        Test-NginxConfigControls -Path $nginxPath -Content $nginxFixture
    ) -TestName 'nginx-route-matrix'

    $remoteNginxFixture = @'
if [ "$DEPLOY_ENVIRONMENT" = "production" ] && [ "$ENABLE_NGINX" != "true" ]; then exit 1; fi
if [ ! -f "$NGINX_SOURCE" ]; then exit 1; fi
if ! NGINX_BIN="$(command -v nginx)"; then exit 1; fi
if [ -d /etc/nginx/conf.d ]; then :; elif [ -d /etc/nginx/sites-available ]; then :; fi
rollback_nginx_config() { :; }
cp -p "$NGINX_TARGET" "$NGINX_BACKUP"
"$NGINX_BIN" -t
systemctl reload nginx
systemctl is-active --quiet nginx
"$NGINX_BIN" -T
grep -Fq "P0-01-PRODUCTION-MANAGEMENT-ROUTE-DENY"
echo "Nginx 管理路由保护检查通过。"
spacetime publish "$SPACETIME_DB_NAME"
'@
    Assert-NoFailures -Failures @(
        Test-RemoteNginxFailClosedControls -Path $expectedRemoteScript -Content $remoteNginxFixture
    ) -TestName 'production-nginx-fail-closed-controls'

    $ignoredReloadFixture = $remoteNginxFixture.Replace(
        'systemctl reload nginx',
        'systemctl reload nginx || true'
    )
    $ignoredReloadFailures = @(
        Test-RemoteNginxFailClosedControls -Path $expectedRemoteScript -Content $ignoredReloadFixture
    )
    if (-not ($ignoredReloadFailures | Where-Object { $_ -like 'forbidden:ignored-nginx-reload:*' })) {
        throw 'ignored Nginx reload negative test was not rejected.'
    }

    Write-Output 'P0-01 trigger, bypass, publish-argument, and route-matrix self-tests passed.'
}

if ($SelfTest) {
    Invoke-SelfTest
    exit 0
}

if ($RouteMatrixTest) {
    $fullNginxPath = Join-Path $RepositoryRoot $nginxPath
    if (-not (Test-Path -LiteralPath $fullNginxPath -PathType Leaf)) {
        Write-Error "missing-required-file:$nginxPath"
        exit 1
    }
    $routeFailures = @(
        Test-NginxRouteMatrixContent -Path $nginxPath -Content (Get-Content -LiteralPath $fullNginxPath -Raw)
    )
    if ($routeFailures.Count -gt 0) {
        foreach ($failure in $routeFailures) {
            Write-Error $failure
        }
        exit 1
    }
    Write-Output 'P0-01 Nginx route matrix passed.'
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
