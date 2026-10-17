# Third-party provisioning sources

The directory `crates/talkops-provisioning/reference/fusionpbx/yealink` contains
all 433 Yealink template files from FusionPBX commit
`be0d0e3220e6b3666775fa46db6f1accc60c79c6`.

Source: https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink

The Original Code is FusionPBX. The Initial Developer is Mark J Crane.
The upstream `resources/licenses.php` identifies the Mozilla Public License
Version 1.1. A copy is in `reference/fusionpbx/licenses.php`. The complete
license is in `reference/fusionpbx/MPL-1.1.txt` in the provisioning crate.
These reference files are unchanged. Their Git blob IDs are recorded in
`docs/research/yealink-template-inventory.json`. The import date is 2026-10-16.

The contents of these reference files are subject to the Mozilla Public
License Version 1.1. You may obtain the License at
https://www.mozilla.org/MPL/1.1/. The software is provided on an AS IS basis,
without warranty of any kind. Refer to the included license for its terms.
The reference source remains available in this repository and the server's
authenticated template library. The server image includes this notice and
the upstream license files in `/usr/share/doc/talkops`.

The reference files are data. TalkOps does not execute upstream PHP or Smarty.
The native MiniJinja templates and application code are maintained separately
under the TalkOps project license. The source library records template
content; it does not certify a phone model or firmware.
