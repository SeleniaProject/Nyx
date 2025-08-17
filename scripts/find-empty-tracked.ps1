[CmdletBinding()]
param(
    [string]$OutFile = "scripts/empty_tracked.json",
    [switch]$Quiet
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Split-NullTerminatedBytes {
    param(
        [byte[]]$Bytes
    )
    $list = New-Object System.Collections.Generic.List[string]
    if (-not $Bytes -or $Bytes.Length -eq 0) { return $list }
    $start = 0
    for ($i = 0; $i -lt $Bytes.Length; $i++) {
        if ($Bytes[$i] -eq 0) {
            if ($i -gt $start) {
                $count = $i - $start
                $s = [System.Text.Encoding]::UTF8.GetString($Bytes, $start, $count)
                if ($s.Length -gt 0) { [void]$list.Add($s) }
            }
            $start = $i + 1
        }
    }
    if ($start -lt $Bytes.Length) {
        $s = [System.Text.Encoding]::UTF8.GetString($Bytes, $start, $Bytes.Length - $start)
        if ($s.Length -gt 0) { [void]$list.Add($s) }
    }
    return $list
}

function Is-WhitespaceOnly {
    param([byte[]]$Bytes)
    if (-not $Bytes -or $Bytes.Length -eq 0) { return $true }
    foreach ($b in $Bytes) {
        switch ($b) {
            0x20 { continue } # space
            0x09 { continue } # tab
            0x0A { continue } # LF
            0x0D { continue } # CR
            default { return $false }
        }
    }
    return $true
}

# Run `git ls-files -z` and capture raw bytes via a temp file to preserve NUL separators
$tmp = Join-Path $env:TEMP ("git-ls-files-{0}.bin" -f ([guid]::NewGuid()))
try {
    $null = & cmd /c "git --no-pager ls-files -z > `"$tmp`""
    if (-not (Test-Path -LiteralPath $tmp)) { throw "Failed to create temp listing file: $tmp" }
    $bytes = [System.IO.File]::ReadAllBytes($tmp)
} finally {
    if (Test-Path -LiteralPath $tmp) { Remove-Item -LiteralPath $tmp -Force -ErrorAction SilentlyContinue }
}

$repoFiles = Split-NullTerminatedBytes -Bytes $bytes

$results = @()
foreach ($rel in $repoFiles) {
    if (-not $rel) { continue }
    $path = Join-Path -Path (Get-Location) -ChildPath $rel
    if (-not (Test-Path -LiteralPath $path -PathType Leaf)) { continue }
    try {
        $contentBytes = [System.IO.File]::ReadAllBytes($path)
    } catch {
        continue
    }
    if (Is-WhitespaceOnly -Bytes $contentBytes) {
        $status = if ($contentBytes.Length -eq 0) { 'empty' } else { 'whitespace-only' }
        $results += [pscustomobject]@{
            path   = $rel
            size   = $contentBytes.Length
            status = $status
        }
    }
}

# Ensure output directory exists if saving to file
if ($OutFile) {
    $outDir = Split-Path -Parent $OutFile
    if ($outDir -and -not (Test-Path -LiteralPath $outDir)) {
        New-Item -ItemType Directory -Path $outDir -Force | Out-Null
    }
    $results | ConvertTo-Json -Depth 5 | Out-File -LiteralPath $OutFile -Encoding UTF8
}

if (-not $Quiet) {
    $results | ConvertTo-Json -Depth 5
    Write-Host ("Found {0} empty/whitespace-only tracked files." -f $results.Count)
    if ($OutFile) { Write-Host ("Saved to: {0}" -f (Resolve-Path $OutFile)) }
}

exit 0
Param(
  [string[]]$Exts = @('rs','toml','md','json','cfg','yml','yaml','ps1','sh')
)
$ErrorActionPreference = 'Stop'

# 1) 追跡ファイルをNUL区切りで取得
$out = & git ls-files -z 2>$null
if ($LASTEXITCODE -ne 0) { throw 'git ls-files failed' }
# PowerShell は自動でstring化するのでそのままNUL分割
$files = $out -split "`0" | Where-Object { $_ -and $_.Length -gt 0 }

$extPattern = "\.({0})$" -f ($Exts -join '|')
$results = @()
foreach ($p in $files) {
  if ($p -notmatch $extPattern) { continue }
  try {
    $fi = Get-Item -LiteralPath $p -ErrorAction Stop
    if ($fi.Length -eq 0) {
      $results += [PSCustomObject]@{ Path=$p; Reason='size==0' }
      continue
    }
    $c = Get-Content -LiteralPath $p -Raw -ErrorAction Stop
    if ([string]::IsNullOrWhiteSpace($c)) {
      $results += [PSCustomObject]@{ Path=$p; Reason='whitespace-only' }
    }
  } catch {
    # バイナリや権限エラーは無視
  }
}
$results | Sort-Object Path | ConvertTo-Json -Depth 3
