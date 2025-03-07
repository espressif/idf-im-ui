#!/bin/bash

{{env_var_pairs}}

# Function to print environment variables
print_env_variables() {
    echo "PATH="{{addition_to_path}}""
    echo "ESP_IDF_VERSION={{idf_version}}"
    for pair in "${env_var_pairs[@]}"; do
        key="${pair%%:*}"
        value="${pair#*:}"
        echo "$key=$value"
    done
}

# Function to add an environment variable
add_env_variable() {
    export ESP_IDF_VERSION="{{idf_version}}"
    echo "Added environment variable ESP_IDF_VERSION = $ESP_IDF_VERSION"
    for pair in "${env_var_pairs[@]}"; do
        key="${pair%%:*}"
        value="${pair#*:}"
        export "${key}=${value}"
        echo "Added environment variable $key = $value"
    done

}

# Function to add a directory to the system PATH
add_to_path() {
    export PATH="$PATH:{{addition_to_path}}"
    echo "Added proper directory to PATH"
}

# Function to activate a Python virtual environment
activate_venv() {
    VENV_PATH="$1"
    if [ -f "${VENV_PATH}/bin/activate" ]; then
        source "${VENV_PATH}/bin/activate"
        echo "Activated virtual environment at ${VENV_PATH}"
    else
        echo "Virtual environment not found at ${VENV_PATH}"
        return 1
    fi
}

# Check if the script is being sourced or executed
(return 0 2>/dev/null) && sourced=1 || sourced=0

if [ "$1" = "-e" ]; then
    print_env_variables
    exit 0
else
    if [ "$sourced" -eq 0 ]; then
        echo "This script should be sourced, not executed."
        echo "If you want to print environment variables, run it with the -e parameter."
        exit 1
    fi
fi

alias idf.py="{{idf_python_env_path_escaped}}/bin/python3 {{idf_path_escaped}}/tools/idf.py"

alias esptool.py="{{idf_python_env_path_escaped}}/bin/python3 {{idf_path_escaped}}/components/esptool_py/esptool/esptool.py"

alias espefuse.py="{{idf_python_env_path_escaped}}/bin/python3 {{idf_path_escaped}}/components/esptool_py/esptool/espefuse.py"

alias espsecure.py="{{idf_python_env_path_escaped}}/bin/python3 {{idf_path_escaped}}/components/esptool_py/esptool/espsecure.py"

alias otatool.py="{{idf_python_env_path_escaped}}/bin/python3 {{idf_path_escaped}}/components/app_update/otatool.py"

alias parttool.py="{{idf_python_env_path_escaped}}/bin/python3 {{idf_path_escaped}}/components/partition_table/parttool.py"


# Main execution
add_env_variable
add_to_path

# Activate virtual environment (uncomment and provide the correct path)
activate_venv "${IDF_PYTHON_ENV_PATH}"

echo "Environment setup complete for the current shell session."
echo "These changes will be lost when you close this terminal."
echo "You are now using IDF version {{idf_version}}."