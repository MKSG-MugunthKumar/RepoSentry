#!/bin/bash

# Enhanced GitHub repository sync script with timestamp preservation
# Author: MK
# Features:
# - Restores file modification dates from git commit history
# - Uses directory names that include latest commit timestamp

set -euo pipefail

GH_USER="MKSG-MugunthKumar"
BASE_DIR="$HOME/dev/"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Logging function
log() {
    echo -e "${BLUE}[$(date '+%H:%M:%S')]${NC} $1"
}

warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}


# Function to set directory timestamp from commit (no renaming)
set_directory_commit_timestamp() {
    local repo_dir="$1"

    if [[ ! -d "$repo_dir" ]]; then
        warn "Directory $repo_dir does not exist"
        return 1
    fi

    cd "$repo_dir"

    # Check if it's a git repository
    if [[ ! -d ".git" ]]; then
        warn "Not a git repository: $repo_dir"
        cd ..
        return 0
    fi

    # Get the latest commit timestamp
    local commit_timestamp
    commit_timestamp=$(git log -1 --format="%ct" 2>/dev/null)

    # Validate and set directory timestamp (allow from 2005 when Git was created, until 2050)
    if [[ -n "$commit_timestamp" && "$commit_timestamp" -gt 1104537600 && "$commit_timestamp" -lt 2524608000 ]]; then
        if [[ "$OSTYPE" == "darwin"* ]]; then
            # macOS: Set directory modification time
            if touch -t "$(date -r "$commit_timestamp" "+%Y%m%d%H%M.%S")" "../$repo_dir" 2>/dev/null; then
                log "Set directory timestamp to $(date -r "$commit_timestamp")"
            else
                warn "Failed to set directory timestamp"
            fi
        else
            # GNU/Linux: Set directory modification time
            if touch -d "@$commit_timestamp" "../$repo_dir" 2>/dev/null; then
                log "Set directory timestamp to $(date -d "@$commit_timestamp")"
            else
                warn "Failed to set directory timestamp"
            fi
        fi
    else
        warn "Invalid commit timestamp: $commit_timestamp"
    fi

    cd ..
}

# Function to find existing repository directory
find_existing_repo_dir() {
    local repo_basename="$1"

    # Look for exact match
    if [[ -d "$repo_basename" ]]; then
        # Verify it's actually a git repository
        if [[ -d "$repo_basename/.git" ]]; then
            echo "$repo_basename"
            return 0
        else
            warn "Directory $repo_basename exists but is not a git repository"
            return 2  # Different exit code for non-git directory
        fi
    fi

    return 1
}

