# Shell integration (OSC 7 + OSC 133)

VsTerm uses shell integration for:

- **OSC 7** — current working directory, so the file browser can sync from the terminal
- **OSC 133** — command marks for the left gutter (time, line number, fold)

## Automatic (recommended)

SSH sessions bootstrap this automatically for **Bash / Zsh / Fish** when **Shell integration** is enabled on the server (default on in Add/Edit Server):

1. VsTerm launches a temporary `/bin/sh -c` bootstrap with a PTY
2. The bootstrap writes short-lived scripts under `/tmp/vsterm-shell.*`
3. It starts the user's login shell with those hooks loaded for this session only
4. Temporary files are removed when the session ends

Nothing is written to `~/.bashrc`, `~/.zshrc`, or Fish config.

Turn the option off for a single host if its audit policy rejects remote bootstrap commands.

## Manual install

If you prefer permanent install, copy the scripts onto the host:

```bash
# Bash / Zsh
source ~/vsterm-osc133.sh

# Fish
source ~/vsterm.fish
```

## Verify

After connecting, change directory in the terminal and click “sync terminal path to file browser”. The file pane should follow `$PWD`.
