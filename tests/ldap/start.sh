#!/usr/bin/env bash
# Starts a throwaway OpenLDAP server with test users for the LDAP login tests
# (needs the Debian/Ubuntu packages slapd and ldap-utils):
#
#   tests/ldap/start.sh            # listens on ldap://127.0.0.1:3890
#   TALKOPS_TEST_LDAP_URL=ldap://127.0.0.1:3890 cargo test -p talkops-api --test ldap
set -euo pipefail
DIR=$(cd "$(dirname "$0")" && pwd)
WORK=${LDAP_WORK:-$(mktemp -d)}
PORT=${LDAP_PORT:-3890}
mkdir -p "$WORK/db"
cat > "$WORK/slapd.conf" <<CONF
include /etc/ldap/schema/core.schema
include /etc/ldap/schema/cosine.schema
include /etc/ldap/schema/inetorgperson.schema
include /etc/ldap/schema/nis.schema
modulepath /usr/lib/ldap
moduleload back_mdb
moduleload memberof
pidfile $WORK/slapd.pid
database mdb
suffix "dc=example,dc=org"
rootdn "cn=admin,dc=example,dc=org"
rootpw adminpw
directory $WORK/db
overlay memberof
memberof-group-oc groupOfNames
memberof-member-ad member
memberof-memberof-ad memberOf
memberof-refint true
access to attrs=userPassword by self write by anonymous auth by * none
access to * by dn.exact="cn=svc,ou=people,dc=example,dc=org" read by self read by * none
CONF
slapd -f "$WORK/slapd.conf" -h "ldap://127.0.0.1:$PORT/"
for _ in $(seq 1 20); do
    ldapsearch -x -H "ldap://127.0.0.1:$PORT" -b "" -s base >/dev/null 2>&1 && break
    sleep 0.5
done
ldapadd -x -H "ldap://127.0.0.1:$PORT" -D "cn=admin,dc=example,dc=org" -w adminpw -f "$DIR/data.ldif" >/dev/null
echo "OpenLDAP test server on ldap://127.0.0.1:$PORT (data in $WORK)"
