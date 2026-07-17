//! Per-session remote shell integration bootstrap.
//!
//! The bootstrap is passed as the SSH remote command, before the interactive
//! shell exists. It never writes into the user's command line or startup files.

use uuid::Uuid;

const POSIX_INTEGRATION: &str = include_str!("../../../assets/shell/vsterm-osc133.sh");
const FISH_INTEGRATION: &str = include_str!("../../../assets/shell/vsterm.fish");

/// Remote command executed under a forced `/bin/sh -c`, so Fish/Zsh login shells
/// still receive a POSIX bootstrap that can hand off to the real interactive shell.
pub(crate) fn remote_bootstrap_command() -> String {
    let nonce = Uuid::new_v4().simple().to_string();
    let script = bootstrap_script(&nonce);
    format!("/bin/sh -c {}", shell_single_quote(&script))
}

fn bootstrap_script(nonce: &str) -> String {
    // Windows checkouts / builds may embed CRLF via autocrlf. Remote POSIX shells
    // treat `\r` as syntax (`do\r`, `$'\r': command not found`), so always emit LF.
    let posix = shell_single_quote(&normalize_unix_newlines(POSIX_INTEGRATION));
    let fish = shell_single_quote(&normalize_unix_newlines(FISH_INTEGRATION));
    let nonce = shell_single_quote(nonce);

    normalize_unix_newlines(
        &format!(
            r#"umask 077
_vsterm_shell=${{SHELL:-/bin/sh}}
_vsterm_name=`basename "$_vsterm_shell" 2>/dev/null || echo sh`
_vsterm_tmp=`mktemp -d "${{TMPDIR:-/tmp}}/vsterm-shell.XXXXXXXX" 2>/dev/null` || {{
  printf '\033]133;P;VSTERM;FAILED;stage\007'
  exec "$_vsterm_shell" -l
}}
_vsterm_cleanup() {{ rm -rf -- "$_vsterm_tmp"; }}
trap '_vsterm_cleanup' EXIT HUP INT TERM
printf '%s' {posix} >"$_vsterm_tmp/vsterm.sh" || exit 125
printf '%s' {fish} >"$_vsterm_tmp/vsterm.fish" || exit 125
chmod 600 "$_vsterm_tmp/vsterm.sh" "$_vsterm_tmp/vsterm.fish"
export VSTERM_READY_NONCE={nonce}
export VSTERM_TMP="$_vsterm_tmp"

case "$_vsterm_name" in
  bash)
    VSTERM_BASH_INJECT=1 ENV="$_vsterm_tmp/vsterm.sh" \
      "$_vsterm_shell" --posix -l
    ;;
  zsh)
    _vsterm_orig_zdotdir=${{ZDOTDIR:-$HOME}}
    cat >"$_vsterm_tmp/.zshenv" <<'VSTERM_ZSHENV'
if [ -r "$VSTERM_ORIG_ZDOTDIR/.zshenv" ]; then
  . "$VSTERM_ORIG_ZDOTDIR/.zshenv"
fi
if [ -n "$ZDOTDIR" ] && [ "$ZDOTDIR" != "$VSTERM_ZDOTDIR" ]; then
  export VSTERM_ORIG_ZDOTDIR="$ZDOTDIR"
fi
export ZDOTDIR="$VSTERM_ZDOTDIR"
VSTERM_ZSHENV
    cat >"$_vsterm_tmp/.zprofile" <<'VSTERM_ZPROFILE'
[ -r "$VSTERM_ORIG_ZDOTDIR/.zprofile" ] && . "$VSTERM_ORIG_ZDOTDIR/.zprofile"
VSTERM_ZPROFILE
    cat >"$_vsterm_tmp/.zshrc" <<'VSTERM_ZSHRC'
[ -r "$VSTERM_ORIG_ZDOTDIR/.zshrc" ] && . "$VSTERM_ORIG_ZDOTDIR/.zshrc"
. "$VSTERM_ZDOTDIR/vsterm.sh"
VSTERM_ZSHRC
    cat >"$_vsterm_tmp/.zlogin" <<'VSTERM_ZLOGIN'
[ -r "$VSTERM_ORIG_ZDOTDIR/.zlogin" ] && . "$VSTERM_ORIG_ZDOTDIR/.zlogin"
VSTERM_ZLOGIN
    cat >"$_vsterm_tmp/.zlogout" <<'VSTERM_ZLOGOUT'
[ -r "$VSTERM_ORIG_ZDOTDIR/.zlogout" ] && . "$VSTERM_ORIG_ZDOTDIR/.zlogout"
VSTERM_ZLOGOUT
    VSTERM_ORIG_ZDOTDIR="$_vsterm_orig_zdotdir" \
      VSTERM_ZDOTDIR="$_vsterm_tmp" ZDOTDIR="$_vsterm_tmp" \
      "$_vsterm_shell" -l
    ;;
  fish)
    "$_vsterm_shell" -l -C "source '$_vsterm_tmp/vsterm.fish'"
    ;;
  *)
    printf '\033]133;P;VSTERM;UNSUPPORTED;%s\007' "$_vsterm_name"
    exec "$_vsterm_shell" -l
    ;;
esac
_vsterm_status=$?
_vsterm_cleanup
trap - EXIT HUP INT TERM
exit "$_vsterm_status"
"#
        ),
    )
}

/// Strip CR so remote `/bin/sh` / bash never see Windows CRLF line endings.
fn normalize_unix_newlines(value: &str) -> String {
    value.replace('\r', "")
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_single_quotes_for_posix_shell() {
        assert_eq!(shell_single_quote("a'b"), "'a'\\''b'");
    }

    #[test]
    fn bootstrap_forces_posix_sh_and_supports_three_shells() {
        let command = remote_bootstrap_command();
        assert!(command.starts_with("/bin/sh -c "));
        assert!(command.contains("mktemp -d"));
        assert!(command.contains("bash)"));
        assert!(command.contains("zsh)"));
        assert!(command.contains("fish)"));
        assert!(!command.contains(".bashrc\" >>"));
        assert!(!command.contains(".zshrc\" >>"));
    }

    #[test]
    fn bootstrap_emits_unix_newlines_only() {
        let command = remote_bootstrap_command();
        assert!(
            !command.contains('\r'),
            "remote bootstrap must not contain CR (Windows CRLF breaks bash on Linux)"
        );
        assert!(command.contains('\n'));
        assert!(command.contains("for __vsterm_rc in"));
        assert!(!command.contains("do\r"));
    }

    #[test]
    fn normalize_unix_newlines_strips_cr() {
        assert_eq!(normalize_unix_newlines("a\r\nb\r\nc\n"), "a\nb\nc\n");
        assert_eq!(normalize_unix_newlines("do\r\n"), "do\n");
        assert_eq!(normalize_unix_newlines("alone\r"), "alone");
    }
}
