for ($i = 1; $i -le 10; $i++) {
    $examplePath = ".\examples\$i.omg"
    if (Test-Path $examplePath) {
        Write-Host "`nRunning $i.omg:`n"
        python .\omg.py $examplePath
    }
}