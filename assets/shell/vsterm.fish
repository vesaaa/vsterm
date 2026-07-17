# VsTerm session-scoped shell integration for Fish (OSC 7 + OSC 133).

if not status --is-interactive
    return
end

if set -q __VSTERM_SHELL_INTEGRATION
    return
end
set -g __VSTERM_SHELL_INTEGRATION 1

function __vsterm_osc --argument payload
    printf '\e]%s\a' "$payload"
end

function __vsterm_report_cwd
    set -l encoded (string escape --style=url "$PWD")
    set -l host $hostname
    if test -z "$host"
        set host localhost
    end
    __vsterm_osc "7;file://$host$encoded"
end

function __vsterm_prompt --on-event fish_prompt
    if set -q VSTERM_READY_NONCE
        __vsterm_osc "133;P;VSTERM;READY;$VSTERM_READY_NONCE"
        set -e VSTERM_READY_NONCE
    end
    __vsterm_report_cwd
    __vsterm_osc '133;A'
end

function __vsterm_chpwd --on-variable PWD
    status --is-command-substitution; and return
    __vsterm_report_cwd
end
