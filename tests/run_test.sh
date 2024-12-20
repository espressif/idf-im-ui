#!/bin/bash

# Save the arguments as environment variables
export EIM_FILE_PATH="$1"
export EIM_VERSION="$2"

cd tests

# install node modules
# The zip file is currently being expanded in the pre-test, if it was not executed before please run this line locally
npm ci

# run tests
set +e
npm run startup
npm run default
npm run expert