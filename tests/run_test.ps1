# Load arguments as environmental variables

param (
    [Parameter(Mandatory=$true)]
    [string]$Path_to_eim,

    [Parameter(Mandatory=$true)]
    [string]$Version
)

# Save the arguments as environment variables
$env:EIM_GUI_PATH = $Path_to_eim
$env:EIM_GUI_VERSION = $Version

Set-Location -Path "./tests"

# Expand Node modules folder
# The zip file is currently being expanded in the pre-test, if it was not executed before please run this line locally
# Expand-Archive node_modules.zip

# Install node modules using npm ci
# This can be used if the node modules folder is not packed with the repo
# npm ci

# Run tests using npm run <test file>
npm run startup
npm run default