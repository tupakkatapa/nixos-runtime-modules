#!/usr/bin/env bash

# Exit immediately if a command fails, unless in a conditional
set -e

# Set default action if only one argument is provided
if [[ $# -eq 1 ]]; then
  ACTION="$1"
  MODULE=""
elif [[ $# -ge 2 ]]; then
  MODULE="$1"
  ACTION="$2"
else
  ACTION="help"
  MODULE=""
fi

SYSTEM_MODULES_DIR="@DATA_DIR@"
HOST="@HOST_NAME@"
BASE_FLAKE="@FLAKE_URL@"
STATE_FILE="$SYSTEM_MODULES_DIR/state.json"
MODULES_FILE="$SYSTEM_MODULES_DIR/modules.json"
ORIGINAL_FLAKE_DIR="$SYSTEM_MODULES_DIR/original_flake"
FLAKE_FILE="$SYSTEM_MODULES_DIR/flake.nix"

# Default flags for the nix command
declare -a NIX_FLAGS=(
  --accept-flake-config
  --impure
)

# Help text
show_help() {
  echo "Usage: runtime-module <module-name> <action>"
  echo "   or: runtime-module <action>"
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

# Make sure the original flake is available and updated
ensure_original_flake() {
  # Create the directory if it doesn't exist
  mkdir -p "$ORIGINAL_FLAKE_DIR"

  if [[ $BASE_FLAKE == github:* ]]; then
    # Extract the GitHub repo URL
    REPO_URL=$(echo "$BASE_FLAKE" | sed 's/^github:/https:\/\/github.com\//')

    # Check if it's already a git repo
    if [[ -d "$ORIGINAL_FLAKE_DIR/.git" ]]; then
      echo "updating existing git repository.."
      (cd "$ORIGINAL_FLAKE_DIR" && git pull)
    else
      echo "cloning git repository.."
      # Clear the directory first
      rm -rf "$ORIGINAL_FLAKE_DIR"
      mkdir -p "$ORIGINAL_FLAKE_DIR"

      # Extract branch/tag if specified (after the first /)
      if [[ $REPO_URL == */* ]]; then
        BRANCH=$(echo "$REPO_URL" | cut -d'/' -f4-)
        REPO_URL=$(echo "$REPO_URL" | cut -d'/' -f1-3)

        # Clone the repo with the specific branch/tag
        git clone --depth 1 --branch "$BRANCH" "$REPO_URL" "$ORIGINAL_FLAKE_DIR"
      else
        # Clone the repo without specific branch
        git clone --depth 1 "$REPO_URL" "$ORIGINAL_FLAKE_DIR"
      fi
    fi
  elif [[ $BASE_FLAKE == path:* ]]; then
    # Extract the local path
    LOCAL_PATH=${BASE_FLAKE#path:}

    # Always update with rsync, taking .gitignore into account
    echo "updating from '$LOCAL_PATH' to '$ORIGINAL_FLAKE_DIR'"
    if [[ -f "$LOCAL_PATH/.gitignore" ]]; then
      # Use .gitignore if it exists
      rsync -a --delete --exclude='/.git' --filter='dir-merge,-n /.gitignore' "$LOCAL_PATH/" "$ORIGINAL_FLAKE_DIR/"
    else
      # If no .gitignore, just exclude .git
      rsync -a --delete --exclude='/.git' "$LOCAL_PATH/" "$ORIGINAL_FLAKE_DIR/"
    fi
  else
    echo "error: unsupported flake URL format '$BASE_FLAKE'"
    exit 1
  fi

  # Ensure flake directory has correct permissions
  chmod -R 755 "$ORIGINAL_FLAKE_DIR"
}

# Check if module exists in the registry
check_module() {
  if ! jq -e '.modules[] | select(.name == "'"$MODULE"'")' "$MODULES_FILE" >/dev/null; then
    echo "error: module '$MODULE' not found"
    echo "available modules:"
    jq -r '.modules[].name' "$MODULES_FILE"
    exit 1
  fi
}

# Get module path from the registry
get_module_path() {
  jq -r '.modules[] | select(.name == "'"$1"'") | .path' "$MODULES_FILE"
}

# Check if a module is enabled
is_module_enabled() {
  local mod="$1"
  jq -e '.enabledModules | index("'"$mod"'")' "$STATE_FILE" >/dev/null
  return $?
}

# List all modules with their status
list_modules() {
  echo "Available runtime modules:"
  echo ""

  jq -r '.modules[].name' "$MODULES_FILE" | while read -r name; do
    if is_module_enabled "$name"; then
      status="[✓]"
    else
      status="[ ]"
    fi
    echo "$status $name"
  done
}

# Get list of all active modules
get_active_modules() {
  jq -r '.enabledModules[]' "$STATE_FILE"
}

# Update the enabled modules list in state.json
update_enabled_modules() {
  local modules=("$@")
  local json_array
  json_array=$(printf '"%s",' "${modules[@]}" | sed 's/,$//')
  echo "{\"enabledModules\": [$json_array]}" >"$STATE_FILE"
}

# Create or update the temporary flake.nix
generate_tmp_flake() {
  # Get list of currently active modules
  mapfile -t active_modules < <(get_active_modules)

  # Generate module imports for flake.nix
  module_paths=""
  for mod in "${active_modules[@]}"; do
    module_path=$(get_module_path "$mod")
    # Use the relative path to the original module in the original flake structure
    module_paths+="        \"$module_path\"\n"
  done

  # Write the flake.nix file
  cat >"$FLAKE_FILE" <<EOF
{
  description = "Runtime modules configuration";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  inputs.base.url = "path:$ORIGINAL_FLAKE_DIR";

  outputs = { self, nixpkgs, base }: {
    nixosConfigurations.runtime = base.nixosConfigurations.$HOST.extendModules {
      modules = [
$(echo -e "$module_paths")
      ];
    };
  };
}
EOF

  # Fix permissions on the flake file
  chmod 644 "$FLAKE_FILE"

  # Debug info
  echo "generated flake.nix at '$FLAKE_FILE'"
}

# Apply the current configuration
apply_configuration() {
  echo "applying configuration.."

  # Generate the temporary flake
  generate_tmp_flake

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
  list_modules
  ;;

reset)
  # Check for sudo privileges
  if [ $EUID != 0 ]; then
    echo "warning: elevated privileges are required to reset the system"
    sudo "$0" reset
    exit $?
  fi

  # Ensure original flake is available
  ensure_original_flake

  echo "resetting to base system (disabling all modules).."

  # Empty the array of enabled modules
  update_enabled_modules

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

  # Ensure original flake is available
  ensure_original_flake

  echo "building module $MODULE.."

  # Get current enabled modules
  mapfile -t active_modules < <(get_active_modules)

  # Add the current module if not already active
  if ! is_module_enabled "$MODULE"; then
    active_modules+=("$MODULE")
    update_enabled_modules "${active_modules[@]}"
  else
    echo "module $MODULE is already enabled"
  fi

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

  # Ensure original flake is available
  ensure_original_flake

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
    if [[ $mod != "$MODULE" ]]; then
      new_enabled_modules+=("$mod")
    fi
  done

  # Update the state file
  update_enabled_modules "${new_enabled_modules[@]}"

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