# Main execution
main() {
    log "Starting GitHub repository sync for user: $GH_USER"
    log "Base directory: $BASE_DIR"

    # Create and navigate to base directory
    mkdir -p "$BASE_DIR"
    cd "$BASE_DIR"

    # Check if gh CLI is available
    if ! command -v gh >/dev/null 2>&1; then
        error "GitHub CLI (gh) is not installed or not in PATH"
        exit 1
    fi

    # Check if gh is authenticated
    if ! gh auth status >/dev/null 2>&1; then
        error "GitHub CLI is not authenticated. Run 'gh auth login' first."
        exit 1
    fi

    log "Fetching repository list..."

    # Get list of repositories and process each one
    repo_count=0
    success_count=0

    # Function to process repositories from a source
    process_repos() {
        local source_type="$1"
        local gh_command="$2"
        local org_name="${3:-}"  # Optional organization name for directory creation

        log "Fetching $source_type repositories..."

        # Create organization directory if this is an org
        if [[ -n "$org_name" ]]; then
            mkdir -p "$org_name"
            cd "$org_name"
            log "Working in organization directory: $org_name"
        fi

        # Get the repository list first, then process it
        local repo_list
        repo_list=$($gh_command 2>&1)
        local cmd_exit_code=$?

        if [[ $cmd_exit_code -ne 0 ]]; then
            error "Failed to fetch repository list for $source_type"
            error "Command: $gh_command"
            error "Output: $repo_list"
            return 1
        fi

        # Process each repository
        while IFS= read -r repo; do
            [[ -z "$repo" ]] && continue

            repo_count=$((repo_count + 1))
            local repo_basename
            repo_basename=$(basename "$repo")

            log "Processing repository $repo_count ($source_type): $repo"

            # Find existing directory for this repo
            local find_result
            find_existing_repo_dir "$repo_basename" >/dev/null
            find_result=$?

            if [[ $find_result -eq 0 ]]; then
                log "Found existing git repository: $repo_basename"

                # Pull latest changes
                cd "$repo_basename"
                if git pull --quiet; then
                    success "Updated $repo"
                else
                    error "Failed to update $repo"
                    cd ..
                    continue
                fi
                cd ..

                # Set directory timestamp to match latest commit
                set_directory_commit_timestamp "$repo_basename"

            elif [[ $find_result -eq 2 ]]; then
                error "Directory $repo_basename exists but is not a git repository. Skipping..."
                continue

            else
                log "Cloning new repository: $repo"

                # Clone the repository with better error handling
                local clone_output
                clone_output=$(gh repo clone "$repo" 2>&1)
                local clone_exit_code=$?

                if [[ $clone_exit_code -eq 0 ]]; then
                    # Verify the clone actually created the directory
                    if [[ -d "$repo_basename" ]]; then
                        success "Cloned $repo"

                        # Set directory timestamp to match latest commit
                        set_directory_commit_timestamp "$repo_basename"
                    else
                        error "Clone reported success but directory $repo_basename not found"
                        continue
                    fi
                else
                    error "Failed to clone $repo (exit code: $clone_exit_code)"

                    # Show helpful error messages for common issues
                    if [[ "$clone_output" =~ "authentication required" ]]; then
                        error "Authentication failed. Run 'gh auth login' to re-authenticate."
                    elif [[ "$clone_output" =~ "repository not found" ]]; then
                        error "Repository not found or no access permissions."
                    elif [[ "$clone_output" =~ "Network is unreachable" ]]; then
                        error "Network connectivity issue. Check your internet connection."
                    else
                        error "Clone error output: $clone_output"
                    fi

                    # Clean up any partial clone directory that might have been created
                    if [[ -d "$repo_basename" ]]; then
                        warn "Removing incomplete clone directory: $repo_basename"
                        rm -rf "$repo_basename"
                    fi
                    continue
                fi
            fi

            success_count=$((success_count + 1))

        done <<< "$repo_list"

        # Return to base directory if we were in an org directory
        if [[ -n "$org_name" ]]; then
            cd ..
            log "Returned to base directory from organization: $org_name"
        fi
    }

    # Process personal repositories
    process_repos "personal" "gh repo list $GH_USER --limit 1000 --json nameWithOwner -q .[].nameWithOwner"

    # Process organization repositories
    log "Fetching organization memberships..."
    local orgs
    orgs=$(gh api user/orgs --jq '.[].login' 2>/dev/null || true)

    if [[ -n "$orgs" ]]; then
        local org_count=0
        while IFS= read -r org; do
            [[ -z "$org" ]] && continue
            ((org_count++))
            log "Processing organization $org_count: $org"

            # Check if we can access this organization's repos
            local repo_check
            repo_check=$(gh repo list "$org" --limit 1 --json nameWithOwner 2>&1)
            if [[ $? -ne 0 ]]; then
                warn "Cannot access repositories for organization: $org"
                warn "Error: $repo_check"
                continue
            fi

            process_repos "organization ($org)" "gh repo list $org --limit 1000 --json nameWithOwner -q .[].nameWithOwner" "$org"
            log "Completed processing organization: $org"
        done <<< "$orgs"
        log "Processed $org_count total organizations"
    else
        log "No organization memberships found or unable to fetch them"
    fi

    log "Sync completed: $success_count/$repo_count repositories processed successfully"
}

# Run main function only when script is executed directly, not when sourced
if [[ "${BASH_SOURCE[0]:-}" == "${0}" ]]; then
    main "$@"
fi