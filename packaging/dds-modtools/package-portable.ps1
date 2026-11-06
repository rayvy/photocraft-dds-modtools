param(
    [Parameter(Mandatory=$true)][string]$SourceDir,
    [Parameter(Mandatory=$true)][string]$OutputDir,
    [string]$SevenZip='C:\Program Files\7-Zip\7z.exe'
)
$ErrorActionPreference='Stop'
$repo=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..\..'))
$source=(Resolve-Path -LiteralPath $SourceDir).Path
$output=[IO.Path]::GetFullPath($OutputDir)
$name='PhotoCraft-DDS-ModTools-0.17.0-windows-x64'
$stage=Join-Path $output $name
$archive=Join-Path $output ($name+'.zip')
if ((Test-Path -LiteralPath $stage) -or (Test-Path -LiteralPath $archive)) { throw 'Output already exists; choose a new output directory.' }
if (!(Test-Path -LiteralPath $SevenZip)) { throw '7-Zip executable not found.' }
$expected=@{
    'photocraft.exe'='3D2B80145D442FC95D2B408D16EC774A9147ABD888DBD68138EFD827E694A496'
    'photocraft-cli.exe'='30501C7A8752932B16E78F59F5B951415AB0D564B9E073C7D0D3A483C2D4D262'
    'codecs\dds\texconv.exe'='786C80F0FF579E053B3C8B73D17628AAFB2FEE2F1F713F858ABEDC1677CE60CF'
}
foreach ($entry in $expected.GetEnumerator()) {
    if ((Get-FileHash -LiteralPath (Join-Path $source $entry.Key)).Hash -ne $entry.Value) { throw "Unexpected tested binary: $($entry.Key)" }
}
New-Item -ItemType Directory -Force -Path $stage | Out-Null
foreach ($file in @('photocraft.exe','photocraft-cli.exe','dxcompiler.dll','dxil.dll','LICENSE-MIT','LICENSE-APACHE','NOTICE')) {
    Copy-Item -LiteralPath (Join-Path $source $file) -Destination $stage
}
foreach ($file in @('ATTRIBUTION.md','DDS-MODTOOLS.md')) {
    Copy-Item -LiteralPath (Join-Path $repo $file) -Destination $stage
}
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'START-HERE.ru.md') -Destination $stage
foreach ($relative in @(
    'codecs\dds\texconv.exe','codecs\dds\LICENSE-DirectXTex.txt',
    'licenses\DXC-LICENSE.TXT','licenses\bcdec_rs-LICENSE-MIT.txt','licenses\bcdec-LICENSE-MIT.txt',
    'SampleDDS\rgba-mips-bc7.dds','SampleDDS\synthetic-hdr.dds'
)) {
    $target=Join-Path $stage $relative
    New-Item -ItemType Directory -Force -Path ([IO.Path]::GetDirectoryName($target)) | Out-Null
    Copy-Item -LiteralPath (Join-Path $source $relative) -Destination $target
}
Set-Content -LiteralPath (Join-Path $stage 'portable.txt') -Value ''
$files=@(Get-ChildItem -LiteralPath $stage -File -Recurse | Sort-Object FullName)
$entries=@($files | ForEach-Object {
    [ordered]@{path=[IO.Path]::GetRelativePath($stage,$_.FullName).Replace('\','/');bytes=$_.Length;sha256=(Get-FileHash -LiteralPath $_.FullName).Hash.ToLowerInvariant()}
})
$manifest=[ordered]@{
    name='PhotoCraft DDS ModTools';version='0.17.0 (dds-modto, 2026-10-29)';platform='windows-x64'
    binarySourceCommit='7094b5d6dbdf914cf09267dee7ce1f1bae36f762'
    sourceRepository='https://github.com/rayvy/photocraft-dds-modtools';files=$entries
}
$manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $stage 'BUILD-INFO.json')
$hashes=Get-ChildItem -LiteralPath $stage -File -Recurse | Sort-Object FullName | ForEach-Object {
    (Get-FileHash -LiteralPath $_.FullName).Hash.ToLowerInvariant()+'  '+[IO.Path]::GetRelativePath($stage,$_.FullName).Replace('\','/')
}
Set-Content -LiteralPath (Join-Path $stage 'SHA256SUMS.txt') -Value $hashes
Push-Location -LiteralPath $output
try {
    & $SevenZip a -tzip -mx=9 -mm=Deflate -mmt=2 $archive $name | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Archive creation failed.' }
    & $SevenZip t $archive | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Archive integrity check failed.' }
} finally { Pop-Location }
$zipHash=(Get-FileHash -LiteralPath $archive).Hash.ToLowerInvariant()
Set-Content -LiteralPath ($archive+'.sha256') -Value ($zipHash+'  '+[IO.Path]::GetFileName($archive))
[ordered]@{
    archive=$archive;archiveBytes=(Get-Item -LiteralPath $archive).Length
    unpackedBytes=(Get-ChildItem -LiteralPath $stage -File -Recurse | Measure-Object Length -Sum).Sum
    sha256=$zipHash;stage=$stage
} | ConvertTo-Json
