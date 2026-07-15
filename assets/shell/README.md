# Shell integration (OSC 133)

VsTerm’s command gutter (wall-clock time, command line number, fold `−`/`+`, boxed `···`) requires **OSC 133** marks from the remote shell. Without them, the gutter stays blank — times are not invented.

## Enable

Copy [`vsterm-osc133.sh`](vsterm-osc133.sh) onto the host and source it from `~/.bashrc` or `~/.zshrc`:

```bash
source ~/vsterm-osc133.sh
```

Open a new SSH session (or `source` again) and run a command. Headers should look like:

```text
[16:13:52]  16 −  ip a
```

Click `−` to fold output; the header becomes:

```text
[16:13:52]  16 +  ip a  [···]
```

## Disable

```bash
export VSTERM_SHELL_INTEGRATION=0
```

before sourcing, or remove the `source` line.
