#!/usr/bin/env bash

# Exit immediately if a command fails, unless in a conditional
set -e

# Set default action if only one argument is provided
if [[ $# -eq 1 ]]; then
  ACTION="$1"
  MODULE=""
elif [[ $# -ge 2 ]]; then
  ACTION="$1"
  MODULE="$2"
fi

SYSTEM_MODULES_DIR="@DATA_DIR@"
MODULES_JSON='@MODULES_JSON@'
MODULES_FILE="$SYSTEM_MODULES_DIR/runtime-modules.nix"

# Default flags for the nix command
declare -a NIX_FLAGS=(
  --accept-flake-config
  --impure
)

# Help text
show_help() {
  echo "Usage: runtime-module <action>"
  echo "   or: runtime-module <action> <module-name>"
  echo ""
  echo "Actions:"
  echo "  enable        - Build and enable the module"
  echo "  disable       - Disable specific module"
  echo "  reset         - Disable all modules (revert to base system)"
  echo "  status        - Show module status (enabled/disabled)"
  echo "  list          - List all available modules"
  echo "  help          - Show this help message"
  echo ""
}

# Check if module exists in the registry
check_module() {
  if ! echo "$MODULES_JSON" | jq -e '.modules[] | select(.name == "'"$MODULE"'")' >/dev/null; then
    echo "error: module '$MODULE' not found"
    echo "available modules:"
    echo "$MODULES_JSON" | jq -r '.modules[].name'
    exit 1
  fi
}

# Get module path from the registry
get_module_path() {
  echo "$MODULES_JSON" | jq -r '.modules[] | select(.name == "'"$1"'") | .path'
}

# Check if a module is enabled by parsing runtime-modules.nix
is_module_enabled() {
  local mod="$1"
  local module_path

  module_path=$(get_module_path "$mod")

  # Module is enabled if its path is in the modules file
  if [[ -f $MODULES_FILE ]]; then
    grep -q "\"$module_path\"" "$MODULES_FILE"
    return $?
  fi
  return 1
}

# Get list of all active modules by parsing runtime-modules.nix
get_active_modules() {
  if [ ! -f "$MODULES_FILE" ]; then
    return
  fi

  local paths

  # Extract all paths from the modules file
  paths=$(grep -oE '"[^"]+"' "$MODULES_FILE" | tr -d '"' | grep -v "^$")

  # For each path, find the module name
  for path in $paths; do
    echo "$MODULES_JSON" | jq -r --arg path "$path" '.modules[] | select(.path == $path) | .name'
  done
}

# Create or update the runtime-modules.nix file with the specified modules
generate_modules_file() {
  local modules=("$@")

  # Start creating the modules file
  echo "# This file is generated by runtime-module script" >"$MODULES_FILE"
  echo "{ ... }:" >>"$MODULES_FILE"
  echo "{" >>"$MODULES_FILE"

  if [[ ${#modules[@]} -eq 0 ]]; then
    # If no modules are active, return an empty module
    echo "  # No active modules" >>"$MODULES_FILE"
  else
    echo "  imports = [" >>"$MODULES_FILE"

    # Add each module path
    for mod in "${modules[@]}"; do
      module_path=$(get_module_path "$mod")
      if [[ -n $module_path ]]; then
        echo "    \"$module_path\"" >>"$MODULES_FILE"
      fi
    done

    echo "  ];" >>"$MODULES_FILE"
  fi

  echo "}" >>"$MODULES_FILE"

  # Fix permissions
  chmod 644 "$MODULES_FILE"

  # Debug info
  echo "generated modules file at '$MODULES_FILE'"
}

# Apply the current configuration
apply_configuration() {
  echo "applying configuration.."

  # Change to the system modules directory
  cd "$SYSTEM_MODULES_DIR"

  # Run nixos-rebuild
  set +e
  nixos-rebuild test "${NIX_FLAGS[@]}" --flake ".#runtime"
  result=$?
  set -e

  return $result
}

# Handle commands
case "$ACTION" in
list)
  echo "Available runtime modules:"
  echo ""

  echo "$MODULES_JSON" | jq -r '.modules[].name' | while read -r name; do
    if is_module_enabled "$name"; then
      status="[✓]"
    else
      status="[ ]"
    fi
    echo "$status $name"
  done
  ;;

reset)
  # Check for sudo privileges
  if [ $EUID != 0 ]; then
    echo "warning: elevated privileges are required to reset the system"
    sudo "$0" reset
    exit $?
  fi

  echo "resetting to base system (disabling all modules).."

  # Create empty modules file (no modules enabled)
  generate_modules_file

  # Apply the configuration
  apply_configuration
  result=$?

  if [[ $result -eq 0 ]]; then
    echo "system reset to base configuration successfully"
  else
    echo "warning: system reset with warnings (some changes may not be fully applied)"
  fi
  ;;

enable)
  if [[ -z $MODULE ]]; then
    echo "info: module name is required for the 'enable' action"
    show_help
    exit 1
  fi

  # Check for sudo privileges
  if [ $EUID != 0 ]; then
    echo "info: elevated privileges are required to enable module '$MODULE'"
    sudo "$0" "$@"
    exit $?
  fi

  # Check if module exists
  check_module

  echo "building module $MODULE.."

  # Get current enabled modules
  mapfile -t active_modules < <(get_active_modules)

  # Add the current module if not already active
  if ! is_module_enabled "$MODULE"; then
    active_modules+=("$MODULE")
  else
    echo "module $MODULE is already enabled"
  fi

  # Generate modules file with updated modules list
  generate_modules_file "${active_modules[@]}"

  # Apply the configuration
  apply_configuration
  result=$?

  if [[ $result -eq 0 ]]; then
    echo "module $MODULE enabled successfully"
  else
    echo "warning: module $MODULE enabled with warnings (some changes may not be fully applied)"
  fi
  ;;

disable)
  if [[ -z $MODULE ]]; then
    echo "error: module name is required for the 'disable' action"
    show_help
    exit 1
  fi

  # Check for sudo privileges
  if [ $EUID != 0 ]; then
    echo "warning: elevated privileges are required to disable module '$MODULE'"
    sudo "$0" "$@"
    exit $?
  fi

  # Check if module exists
  check_module

  # Check if module is enabled first
  if ! is_module_enabled "$MODULE"; then
    echo "error: module $MODULE is already disabled"
    exit 0
  fi

  echo "disabling module $MODULE.."

  # Get current enabled modules and remove the one being disabled
  mapfile -t enabled_modules < <(get_active_modules)
  new_enabled_modules=()

  for mod in "${enabled_modules[@]}"; do
    if [[ $mod != "$MODULE" && -n $mod ]]; then
      new_enabled_modules+=("$mod")
    fi
  done

  # Generate modules file with updated modules list
  generate_modules_file "${new_enabled_modules[@]}"

  # Apply the configuration
  apply_configuration
  result=$?

  if [[ $result -eq 0 ]]; then
    echo "module $MODULE disabled successfully"
  else
    echo "warning: module $MODULE disabled with warnings (some changes may not be fully applied)"
  fi
  ;;

status)
  if [[ -z $MODULE ]]; then
    echo "error: module name is required for the 'status' action"
    show_help
    exit 1
  fi

  check_module

  if is_module_enabled "$MODULE"; then
    echo "enabled"
    exit 0
  else
    echo "disabled"
    exit 1
  fi
  ;;

help | *)
  show_help
  ;;
esac
