# Store env var pairs as semicolon-separated string for later parsing
set -g FISH_ENV_VAR_PAIRS "{{fish_env_var_pairs}}"

# --- Capture absolute eim path early (before PATH changes) ---
set -g _EIM_BIN (command -v eim 2>/dev/null; or true)

function parse_cmake_version
    set cmake_file "{{idf_path_escaped}}/tools/cmake/version.cmake"

    # Check if file exists
    if not test -f "$cmake_file"
        echo "Error: CMake version file not found at: $cmake_file" >&2
        return 1
    end

    set major ""
    set minor ""

    # Read the file and extract version numbers
    while read -l line
        # Trim leading whitespace
        set line (string trim --left "$line")

        # Check for version lines
        if string match -q "set(IDF_VERSION_MAJOR *" "$line"
            set -l matches (string match -r 'set\(IDF_VERSION_MAJOR\s+(\d+)' "$line")
            and set major $matches[2]
        else if string match -q "set(IDF_VERSION_MINOR *" "$line"
            set -l matches (string match -r 'set\(IDF_VERSION_MINOR\s+(\d+)' "$line")
            and set minor $matches[2]
        end
    end < "$cmake_file"

    # Check if both versions were found
    if test -z "$major" -o -z "$minor"
        echo "Error: Could not find both major and minor version numbers" >&2
        return 1
    end

    # Return the version
    echo "$major.$minor"
    return 0
end

set IDF_VERSION (parse_cmake_version)

# Store env var pairs as a list (parse the semicolon-separated string)
set -g ENV_VAR_PAIRS (string split ";" "$FISH_ENV_VAR_PAIRS")

# Function to print environment variables
function print_env_variables
    printf '%s\n' "PATH={{addition_to_path}}"
    printf '%s\n' "SYSTEM_PATH={{current_system_path}}"
    printf '%s\n' "ESP_IDF_VERSION=$IDF_VERSION"

    # Process environment variables
    for pair in $ENV_VAR_PAIRS
        if test -n "$pair"
            set -l key (string split -m 1 ":" "$pair")[1]
            set -l value (string split -m 1 ":" "$pair")[2]
            printf '%s=%s\n' "$key" "$value"
        end
    end
end

function add_env_variable
    set -gx ESP_IDF_VERSION "$IDF_VERSION"
    printf '%s\n' "Added environment variable ESP_IDF_VERSION = $ESP_IDF_VERSION"

    # Process environment variables
    for pair in $ENV_VAR_PAIRS
        if test -n "$pair"
            set -l key (string split -m 1 ":" "$pair")[1]
            set -l value (string split -m 1 ":" "$pair")[2]
            set -gx $key "$value"
            printf '%s\n' "Added environment variable $key = $value"
        end
    end
end

# Function to add a directory to the system PATH
function add_to_path
    # Fish PATH is a list; split colon-separated paths into individual entries
    set -l new_paths (string split ":" "{{addition_to_path}}")
    set -gx PATH $new_paths $PATH
    printf '%s\n' "Added proper directory to PATH"
end

# Function to activate Python virtual environment
function activate_venv
    set -l venv_path $argv[1]
    if test -f "$venv_path/bin/activate.fish"
        source "$venv_path/bin/activate.fish"
        printf '%s\n' "Activated virtual environment at $venv_path"
    else if test -f "$venv_path/bin/activate"
        # Fallback to bash-style activate
        source "$venv_path/bin/activate"
        printf '%s\n' "Activated virtual environment at $venv_path (bash-style)"
    else
        printf '%s\n' "Virtual environment not found at $venv_path"
        return 1
    end
end

# Check if the script is being sourced
if not status is-interactive
    if test (count $argv) -ge 1; and test "$argv[1]" = "-e"
        print_env_variables
        exit 0
    else
        echo "This script should be sourced, not executed."
        echo "If you want to print environment variables, run it with the -e parameter."
        exit 1
    end
end

# Define functions for IDF tools
function idf.py
    {{python_bin_path}} {{idf_path_escaped}}/tools/idf.py $argv
end

function esptool.py
    {{python_bin_path}} {{idf_path_escaped}}/components/esptool_py/esptool/esptool.py $argv
end

function espefuse.py
    {{python_bin_path}} {{idf_path_escaped}}/components/esptool_py/esptool/espefuse.py $argv
end

function espsecure.py
    {{python_bin_path}} {{idf_path_escaped}}/components/esptool_py/esptool/espsecure.py $argv
end

function otatool.py
    {{python_bin_path}} {{idf_path_escaped}}/components/app_update/otatool.py $argv
end

function parttool.py
    {{python_bin_path}} {{idf_path_escaped}}/components/partition_table/parttool.py $argv
end

# Main execution
add_env_variable
add_to_path

# Activate virtual environment (use IDF_PYTHON_ENV_PATH if set, otherwise default)
set -l venv_default "{{idf_python_env_path_escaped}}"
if set -q IDF_PYTHON_ENV_PATH; and test -n "$IDF_PYTHON_ENV_PATH"
    activate_venv "$IDF_PYTHON_ENV_PATH"
else
    activate_venv "$venv_default"
end

printf '%s\n' "Environment setup complete for the current shell session."
printf '%s\n' "These changes will be lost when you close this terminal."
printf '%s\n' "You are now using IDF version $IDF_VERSION."

# Sync selection with eim_idf.json for IDEs (silent on failure)
if test -n "$_EIM_BIN"; and test -x "$_EIM_BIN"
    $_EIM_BIN select "{{idf_version}}" >/dev/null 2>&1; and printf '%s\n' "eim select {{idf_version}}"
end
