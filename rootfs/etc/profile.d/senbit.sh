# Senbit interactive shell defaults (read by login shells through /etc/profile).

# User-installed programs (curl ... | sh installers put them in ~/.local/bin):
# available as soon as the directory exists, in the next login.
if [ -d "$HOME/.local/bin" ]; then
    case ":$PATH:" in
        *":$HOME/.local/bin:"*) ;;
        *) PATH="$HOME/.local/bin:$PATH"; export PATH ;;
    esac
fi

case "$-" in
    *i*)
        # --- Prompt -------------------------------------------------------
        # senbit-login exports a colored prompt: PS1 for ash, and SENBIT_PS1_BASH
        # (the same prompt with the \[ \] markers bash needs to measure the
        # line). They are kept; a plain prompt is only used when none was given.
        if [ -n "$BASH_VERSION" ] && [ -n "$SENBIT_PS1_BASH" ]; then
            PS1="$SENBIT_PS1_BASH"
        elif [ -z "$PS1" ]; then
            if [ "$(id -u)" = 0 ]; then
                PS1='\u@\h:\w# '
            else
                PS1='\u@\h:\w$ '
            fi
        fi
        export PS1

        # --- Colors everywhere -------------------------------------------
        export SYSTEMD_COLORS=true      # systemctl, journalctl, systemd-analyze...
        export CLICOLOR=1
        alias ls='ls --color=auto'
        alias ll='ls -l'
        alias la='ls -la'
        alias grep='grep --color=auto'
        alias egrep='grep -E --color=auto'
        alias fgrep='grep -F --color=auto'
        alias diff='diff --color=auto'

        # --- Pager --------------------------------------------------------
        # GNU less (package `less`): short outputs are printed directly instead
        # of being shown in a pager full of "~" lines (BusyBox less).
        export PAGER=less
        export LESS='-FRX'
        export SYSTEMD_LESS='FRSXMK'
        ;;
esac
