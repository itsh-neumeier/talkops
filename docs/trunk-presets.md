# Trunk presets / Provider-Vorlagen

Presets describe how TalkOps connects to a SIP provider. They are data files
in [`presets/trunks/`](../presets/trunks), one per provider product, and are
loaded by the server at startup (`TALKOPS_PRESETS_DIR`). In the web UI you pick
a preset, enter your credentials and numbers – everything else comes from the
file. Every value of the `sip` section can be overridden per trunk.

*Deutsch: siehe unten.*

## Format

```yaml
id: leonet                    # = file name, a-z 0-9 '-'
name: LEONET                  # provider name
product: SIP-Anschluss        # product/variant
country: DE                   # ISO code, '*' for international providers
status: untested              # verified | community | untested
sources:                      # official documentation the values come from
  - https://…
notes:                        # shown in the UI
  en: …
  de: …
credentials:
  mode: per_number            # per_number | shared | no_registration
  username_template: "leo{e164_digits}"   # {e164} {e164_digits} {input}
  auth_username_template: ""  # empty = same as username
  username_hint: { en: …, de: … }
sip:
  registrar: sip.leovoice.online   # omit if every customer gets an individual one
  realm: …                    # optional
  proxy: …                    # optional
  outbound_proxy: …           # optional
  transport: udp              # udp | tcp | tls
  srtp: off                   # off | optional | required
  register: true
  expire_seconds: 600
  ping_seconds: 30            # OPTIONS keepalive, omit to disable
  number_format: national     # dialed number: e164 | e164_no_plus | international | national
  caller_id_format: e164      # own number in From/PAI/PPI (same values)
  caller_id_header: from      # from | pai | ppi
  from_user: username         # username | number
  from_domain: …              # optional, defaults to the registrar
  codecs: [G722, PCMA, PCMU]
  dtmf: rfc2833               # rfc2833 | info | inband
  t38: false
```

### Credential modes

| Mode | Meaning | Examples |
|---|---|---|
| `per_number` | Each number registers with its own credentials; TalkOps creates one gateway per number. | LEONET, Telekom IP-Anschluss, 1&1 |
| `shared` | One account carries all numbers of the trunk. | sipgate trunking, Telekom SIP-Trunk, easybell |
| `no_registration` | The provider authenticates by IP or digest on INVITE; inbound calls need a public address. | Twilio, Sunrise, Vonage |

### Status

- `verified` – tested end-to-end by maintainers (incoming, outgoing, CLIP, CLIR).
- `community` – reported working by users (link the report in `sources`).
- `untested` – derived from documentation only. **All presets start here.**

## Contributing a preset

1. Copy `generic.yaml` to `<provider>-<product>.yaml` and fill in the values
   **from the provider's official documentation** – never guess. Put the
   links into `sources`; if a value is unclear, leave the preset `untested`
   and explain in `notes`.
2. `cargo test -p talkops-core presets` validates every file.
3. Open a pull request. If you tested the preset with a real line, say what
   you tested; maintainers then raise the status to `community`.

---

## Deutsch

Vorlagen beschreiben, wie TalkOps sich mit einem SIP-Anbieter verbindet. Sie
liegen als Datendateien in `presets/trunks/` (eine Datei je Anbieterprodukt)
und werden beim Start geladen. Im Web-UI wählt man die Vorlage, trägt nur
Zugangsdaten und Rufnummern ein; alle Werte unter `sip` lassen sich pro Trunk
überschreiben.

**Neue Vorlage beitragen:** `generic.yaml` kopieren, Werte **ausschließlich aus
der offiziellen Anbieter-Doku** übernehmen und die Quellen unter `sources`
eintragen. Unklare Werte → Status `untested` und Hinweis in `notes`.
`cargo test -p talkops-core presets` prüft alle Dateien. Danach Pull Request
öffnen und angeben, was mit einem echten Anschluss getestet wurde.

**Zugangsmodelle:** `per_number` (jede Rufnummer mit eigenen Zugangsdaten,
z. B. LEONET), `shared` (ein Account für alle Rufnummern, klassischer
SIP-Trunk), `no_registration` (Authentifizierung über IP, z. B. Twilio).
