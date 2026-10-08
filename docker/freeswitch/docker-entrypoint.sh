#!/bin/sh
# Renders environment-dependent variables for the bootstrap configuration and
# starts FreeSWITCH in the foreground.
set -eu

: "${TALKOPS_ESL_PASSWORD:?TALKOPS_ESL_PASSWORD must be set}"
: "${TALKOPS_XMLCURL_PASSWORD:?TALKOPS_XMLCURL_PASSWORD must be set}"

conf=/usr/local/freeswitch/etc/freeswitch
out="$conf/vars_env.xml"

# Values end up inside XML attributes: reject characters that would break them.
check() {
    case "$2" in
        *'"'* | *'<'* | *'>'* | *'&'*) echo "error: $1 must not contain \" < > or &" >&2; exit 1 ;;
    esac
}

{
    echo '<include>'
    for pair in \
        "talkops_esl_listen_ip=${TALKOPS_ESL_LISTEN_IP:-127.0.0.1}" \
        "talkops_esl_port=${TALKOPS_ESL_PORT:-8021}" \
        "talkops_esl_password=${TALKOPS_ESL_PASSWORD}" \
        "talkops_xmlcurl_url=${TALKOPS_XMLCURL_URL:-http://127.0.0.1:8080/fs/xml}" \
        "talkops_xmlcurl_password=${TALKOPS_XMLCURL_PASSWORD}" \
        "talkops_cdr_url=${TALKOPS_CDR_URL:-http://127.0.0.1:8080/fs/cdr}" \
        "talkops_rtp_start_port=${TALKOPS_RTP_START_PORT:-16384}" \
        "talkops_rtp_end_port=${TALKOPS_RTP_END_PORT:-16999}" \
        "talkops_max_sessions=${TALKOPS_MAX_SESSIONS:-200}" \
        "talkops_log_level=${TALKOPS_FS_LOG_LEVEL:-info}"
    do
        check "${pair%%=*}" "${pair#*=}"
        printf '  <X-PRE-PROCESS cmd="set" data="%s"/>\n' "$pair"
    done
    echo '</include>'
} > "$out"
chmod 600 "$out"

# Built-in hold music into the shared sounds volume, so TalkOps can offer the
# pieces for preview and select one (missing files only; never overwrite).
music=/var/lib/talkops/sounds/music
if mkdir -p "$music" 2>/dev/null; then
    for f in /usr/local/freeswitch/share/freeswitch/sounds/music/default/*.wav; do
        [ -e "$f" ] && [ ! -e "$music/${f##*/}" ] && cp "$f" "$music/" || true
    done
fi

if [ "$#" -gt 0 ]; then
    exec "$@"
fi
# -nonat: no UPnP/NAT-PMP probing; -np: normal scheduling priority (realtime
# needs CAP_SYS_NICE, which an unprivileged container does not have);
# -c: foreground with console log on stdout.
# stdbuf: line-buffer stdout so `docker logs` shows log lines immediately.
exec stdbuf -oL -eL /usr/local/freeswitch/bin/freeswitch -nonat -np -c
