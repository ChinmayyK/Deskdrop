$ErrorActionPreference = "Stop"

Write-Host "Deskdrop IPC Smoke Test" -ForegroundColor Cyan
Write-Host "-----------------------"

$pipeName = "DeskdropIPC"
$pipePath = "\\.\pipe\$pipeName"

if (!(Test-Path $pipePath)) {
    Write-Host "Pipe $pipePath not found! Is Deskdrop running?" -ForegroundColor Red
    exit 1
}

function Send-Command($cmd) {
    Write-Host "Sending: $cmd"
    try {
        $pipeClient = New-Object System.IO.Pipes.NamedPipeClientStream(".", $pipeName, [System.IO.Pipes.PipeDirection]::Out)
        $pipeClient.Connect(2000)
        $writer = New-Object System.IO.StreamWriter($pipeClient)
        $writer.WriteLine($cmd)
        $writer.Flush()
        $writer.Close()
        $pipeClient.Dispose()
        Write-Host "Command sent successfully." -ForegroundColor Green
    } catch {
        Write-Host "Failed to send command: $_" -ForegroundColor Red
    }
}

Send-Command "--open-dashboard"
Start-Sleep -Seconds 1

Send-Command "--send-file-dialog"
Start-Sleep -Seconds 1

Write-Host "Smoke tests complete." -ForegroundColor Cyan
