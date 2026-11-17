# Senbit: boot report at every login.
#
# senbit-init no longer stops the boot when a non-critical step fails (network,
# hostname, logs, keyboard): it records the result and starts the system anyway.
# This box recalls what worked and what did not. It is printed once, so it stays
# on screen (and in the scrollback) until you run `clear`.
#
#   bootreport            details
#   bootreport --logs     systemd logs of this boot
#
# Only for a terminal: never in scripts or `sh -c`.
if [ -t 1 ] && [ -x /usr/bin/bootreport ]; then
    /usr/bin/bootreport --banner
fi
