# Test script for photo-engine.dll

$dllPath = Resolve-Path "target/release/photo_engine.dll"
$jsonPath = Resolve-Path "test_result.json"
$pdfPath = [System.IO.Path]::Combine((Get-Location).Path, "test_output.pdf")

$signature = @"
[DllImport("$($dllPath.Path.Replace('\', '\\'))", CallingConvention = CallingConvention.Cdecl)]
public static extern int photo_engine_generate_pdf(string reqJson, IntPtr outBuf, UIntPtr outLen);
"@

$type = Add-Type -MemberDefinition $signature -Name "PhotoEngine" -Namespace "Win32" -PassThru

# Request JSON
$req = @{
    inputJson = $jsonPath.Path
    output = $pdfPath
    photosPerPage = 3
    quality = "medium"
} | ConvertTo-Json -Compress

# Buffer (64KB)
$bufSize = 64 * 1024
$bufPtr = [System.Runtime.InteropServices.Marshal]::AllocHGlobal($bufSize)

try {
    Write-Host "Calling DLL: photo_engine_generate_pdf..."
    $ret = $type::photo_engine_generate_pdf($req, $bufPtr, [UIntPtr]$bufSize)

    if ($ret -lt 0) {
        $needed = -$ret
        Write-Host "Buffer too small, need $needed bytes. Retrying..."
        [System.Runtime.InteropServices.Marshal]::FreeHGlobal($bufPtr)
        $bufPtr = [System.Runtime.InteropServices.Marshal]::AllocHGlobal($needed)
        $ret = $type::photo_engine_generate_pdf($req, $bufPtr, [UIntPtr]$needed)
    }

    if ($ret -gt 0) {
        $respJson = [System.Runtime.InteropServices.Marshal]::PtrToStringAnsi($bufPtr, $ret)
        Write-Host "Response: $respJson"
        
        $resp = $respJson | ConvertFrom-Json
        if ($resp.error) {
            Write-Host "DLL Error: $($resp.error)" -ForegroundColor Red
        } else {
            Write-Host "Success! Output: $($resp.outputPath)" -ForegroundColor Green
            if (Test-Path $resp.outputPath) {
                Write-Host "Verified: PDF file exists."
            } else {
                Write-Host "Error: PDF file not found." -ForegroundColor Red
            }
        }
    } else {
        Write-Host "DLL returned 0 (failed)." -ForegroundColor Red
    }
} catch {
    Write-Error $_
} finally {
    [System.Runtime.InteropServices.Marshal]::FreeHGlobal($bufPtr)
}
