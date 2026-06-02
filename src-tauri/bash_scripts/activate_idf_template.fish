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
    set patch ""

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
        else if string match -q "set(IDF_VERSION_PATCH *" "$line"
            set -l matches (string match -r 'set\(IDF_VERSION_PATCH\s+(\d+)' "$line")
            and set patch $matches[2]
        end
    end < "$cmake_file"

    # Check if both versions were found
    if test -z "$major" -o -z "$minor"
        echo "Error: Could not find both major and minor version numbers" >&2
        return 1
    end

    # Return the version (include patch if available)
    if test -n "$patch"
        echo "$major.$minor.$patch"
    else
        echo "$major.$minor"
    end
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

function _idf_py_completion
    set -l words (commandline -opc)
    if test (count $words) -eq 0
        set words "idf.py"
    end

    set -l current_word (commandline -ct)

    if string match -q "@*" "$current_word"
        set -l search_path (string sub -s 2 -- "$current_word")

        # Handle directory paths (ends with /)
        if string match -q "*/" "$search_path"
            set -l dir (string replace -r '/$' '' "$search_path")
            if test -d "$dir"
                # List contents, prepend @, and return
                for f in "$dir"/*
                    echo "@$f"
                end
            end
            return
        end

        set -l candidates $search_path*
        set -l found false

        for f in $candidates
            if test -e "$f"
                echo "@$f"
                set found true
            end
        end

        if not test "$found" = true
            echo "@$search_path"
        end
        return
    end

    set -l comp_words (string join " " $words)
    set -l comp_cword (math (count $words) - 1)
    if test "$comp_cword" -lt 1
        set comp_cword 1
    end
    set -l comp_line (string join " " $words)

    set -l results (env COMP_WORDS="$comp_words" COMP_CWORD="$comp_cword" COMP_LINE="$comp_line" \
        _IDF.PY_COMPLETE=zsh_complete \
        IDF_SKIP_DEPS=1 IDF_COMPONENT_MERGE=0 \
        IDF_PATH="{{idf_path_escaped}}" \
        IDF_PYTHON_ENV_PATH="{{idf_python_env_path_escaped}}" \
        "{{python_bin_path}}" "{{idf_path_escaped}}/tools/idf.py" 2>/dev/null)

    if test -z "$results"
        set results (env COMP_WORDS="$comp_words" COMP_CWORD="$comp_cword" COMP_LINE="$comp_line" \
            _IDF.PY_COMPLETE=fish_complete \
            IDF_SKIP_DEPS=1 IDF_COMPONENT_MERGE=0 \
            IDF_PATH="{{idf_path_escaped}}" \
            IDF_PYTHON_ENV_PATH="{{idf_python_env_path_escaped}}" \
            "{{python_bin_path}}" "{{idf_path_escaped}}/tools/idf.py" 2>/dev/null)
    end

    for line in $results
        set line (string trim "$line")
        if test -z "$line"; continue; end

        set line (string replace -r "^plain\s+" "" "$line")

        if string match -q "*\t*" "$line"
            echo "$line"
            continue
        end

        set -l parts (string split -m 1 " " "$line")
        set -l cmd $parts[1]

        if test (count $parts) -gt 1
            printf '%s\t%s\n' "$cmd" "$parts[2]"
        else
            echo "$line"
        end
    end
end

function register_idf_completions
    # Clear stale completions
    complete -c idf.py -e 2>/dev/null

    # Register the function
    complete -c idf.py -f -a '(_idf_py_completion)' 2>/dev/null
    and printf '%s\n' "Registered idf.py tab completion (fish)."
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
    "{{python_bin_path}}" "{{idf_path_escaped}}/tools/idf.py" $argv
end

function esptool
    "{{python_bin_path}}" "{{idf_path_escaped}}/components/esptool_py/esptool/esptool.py" $argv
end

function esptool.py
    esptool $argv
end

function espefuse
    "{{python_bin_path}}" "{{idf_path_escaped}}/components/esptool_py/esptool/espefuse.py" $argv
end

function espefuse.py
    espefuse $argv
end

function espsecure
    "{{python_bin_path}}" "{{idf_path_escaped}}/components/esptool_py/esptool/espsecure.py" $argv
end

function espsecure.py
    espsecure $argv
end

function otatool.py
    "{{python_bin_path}}" "{{idf_path_escaped}}/components/app_update/otatool.py" $argv
end

function parttool.py
    "{{python_bin_path}}" "{{idf_path_escaped}}/components/partition_table/parttool.py" $argv
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

register_idf_completions

printf '%s\n' "Environment setup complete for the current shell session."
printf '%s\n' "These changes will be lost when you close this terminal."
printf '%s\n' "You are now using IDF version $IDF_VERSION."

# Sync selection with eim_idf.json for IDEs (silent on failure)
if test -n "$_EIM_BIN"; and test -x "$_EIM_BIN"
    $_EIM_BIN select "{{idf_version}}" >/dev/null 2>&1; and printf '%s\n' "eim select {{idf_version}}"
end
