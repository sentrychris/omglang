for ($i = 1; $i -le 10; $i++) {
    $examplePath = ".\examples\example$i.omg"
    if (Test-Path $examplePath) {
        Write-Host "`nRunning example$i.omg:`n"
        python .\oli.py $examplePath
    } else {
        Write-Host "`nSkipping example$i.omg (file not found)"
    }
}