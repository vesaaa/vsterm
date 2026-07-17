# VsTerm shell integration for Bash and Zsh (OSC 7 + OSC 133).
# This file can be sourced manually, and is also embedded in VsTerm's
# session-scoped SSH bootstrap. It never writes shell startup files.

if [ "${VSTERM_SHELL_INTEGRATION:-1}" = "0" ]; then
  return 0 2>/dev/null || true
fi

# Bash is started in POSIX mode for automatic injection so ENV can source this
# file before the first prompt. Recreate Bash's normal login startup sequence,
# then leave POSIX mode before installing hooks.
if [ -n "${BASH_VERSION-}" ] && [ "${VSTERM_BASH_INJECT:-0}" = "1" ]; then
  unset VSTERM_BASH_INJECT ENV
  set +o posix
  shopt -u inherit_errexit 2>/dev/null || true
  if shopt -q login_shell; then
    [ -r /etc/profile ] && . /etc/profile
    for __vsterm_rc in "$HOME/.bash_profile" "$HOME/.bash_login" "$HOME/.profile"; do
      if [ -r "$__vsterm_rc" ]; then
        . "$__vsterm_rc"
        break
      fi
    done
  else
    [ -r /etc/bash.bashrc ] && . /etc/bash.bashrc
    [ -r "$HOME/.bashrc" ] && . "$HOME/.bashrc"
  fi
  unset __vsterm_rc
fi

if [ -n "${__VSTERM_SHELL_INTEGRATION:-}" ]; then
  return 0 2>/dev/null || true
fi
__VSTERM_SHELL_INTEGRATION=1

__vsterm_osc() {
  printf '\033]%s\007' "$1"
}

__vsterm_urlencode_path() {
  # Portable enough for Bash/Zsh interactive shells. Encode everything outside
  # the unreserved URI path set so spaces / Unicode / percent signs survive.
  local input="$1" output="" char hex i=0
  local old_lc="${LC_ALL-}"
  LC_ALL=C
  while [ "$i" -lt "${#input}" ]; do
    char="${input:$i:1}"
    case "$char" in
      [a-zA-Z0-9/._~-]) output="${output}${char}" ;;
      *)
        hex=$(printf '%%%02X' "'$char")
        output="${output}${hex}"
        ;;
    esac
    i=$((i + 1))
  done
  if [ -n "$old_lc" ]; then LC_ALL="$old_lc"; else unset LC_ALL; fi
  printf '%s' "$output"
}

__vsterm_report_cwd() {
  local encoded
  encoded="$(__vsterm_urlencode_path "$PWD")"
  __vsterm_osc "7;file://${HOSTNAME:-localhost}${encoded}"
}

__vsterm_ready() {
  if [ -n "${VSTERM_READY_NONCE:-}" ]; then
    __vsterm_osc "133;P;VSTERM;READY;${VSTERM_READY_NONCE}"
    unset VSTERM_READY_NONCE
  fi
}

__vsterm_precmd() {
  __vsterm_ready
  __vsterm_report_cwd
  __vsterm_osc "133;A"
}

if [ -n "${BASH_VERSION-}" ]; then
  if [[ ";${PROMPT_COMMAND[*]-};" != *";__vsterm_precmd;"* ]]; then
    if [[ "$(declare -p PROMPT_COMMAND 2>/dev/null)" == "declare -a "* ]]; then
      PROMPT_COMMAND+=(__vsterm_precmd)
    elif [ -n "${PROMPT_COMMAND-}" ]; then
      PROMPT_COMMAND="${PROMPT_COMMAND%;};__vsterm_precmd"
    else
      PROMPT_COMMAND="__vsterm_precmd"
    fi
  fi
elif [ -n "${ZSH_VERSION-}" ]; then
  autoload -Uz add-zsh-hook 2>/dev/null || true
  if typeset -f add-zsh-hook >/dev/null 2>&1; then
    add-zsh-hook precmd __vsterm_precmd
  else
    precmd_functions+=(__vsterm_precmd)
  fi
fi
