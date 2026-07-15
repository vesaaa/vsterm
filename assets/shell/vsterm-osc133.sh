# VsTerm shell integration (OSC 133)
#
# Emits Final-Term / VS Code compatible marks so VsTerm can show wall-clock
# times, command line numbers, and fold/collapse for command output.
#
# Without this script, the left gutter stays empty — no fake timestamps.
#
# Install (bash/zsh):
#   echo 'source /path/to/vsterm-osc133.sh' >> ~/.bashrc   # or ~/.zshrc
#
# Optional: set VSTERM_SHELL_INTEGRATION=0 to disable after sourcing.

if [ "${VSTERM_SHELL_INTEGRATION:-1}" = "0" ]; then
  return 0 2>/dev/null || true
fi

# Avoid double-install when re-sourced.
if [ -n "${__VSTERM_OSC133:-}" ]; then
  return 0 2>/dev/null || true
fi
__VSTERM_OSC133=1

__vsterm_osc() {
  printf '\033]%s\007' "$1"
}

__vsterm_precmd() {
  local status=$?
  if [ -n "${__vsterm_cmd_active:-}" ]; then
    __vsterm_osc "133;D;${status}"
    unset __vsterm_cmd_active
  fi
  __vsterm_osc "133;A"
}

__vsterm_preexec() {
  # bash DEBUG also runs for PROMPT_COMMAND; ignore those.
  if [ -n "${__vsterm_in_prompt:-}" ]; then
    return 0
  fi
  if [ -z "${__vsterm_cmd_active:-}" ]; then
    __vsterm_cmd_active=1
    __vsterm_osc "133;C"
  fi
}

if [ -n "${BASH_VERSION-}" ]; then
  __vsterm_prompt_wrap() {
    __vsterm_in_prompt=1
    __vsterm_precmd
    unset __vsterm_in_prompt
  }
  if [[ ! "${PROMPT_COMMAND-}" =~ __vsterm_prompt_wrap ]]; then
    if [ -n "${PROMPT_COMMAND-}" ]; then
      PROMPT_COMMAND="__vsterm_prompt_wrap; ${PROMPT_COMMAND}"
    else
      PROMPT_COMMAND="__vsterm_prompt_wrap"
    fi
  fi
  # Prefer preexec hooks from bash-preexec / ble.sh when available.
  if declare -F preexec >/dev/null 2>&1; then
    :
  else
    if ! trap -p DEBUG 2>/dev/null | grep -q __vsterm_preexec; then
      trap '__vsterm_preexec' DEBUG
    fi
  fi
elif [ -n "${ZSH_VERSION-}" ]; then
  autoload -Uz add-zsh-hook 2>/dev/null || true
  if typeset -f add-zsh-hook >/dev/null 2>&1; then
    add-zsh-hook precmd __vsterm_precmd
    add-zsh-hook preexec __vsterm_preexec
  else
    precmd_functions+=(__vsterm_precmd)
    preexec_functions+=(__vsterm_preexec)
  fi
fi
