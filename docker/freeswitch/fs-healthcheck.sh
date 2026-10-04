#!/bin/sh
# Healthy when FreeSWITCH answers on the event socket and reports "UP".
/usr/local/freeswitch/bin/fs_cli \
    -H "${TALKOPS_ESL_LISTEN_IP:-127.0.0.1}" -P "${TALKOPS_ESL_PORT:-8021}" -p "${TALKOPS_ESL_PASSWORD}" \
    -T 3000 -t 3000 -x status 2>/dev/null | grep -q '^UP '
