#!/usr/bin/env bash

# Exit immediately if a command fails, unless in a conditional
set -e

SYSTEM_MODULES_DIR="@DATA_DIR@"
MODULES_JSON='@MODULES_JSON@'
MODULES_FILE="@MODULES_NIX@"

# Default flags for the nix command
declare -a NIX_FLAGS=(
  --accept-flake-config
  --impure
)

# Help text
show_help() {
  cat <<EOF
Usage: runtime-module <action> [<module-name>...]

Actions:
  enable <module...>   - Build and enable one or more modules
  disable <module...>  - Disable one or more specific modules
  reset                - Disable all modules (revert to base system)
  status <module...>   - Show module status (enabled/disabled)
  list                 - List all available modules
  help                 - Show this help message

EOF
}

# Ensure we have sudo access when needed
require_sudo() {
  local action=$1
  shift
  local args=("$@")

  if [ $EUID != 0 ]; then
    echo "info: elevated privileges are required for this action"
    sudo "$0" "$action" "${args[@]}"
    exit $?
  fi
}

# Check if modules are provided and they exist
require_and_verify_modules() {
  if [[ ${#MODULES[@]} -eq 0 ]]; then
    echo "error: module name is required for the '$ACTION' action"
    show_help
    exit 1
  fi

  for module in "${MODULES[@]}"; do
    if ! echo "$MODULES_JSON" | jq -e '.modules[] | select(.name == "'"$module"'")' >/dev/null; then
      echo "error: module '$module' not found"
      echo "available modules:"
      echo "$MODULES_JSON" | jq -r '.modules[].name'
      exit 1
    fi
  done
}

# Get module path from the registry
get_module_path() {
  echo "$MODULES_JSON" | jq -r '.modules[] | select(.name == "'"$1"'") | .path'
}

# Check if a module is enabled
is_module_enabled() {
  local mod="$1"

  if [[ -f $MODULES_FILE ]]; then
    grep -q "# ${mod}$" "${MODULES_FILE}"
    return $?
  fi
  return 1
}

# Get list of all active modules
get_active_modules() {
  [ ! -f "$MODULES_FILE" ] && return

  # Extract module names from the comment suffixes
  while read -r line; do
    if [[ $line =~ \#[[:space:]]*([[:alnum:]-]+)[[:space:]]*$ ]]; then
      module_name="${BASH_REMATCH[1]}"
      echo "$module_name"
    fi
  done <"$MODULES_FILE"
}

# Create or update the runtime-modules.nix file with the specified modules
generate_modules_file() {
  local modules=("$@")

  cat >"$MODULES_FILE" <<EOF
# This file is generated by runtime-module script
{ ... }:
{
EOF

  if [[ ${#modules[@]} -eq 0 ]]; then
    # If no modules are active, return an empty module
    echo "  # No active modules" >>"$MODULES_FILE"
  else
    echo "  imports = [" >>"$MODULES_FILE"

    # Add each module path
    for mod in "${modules[@]}"; do
      module_path=$(get_module_path "$mod")
      if [[ -n $module_path ]]; then
        echo "    \"$module_path\" # $mod" >>"$MODULES_FILE"
      fi
    done

    echo "  ];" >>"$MODULES_FILE"
  fi

  echo "}" >>"$MODULES_FILE"

  # Fix permissions
  chmod 755 "$MODULES_FILE"

  echo "generated modules file at '$MODULES_FILE'"
}

# Apply the current configuration
apply_configuration() {
  echo "applying configuration..."

  # Change to the system modules directory
  cd "$SYSTEM_MODULES_DIR"

  # Update flake before rebuild
  echo "updating flake..."
  nix flake update "${NIX_FLAGS[@]}"

  # Run nixos-rebuild
  set +e
  nixos-rebuild test "${NIX_FLAGS[@]}" --flake ".#runtime"
  local result=$?
  set -e

  # Report result
  if [[ $result -eq 0 ]]; then
    echo "configuration applied successfully"
  else
    echo "warning: configuration applied with warnings (some changes may not be fully applied)"
  fi

  return $result
}

# Parse command line arguments
parse_arguments() {
  if [[ $# -eq 0 ]]; then
    ACTION="help"
    MODULES=()
  elif [[ $# -eq 1 ]]; then
    ACTION="$1"
    MODULES=()
  elif [[ $# -ge 2 ]]; then
    ACTION="$1"
    shift
    MODULES=("$@")
  fi

  # Check if action is valid
  case "$ACTION" in
  list | reset | enable | disable | status | help)
    # Valid action
    ;;
  *)
    echo "error: invalid action '$ACTION'"
    show_help
    exit 1
    ;;
  esac
}

# Command handlers
cmd_list() {
  echo "Available modules:"

  # Print module list with status indicators
  echo "$MODULES_JSON" | jq -r '.modules[].name' | while read -r name; do
    if is_module_enabled "$name"; then
      echo "  [✓] $name"
    else
      echo "  [ ] $name"
    fi
  done
}

cmd_reset() {
  require_sudo "$ACTION"

  echo "resetting to base system (disabling all modules)..."

  # Create empty modules file (no modules enabled)
  generate_modules_file

  # Apply the configuration
  apply_configuration
}

cmd_enable() {
  require_and_verify_modules
  require_sudo "$ACTION" "${MODULES[@]}"

  # Get current enabled modules
  mapfile -t active_modules < <(get_active_modules)

  # Process each module
  for MODULE in "${MODULES[@]}"; do
    if ! is_module_enabled "$MODULE"; then
      active_modules+=("$MODULE")
    else
      echo "module $MODULE is already enabled"
    fi
  done

  # Generate modules file with updated modules list
  generate_modules_file "${active_modules[@]}"

  # Apply the configuration
  apply_configuration

  echo "modules enabled successfully"
}

cmd_disable() {
  require_and_verify_modules
  require_sudo "$ACTION" "${MODULES[@]}"

  # Get current enabled modules and prepare new list
  mapfile -t enabled_modules < <(get_active_modules)
  new_enabled_modules=()

  # Add all modules except those being disabled
  for mod in "${enabled_modules[@]}"; do
    keep=true
    for disable_mod in "${MODULES[@]}"; do
      if [[ $mod == "$disable_mod" ]]; then
        echo "disabling module $mod..."
        keep=false
        break
      fi
    done

    if [[ $keep == true && -n $mod ]]; then
      new_enabled_modules+=("$mod")
    fi
  done

  # Check if any modules to disable were not enabled
  for MODULE in "${MODULES[@]}"; do
    if ! is_module_enabled "$MODULE"; then
      echo "module $MODULE is already disabled"
    fi
  done

  # Generate modules file with updated modules list
  generate_modules_file "${new_enabled_modules[@]}"

  # Apply the configuration
  apply_configuration

  echo "modules disabled successfully"
}

cmd_status() {
  require_and_verify_modules

  exit_code=0

  for MODULE in "${MODULES[@]}"; do
    if is_module_enabled "$MODULE"; then
      echo "$MODULE: enabled"
    else
      echo "$MODULE: disabled"
      exit_code=1
    fi
  done

  exit $exit_code
}

# Main function
main() {
  # Parse arguments
  parse_arguments "$@"

  # Call the appropriate command handler
  case "$ACTION" in
  list) cmd_list ;;
  reset) cmd_reset ;;
  enable) cmd_enable ;;
  disable) cmd_disable ;;
  status) cmd_status ;;
  help | *) show_help ;;
  esac
}

# Call main function with all arguments
main "$@"
