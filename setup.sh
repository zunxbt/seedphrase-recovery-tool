#!/bin/bash

set -o pipefail

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
WHITE='\033[1;37m'
NC='\033[0m'

LOG_FILE="/tmp/seedphrase_recovery_setup.log"
if ! touch "$LOG_FILE" 2>/dev/null; then
    LOG_FILE="$HOME/seedphrase_recovery_setup.log"
    touch "$LOG_FILE" || { echo "Error: Cannot create log file."; exit 1; }
fi

show_header() {
    clear
    echo ""
    
    local line1="   ███████╗███████╗███████╗██████╗ "
    local line2="   ██╔════╝██╔════╝██╔════╝██╔══██╗"
    local line3="   ███████╗█████╗  █████╗  ██║  ██║"
    local line4="   ╚════██║██╔══╝  ██╔══╝  ██║  ██║"
    local line5="   ███████║███████╗███████╗██████╔╝"
    local line6="   ╚══════╝╚══════╝╚══════╝╚═════╝ "
    
    local line7="   ██████╗ ███████╗ ██████╗ ██████╗ ██╗   ██╗███████╗██████╗ ██╗   ██╗"
    local line8="   ██╔══██╗██╔════╝██╔════╝██╔═══██╗██║   ██║██╔════╝██╔══██╗╚██╗ ██╔╝"
    local line9="   ██████╔╝█████╗  ██║     ██║   ██║██║   ██║█████╗  ██████╔╝ ╚████╔╝ "
    local line10="   ██╔══██╗██╔══╝  ██║     ██║   ██║╚██╗ ██╔╝██╔══╝  ██╔══██╗  ╚██╔╝  "
    local line11="   ██║  ██║███████╗╚██████╗╚██████╔╝ ╚████╔╝ ███████╗██║  ██║   ██║   "
    local line12="   ╚═╝  ╚═╝╚══════╝ ╚═════╝ ╚═════╝   ╚═══╝  ╚══════╝╚═╝  ╚═╝   ╚═╝   "

    echo -e "${CYAN}$line1${NC}"; sleep 0.03
    echo -e "${CYAN}$line2${NC}"; sleep 0.03
    echo -e "${CYAN}$line3${NC}"; sleep 0.03
    echo -e "${CYAN}$line4${NC}"; sleep 0.03
    echo -e "${CYAN}$line5${NC}"; sleep 0.03
    echo -e "${CYAN}$line6${NC}"; sleep 0.03
    
    echo ""
    
    echo -e "${CYAN}$line7${NC}"; sleep 0.03
    echo -e "${CYAN}$line8${NC}"; sleep 0.03
    echo -e "${CYAN}$line9${NC}"; sleep 0.03
    echo -e "${CYAN}$line10${NC}"; sleep 0.03
    echo -e "${CYAN}$line11${NC}"; sleep 0.03
    echo -e "${CYAN}$line12${NC}"; sleep 0.03

    echo ""
    echo -ne "      ${WHITE}Built by ${NC}"
    
    local name="Zun"
    for (( i=0; i<${#name}; i++ )); do
        echo -ne "${MAGENTA}${name:$i:1}${NC}"
        sleep 0.1
    done
    echo ""
    echo ""
    echo -e "${YELLOW}  ==================================================================${NC}"
    echo ""
}

show_progress() {
    local current=$1
    local total=$2
    local width=40
    local percentage=$((current * 100 / total))
    local filled=$((percentage * width / 100))
    local empty=$((width - filled))
    
    local bar=""
    if [ $filled -gt 0 ]; then
        bar=$(printf "%0.s#" $(seq 1 $filled))
    fi
    local space=""
    if [ $empty -gt 0 ]; then
        space=$(printf "%0.s-" $(seq 1 $empty))
    fi
    
    echo -e "${CYAN}[${bar}${space}] ${percentage}%${NC}"
}

run_silent() {
    "$@" > "$LOG_FILE" 2>&1
    local status=$?
    
    if [ $status -ne 0 ]; then
        echo -e "${RED}Failed!${NC}"
        echo -e "${RED}An error occurred. Details from $LOG_FILE:${NC}"
        echo "----------------------------------------"
        tail -n 20 "$LOG_FILE"
        echo "----------------------------------------"
        exit $status
    fi
}

detect_os() {
    case "$(uname -s)" in
        Linux*)     os=Linux;;
        Darwin*)    os=Mac;;
        CYGWIN*)    os=Windows;;
        MINGW*)     os=Windows;;
        MSYS*)      os=Windows;;
        *)          os="UNKNOWN:${uname -s}";;
    esac
    echo $os
}

show_header

echo -e "${GREEN}Starting Setup...${NC}"
OS=$(detect_os)
echo -e "${CYAN}Detected OS: $OS${NC}"
echo -e "${CYAN}Logs are being written to: $LOG_FILE${NC}"
echo ""

if command -v sudo &> /dev/null; then
    if [ "$EUID" -ne 0 ]; then
        sudo -v || { echo -e "${RED}Sudo privileges required for dependency installation.${NC}"; exit 1; }
    fi
fi

TOTAL_STEPS=4
CURRENT_STEP=0

