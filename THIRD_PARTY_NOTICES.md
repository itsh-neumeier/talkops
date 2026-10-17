# Third-party provisioning sources

The directory `crates/talkops-provisioning/reference/fusionpbx` contains all
1,037 provisioning source files from FusionPBX commit
`20143aec4366afc2755f336d3058243479d49d44`. This includes the 433 Yealink files
previously imported at `be0d0e3220e6b3666775fa46db6f1accc60c79c6`.
All 433 Yealink blobs have the same bytes at both commits.

Source: https://github.com/fusionpbx/fusionpbx/tree/20143aec4366afc2755f336d3058243479d49d44/resources/templates/provision

The Original Code is FusionPBX. The Initial Developer is Mark J Crane.
The upstream `resources/licenses.php` identifies the Mozilla Public License
Version 1.1. A copy is in `reference/fusionpbx/licenses.php`. The complete
license is in `reference/fusionpbx/MPL-1.1.txt` in the provisioning crate.
These reference files are unchanged. Their Git blob IDs are recorded in
`docs/research/fusionpbx-provisioning-files.json`. The complete import date is
2026-10-17. The earlier Yealink inventory remains available.

The contents of these reference files are subject to the Mozilla Public
License Version 1.1. You may obtain the License at
https://www.mozilla.org/MPL/1.1/. The software is provided on an AS IS basis,
without warranty of any kind. Refer to the included license for its terms.
The reference source remains available in this repository and the server's
authenticated template library. The server image includes this notice and
the upstream license files in `/usr/share/doc/talkops`.

The reference files are data. TalkOps does not execute upstream PHP or Smarty.
The files under `crates/talkops-provisioning/templates/fusionpbx` are modified
derivatives of these sources. They retain the MPL-1.1 terms. Each file identifies
its source commit, path and blob. The build scripts record and reproduce the
conversion and TalkOps-specific changes. These derivative files are available
in this repository and through authenticated native-template copies.
The independently maintained files under `templates/yealink` and the application
code use the TalkOps project license. The source library records template
content; it does not certify a phone model or firmware.
