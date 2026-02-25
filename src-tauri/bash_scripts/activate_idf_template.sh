#!/bin/sh

{{env_var_pairs}}

# --- Capture absolute eim path early (before PATH changes) ---
_EIM_BIN="$(command -v eim 2>/dev/null || true)"

parse_cmake_version() {
    cmake_file="{{idf_path_escaped}}/tools/cmake/version.cmake"

    # Check if file exists
    if [ ! -f "$cmake_file" ]; then
        echo "Error: CMake version file not found at: $cmake_file" >&2
        return 1
    fi

    major=""
    minor=""

    # Read the file and extract version numbers using POSIX tools
    while IFS= read -r line || [ -n "$line" ]; do
        # Trim leading whitespace using parameter expansion
        line=$(printf '%s\n' "$line" | sed 's/^[[:space:]]*//')

        # Check for version lines using case statement (POSIX compatible)
        case "$line" in
            "set(IDF_VERSION_MAJOR "*)
                # Extract number using sed
                major=$(printf '%s\n' "$line" | sed 's/.*set(IDF_VERSION_MAJOR[[:space:]]*\([0-9]*\).*/\1/')
                ;;
            "set(IDF_VERSION_MINOR "*)
                # Extract number using sed
                minor=$(printf '%s\n' "$line" | sed 's/.*set(IDF_VERSION_MINOR[[:space:]]*\([0-9]*\).*/\1/')
                ;;
        esac
    done < "$cmake_file"

    # Check if both versions were found
    if [ -z "$major" ] || [ -z "$minor" ]; then
        echo "Error: Could not find both major and minor version numbers" >&2
        return 1
    fi

    # Return the version
    printf '%s.%s\n' "$major" "$minor"
    return 0
}

IDF_VERSION=$(parse_cmake_version)
ENV_VAR_PAIRS=$(get_env_var_pairs)

# Function to print environment variables
print_env_variables() {
    printf '%s\n' "PATH={{addition_to_path}}"
    printf '%s\n' "SYSTEM_PATH={{current_system_path}}"
    printf '%s\n' "ESP_IDF_VERSION=$IDF_VERSION"

    # Process environment variables
    printf '%s\n' "$ENV_VAR_PAIRS" | while read -r pair; do
        if [ -n "$pair" ]; then
            key="${pair%%:*}"
            value="${pair#*:}"
            printf '%s=%s\n' "$key" "$value"
        fi
    done
}

add_env_variable() {
    export ESP_IDF_VERSION="$IDF_VERSION"
    printf '%s\n' "Added environment variable ESP_IDF_VERSION = $ESP_IDF_VERSION"

    # Create a temporary file
    temp_file=$(mktemp)
    printf '%s\n' "$ENV_VAR_PAIRS" > "$temp_file"

    # Process environment variables
    while read -r pair; do
        if [ -n "$pair" ]; then
            key="${pair%%:*}"
            value="${pair#*:}"
            eval "export $key=\"$value\""
            printf '%s\n' "Added environment variable $key = $value"
        fi
    done < "$temp_file"
}

# Function to add a directory to the system PATH
add_to_path() {
    export PATH="{{addition_to_path}}:$PATH"
    printf '%s\n' "Added proper directory to PATH"
}

# Function to activate Python virtual environment
activate_venv() {
    venv_path="$1"
    if [ -f "$venv_path/bin/activate" ]; then
        # shellcheck disable=SC1090
        . "$venv_path/bin/activate"
        printf '%s\n' "Activated virtual environment at $venv_path"
    else
        printf '%s\n' "Virtual environment not found at $venv_path"
        return 1
    fi
}

# Check if the script is being sourced or executed
is_sourced() {
  if [ -n "$ZSH_VERSION" ]; then
      case $ZSH_EVAL_CONTEXT in *:file:*) return 0;; esac
  else  # Add additional POSIX-compatible shell names here, if needed.
      case ${0##*/} in dash|-dash|bash|-bash|ksh|-ksh|sh|-sh) return 0;; esac
  fi
  return 1  # NOT sourced.
}

# Sample call.
is_sourced && sourced=1 || sourced=0

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

alias idf.py="{{idf_python_env_path_escaped}}/bin/python {{idf_path_escaped}}/tools/idf.py"

alias esptool.py="{{idf_python_env_path_escaped}}/bin/python {{idf_path_escaped}}/components/esptool_py/esptool/esptool.py"

alias espefuse.py="{{idf_python_env_path_escaped}}/bin/python {{idf_path_escaped}}/components/esptool_py/esptool/espefuse.py"

alias espsecure.py="{{idf_python_env_path_escaped}}/bin/python {{idf_path_escaped}}/components/esptool_py/esptool/espsecure.py"

alias otatool.py="{{idf_python_env_path_escaped}}/bin/python {{idf_path_escaped}}/components/app_update/otatool.py"

alias parttool.py="{{idf_python_env_path_escaped}}/bin/python {{idf_path_escaped}}/components/partition_table/parttool.py"


# Main execution
add_env_variable
add_to_path

# Activate virtual environment (uncomment and provide the correct path)
venv_default="{{idf_python_env_path_escaped}}"
activate_venv "${IDF_PYTHON_ENV_PATH:-$venv_default}"

printf '%s\n' "Environment setup complete for the current shell session."
printf '%s\n' "These changes will be lost when you close this terminal."
printf '%s\n' "You are now using IDF version $IDF_VERSION."

# Sync selection with eim_idf.json for IDEs (silent on failure)
if [ -n "$_EIM_BIN" ] && [ -x "$_EIM_BIN" ]; then
    "$_EIM_BIN" select "{{idf_version}}" >/dev/null 2>&1 && printf '%s\n' "eim select {{idf_version}}"
fi
