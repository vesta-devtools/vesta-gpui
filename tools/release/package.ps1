param([Parameter(Mandatory=$true)][string]$Candidate)
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
if ($Candidate -notmatch '^[0-9]+\.[0-9]+\.[0-9]+-rc\.[0-9]+$') { throw 'Invalid candidate name' }
& node (Join-Path $root 'tools/sdk/gpui_source.mjs') --verify $root
if ($LASTEXITCODE -ne 0) { throw 'Source verification failed' }
$sourcePath = Join-Path $root 'SOURCE.json'
$utf8 = [Text.UTF8Encoding]::new($false, $true)
$manifest = $utf8.GetString([IO.File]::ReadAllBytes($sourcePath)) | ConvertFrom-Json
if ($manifest.candidate -ne $Candidate) { throw 'Candidate differs from SOURCE.json' }
$directory = Join-Path $root ('target/distribution/' + $Candidate)
[void][IO.Directory]::CreateDirectory($directory)
$zipPath = Join-Path $directory ('vesta-gpui-' + $Candidate + '.zip')
if ([IO.File]::Exists($zipPath)) { throw 'Refusing to overwrite an existing distribution' }
Add-Type -AssemblyName System.IO.Compression
function Get-BytesSha([byte[]]$Bytes) {
    $algorithm = [Security.Cryptography.SHA256]::Create()
    try { return ([BitConverter]::ToString($algorithm.ComputeHash($Bytes))).Replace('-', '').ToLowerInvariant() }
    finally { $algorithm.Dispose() }
}
$sourceBytes = [IO.File]::ReadAllBytes($sourcePath)
$entries = @($manifest.files) + @([pscustomobject]@{path='SOURCE.json'; bytes=$sourceBytes.Length; sha256=(Get-BytesSha $sourceBytes)})
$stream = [IO.File]::Open($zipPath, [IO.FileMode]::CreateNew, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
try {
    $archive = [IO.Compression.ZipArchive]::new($stream, [IO.Compression.ZipArchiveMode]::Create, $true)
    try {
        foreach ($item in ($entries | Sort-Object path)) {
            $bytes = [IO.File]::ReadAllBytes((Join-Path $root $item.path))
            if ($bytes.Length -ne $item.bytes -or (Get-BytesSha $bytes) -ne $item.sha256) { throw ('Source changed during packaging: ' + $item.path) }
            $entry = $archive.CreateEntry($item.path, [IO.Compression.CompressionLevel]::Optimal)
            $output = $entry.Open()
            try { $output.Write($bytes, 0, $bytes.Length) } finally { $output.Dispose() }
        }
    } finally { $archive.Dispose() }
} finally { $stream.Dispose() }
$stream = [IO.File]::OpenRead($zipPath)
try {
    $archive = [IO.Compression.ZipArchive]::new($stream, [IO.Compression.ZipArchiveMode]::Read, $true)
    try {
        if ($archive.Entries.Count -ne $entries.Count) { throw 'ZIP entry count mismatch' }
        foreach ($item in $entries) {
            $entry = $archive.GetEntry($item.path)
            if ($null -eq $entry) { throw ('Missing ZIP entry: ' + $item.path) }
            $input = $entry.Open(); $memory = [IO.MemoryStream]::new()
            try { $input.CopyTo($memory); $bytes = $memory.ToArray() }
            finally { $input.Dispose(); $memory.Dispose() }
            if ($bytes.Length -ne $item.bytes -or (Get-BytesSha $bytes) -ne $item.sha256) { throw ('ZIP content mismatch: ' + $item.path) }
        }
    } finally { $archive.Dispose() }
} finally { $stream.Dispose() }
$zipSha = (Get-FileHash -LiteralPath $zipPath -Algorithm SHA256).Hash.ToLowerInvariant()
[IO.File]::WriteAllText((Join-Path $directory 'SHA256SUMS.txt'), ($zipSha + '  ' + [IO.Path]::GetFileName($zipPath) + "`n"), [Text.UTF8Encoding]::new($false))
[pscustomobject]@{status='verified_source_archive'; candidate=$Candidate; files=$entries.Count; source_sha256=(Get-BytesSha $sourceBytes); tree_sha256=$manifest.tree_sha256; zip_sha256=$zipSha; zip_bytes=([IO.FileInfo]$zipPath).Length; zip=$zipPath} | ConvertTo-Json