install_rust() {
    if ! command -v cargo &> /dev/null; then
        echo "Installing Rust..."
        export RUSTUP_HOME=${RUSTUP_HOME:-$HOME/.rustup}
        export CARGO_HOME=${CARGO_HOME:-$HOME/.cargo}
        
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
        
        if [ -f "$CARGO_HOME/env" ]; then
            source "$CARGO_HOME/env"
        fi
    else
        echo "Rust is already installed."
    fi
}

install_foundry() {
    if ! command -v forge &> /dev/null; then
        echo "Installing Foundry..."
        export FOUNDRY_DIR=${FOUNDRY_DIR:-$HOME/.foundry}
        
        curl -L https://foundry.paradigm.xyz | bash
        
        export PATH="$FOUNDRY_DIR/bin:$PATH"
        
        if [ -f "$FOUNDRY_DIR/bin/foundryup" ]; then
            "$FOUNDRY_DIR/bin/foundryup"
        fi
    else
        echo "Foundry is already installed."
    fi
}

install_linux_deps() {
    echo "Installing Linux dependencies..."
    
    if command -v apt-get &> /dev/null; then
        PKG_MGR="apt-get"
        INSTALL_CMD="sudo apt-get install -y"
        UPDATE_CMD="sudo apt-get update"
        PKGS="build-essential libssl-dev pkg-config curl git"
    elif command -v dnf &> /dev/null; then
        PKG_MGR="dnf"
        INSTALL_CMD="sudo dnf install -y"
        UPDATE_CMD="sudo dnf check-update"
        PKGS="@development-tools openssl-devel curl git"
    elif command -v yum &> /dev/null; then
        PKG_MGR="yum"
        INSTALL_CMD="sudo yum install -y"
        UPDATE_CMD="sudo yum check-update"
        PKGS="openssl-devel curl git"
    elif command -v pacman &> /dev/null; then
        PKG_MGR="pacman"
        INSTALL_CMD="sudo pacman -S --noconfirm"
        UPDATE_CMD="sudo pacman -Sy"
        PKGS="base-devel openssl curl git"
    elif command -v apk &> /dev/null; then
        PKG_MGR="apk"
        INSTALL_CMD="sudo apk add"
        UPDATE_CMD="sudo apk update"
        PKGS="build-base openssl-dev curl git"
    else
        echo "Error: Could not detect a supported package manager (apt, dnf, yum, pacman, apk)."
        exit 1
    fi

    echo "Using package manager: $PKG_MGR"
    $UPDATE_CMD || true
    $INSTALL_CMD $PKGS
}

add_to_shell_config() {
    local line="$1"
    local shell_config=""
    
    if [ -n "$BASH_VERSION" ]; then
        shell_config="$HOME/.bashrc"
    elif [ -n "$ZSH_VERSION" ]; then
        shell_config="$HOME/.zshrc"
    else
        shell_config="$HOME/.profile"
    fi
    
    if [ -f "$shell_config" ]; then
        if ! grep -qF "$line" "$shell_config"; then
            echo "$line" >> "$shell_config"
        fi
    fi
}

if [ "$OS" == "Linux" ]; then
    echo -e "${YELLOW}Step 1/$TOTAL_STEPS: System Dependencies${NC}"
    run_silent install_linux_deps
    CURRENT_STEP=1
    show_progress $CURRENT_STEP $TOTAL_STEPS

    if [ ! -d ".git" ] && [ ! -f "Cargo.toml" ]; then
        echo -e "${YELLOW}Cloning repository...${NC}"
        REPO_URL="https://github.com/zunxbt/seedphrase-recovery-tool.git"
        
        if [ -d "seedphrase-recovery-tool" ]; then
            echo -e "${YELLOW}Directory 'seedphrase-recovery-tool' exists. Removing and re-cloning...${NC}"
            rm -rf seedphrase-recovery-tool
        fi
        
        run_silent git clone "$REPO_URL"
        cd seedphrase-recovery-tool || exit 1
    fi
    
    echo -e "${YELLOW}Step 2/$TOTAL_STEPS: Rust Toolchain${NC}"
    run_silent install_rust
    CURRENT_STEP=2
    show_progress $CURRENT_STEP $TOTAL_STEPS
    
    echo -e "${YELLOW}Step 3/$TOTAL_STEPS: Foundry${NC}"
    run_silent install_foundry
    CURRENT_STEP=3
    show_progress $CURRENT_STEP $TOTAL_STEPS
    
    add_to_shell_config 'source "$HOME/.cargo/env"'
    
else
    echo -e "${RED}Unsupported OS: $OS. This script currently supports Linux only.${NC}"
    exit 1
fi

echo -e "${YELLOW}Step 4/$TOTAL_STEPS: Project Dependencies${NC}"
export PATH="$HOME/.cargo/bin:$PATH"

if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Error: Cargo not found even after installation attempt.${NC}"
    exit 1
fi

run_silent cargo fetch
CURRENT_STEP=4
show_progress $CURRENT_STEP $TOTAL_STEPS

echo ""
echo -e "${GREEN}Setup Complete!${NC}"
echo -e "You can now run the tool using: ${YELLOW}./bin/seedphrase_recovery${NC}"
if [ "$PWD" != "$OLDPWD" ]; then
    echo -e "(Note: You are currently in $(pwd). The tool is inside this directory.)"
fi
