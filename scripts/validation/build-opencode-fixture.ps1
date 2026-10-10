param([Parameter(Mandatory = $true)][string]$Target)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '../toolchain/msvc.ps1')
Enable-MsvcEnvironment
$directory = Split-Path -Parent $Target
New-Item -ItemType Directory -Path $directory -Force | Out-Null
$source = Join-Path $directory 'opencode-fixture.cpp'
@'
#include <cstdio>
#include <cstring>
int main(int count,char** args) {
  for(int i=1;i<count;i++) {
    if(!strcmp(args[i],"--version")){puts("fixture-native");return 0;}
    if(!strcmp(args[i],"models")){
      puts("opencode/fixture-free\n{\"id\":\"fixture-free\",\"providerID\":\"opencode\",\"name\":\"Fixture free\",\"status\":\"active\",\"cost\":{\"input\":0,\"output\":0}}\nopencode/fixture-retired\n{\"id\":\"fixture-retired\",\"providerID\":\"opencode\",\"name\":\"Fixture retired\",\"status\":\"deprecated\",\"cost\":{\"input\":0,\"output\":0}}");return 0;
    }
    if(!strcmp(args[i],"run")){puts("{\"type\":\"text\",\"part\":{\"text\":\"OK\"}}");return 0;}
  }
  return 1;
}
'@ | Set-Content -LiteralPath $source -Encoding utf8
Push-Location $directory
try { & cl.exe /nologo /MT $source "/Fe:$Target"; $buildExit=$LASTEXITCODE } finally {Pop-Location}
exit $buildExit
