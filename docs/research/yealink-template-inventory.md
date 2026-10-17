# FusionPBX Yealink key template inventory

Status: Research for review. No application change is approved.

Source revision: [be0d0e3220e6b3666775fa46db6f1accc60c79c6](https://github.com/fusionpbx/fusionpbx/commit/be0d0e3220e6b3666775fa46db6f1accc60c79c6).

Inspected all 433 files in all 70 Yealink directories.
These directories include model families, conference phones and DECT bases.
Directory names are not a count of tested phone models.

The scan reads template assignments, key loops and key comments.
It does not execute PHP or Smarty. It does not render a phone configuration.
The JSON companion records every file, upstream Git blob and key parameter pattern.
It records the first source line for each pattern. Numeric slots become `{slot}`.

To reproduce, check out the pinned FusionPBX revision with the Yealink tree
and `app/devices/app_config.php`. Then run:

```sh
node docs/research/survey-yealink-templates.cjs /path/to/fusionpbx
```

## Evidence rules

- A table entry means that the template contains an active parameter assignment.
- An empty assignment is evidence of a template field, not an active phone feature.
- A loop can accept key IDs that the model does not have. A loop proves no key count.
- Literal slots and comment ranges can be copied from a different model.
- Do not use this table alone to enable a hardware capability in TalkOps.
- The function registry is vendor-wide. It is not a per-model allowed-function list.
- Comments and registry entries can disagree. Keep their evidence separate.

## Groups by directory

L = `linekey`; P = `programablekey`; M = `memorykey`; E = `expansion_module`.
S = custom softkey layout; H = defined handset keys; D = DSS icon assets;
A = alarm button parameters. A dash means no assignment was found.

| Directory | Files | L | P | M | E | S | H | D | A | FusionPBX key loops |
| --- | ---: | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| [ax83h](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/ax83h) | 6 | yes | yes | - | yes | - | - | - | yes | `programmable` |
| [ax86r](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/ax86r) | 6 | yes | yes | - | yes | - | - | - | yes | `programmable` |
| [cp860](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/cp860) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [cp920](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/cp920) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [cp925](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/cp925) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [cp960](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/cp960) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [cp965](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/cp965) | 3 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t19p](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t19p) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t20p](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t20p) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t21p](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t21p) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `memory`, `programmable` |
| [t22p](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t22p) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t23g](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t23g) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `memory`, `programmable` |
| [t23p](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t23p) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `memory`, `programmable` |
| [t26p](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t26p) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `memory`, `programmable` |
| [t27g](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t27g) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `expansion-3`, `expansion-4`, `expansion-5`, `expansion-6`, `line`, `memory`, `programmable` |
| [t27p](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t27p) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `memory`, `programmable` |
| [t28p](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t28p) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `memory`, `programmable` |
| [t29g](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t29g) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `expansion-3`, `expansion-4`, `expansion-5`, `expansion-6`, `line`, `memory`, `programmable` |
| [t2x](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t2x) | 10 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `expansion-3`, `expansion-4`, `expansion-5`, `expansion-6`, `line`, `memory`, `programmable` |
| [t31g](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t31g) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `memory`, `programmable` |
| [t31w](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t31w) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t32g](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t32g) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `memory`, `programmable` |
| [t33g](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t33g) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t34w](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t34w) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t38g](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t38g) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `memory`, `programmable` |
| [t40g](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t40g) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `memory`, `programmable` |
| [t40p](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t40p) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `memory`, `programmable` |
| [t41p](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t41p) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `memory`, `programmable` |
| [t41s](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t41s) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `expansion-3`, `expansion-4`, `expansion-5`, `expansion-6`, `line`, `memory`, `programmable` |
| [t42g](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t42g) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `memory`, `programmable` |
| [t42s](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t42s) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `memory`, `programmable` |
| [t42u](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t42u) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `memory`, `programmable` |
| [t43u](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t43u) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t44w](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t44w) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t46g](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t46g) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `expansion-3`, `expansion-4`, `expansion-5`, `expansion-6`, `line`, `memory`, `programmable` |
| [t46s](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t46s) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `expansion-3`, `expansion-4`, `expansion-5`, `expansion-6`, `line`, `memory`, `programmable` |
| [t46u](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t46u) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t48g](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t48g) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `expansion-3`, `expansion-4`, `expansion-5`, `expansion-6`, `line`, `memory`, `programmable` |
| [t48s](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t48s) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `expansion-3`, `expansion-4`, `expansion-5`, `expansion-6`, `line`, `memory`, `programmable` |
| [t48u](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t48u) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t49g](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t49g) | 6 | yes | yes | yes | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `memory`, `programmable` |
| [t4x](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t4x) | 21 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `expansion-3`, `expansion-4`, `expansion-5`, `expansion-6`, `line`, `programmable` |
| [t52s](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t52s) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t53](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t53) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t53w](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t53w) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t54s](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t54s) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t54w](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t54w) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t56a](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t56a) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t57w](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t57w) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t58a](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t58a) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t58v](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t58v) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t58w](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t58w) | 5 | yes | yes | - | yes | yes | - | yes | - | `expansion`, `line`, `programmable` |
| [t5x](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t5x) | 10 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t73u](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t73u) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t73w](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t73w) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t74u](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t74u) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t74w](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t74w) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t77u](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t77u) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t85w](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t85w) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t87w](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t87w) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t88v](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t88v) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [t88w](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t88w) | 6 | yes | yes | - | yes | yes | - | - | - | `expansion`, `expansion-1`, `expansion-2`, `line`, `programmable` |
| [vp530](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/vp530) | 6 | yes | - | yes | yes | - | - | - | - | `expansion`, `line`, `memory` |
| [vp59](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/vp59) | 6 | yes | yes | - | - | yes | - | yes | - | `line` |
| [w52p](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/w52p) | 5 | - | - | - | - | - | - | - | - | - |
| [w56p](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/w56p) | 5 | - | - | - | - | - | - | - | - | - |
| [w60b](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/w60b) | 5 | - | - | - | - | - | - | - | - | - |
| [w70b](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/w70b) | 5 | - | - | - | - | - | - | - | - | - |
| [w7xp](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/w7xp) | 5 | - | - | - | - | - | - | - | - | - |
| [w80](https://github.com/fusionpbx/fusionpbx/tree/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/w80) | 5 | - | - | - | - | - | yes | - | - | - |

## Vendor function registry

Source: [app/devices/app_config.php](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/app/devices/app_config.php#L1290).

The Yealink registry contains 49 entries. These entries need a model and key-group filter.

| Code | FusionPBX function ID |
| ---: | --- |
| 0 | `na` |
| 15 | `line` |
| 1 | `conference` |
| 2 | `forward` |
| 3 | `transfer` |
| 4 | `hold` |
| 5 | `dnd` |
| 6 | `redial` |
| 7 | `call_return` |
| 8 | `sms` |
| 9 | `call_pickup` |
| 10 | `call_park` |
| 11 | `dtmf` |
| 12 | `voicemail` |
| 13 | `speed_dial` |
| 14 | `intercom` |
| 16 | `blf` |
| 17 | `url` |
| 19 | `public_hold` |
| 20 | `private` |
| 21 | `shared_line` |
| 22 | `xml_group` |
| 23 | `group_pickup` |
| 24 | `paging` |
| 25 | `record` |
| 27 | `xml_browser` |
| 28 | `history` |
| 29 | `directory` |
| 30 | `menu` |
| 32 | `new_sms` |
| 33 | `status` |
| 34 | `hot_desking` |
| 35 | `url_record` |
| 38 | `ldap` |
| 39 | `blf_list` |
| 40 | `prefix` |
| 41 | `zero_sp_touch` |
| 42 | `acd` |
| 43 | `local_phonebook` |
| 44 | `broadsoft_phonebook` |
| 45 | `local_group` |
| 46 | `broadsoft_group` |
| 47 | `xml_directory` |
| 48 | `switch_account_up` |
| 49 | `switch_account_down` |
| 50 | `keypad_lock` |
| 66 | `paging_list` |
| 73 | `custom` |
| 77 | `mobile_line` |

## Per-directory details

Each section links the configuration files that contain key assignments.
The slot lists below are literal assignments in the source. They are not model capacities.
The JSON record links comment lines without copying the source comment text.
Type-code hints come from selected legacy type-list comments. An empty list is not a restriction.

### ax83h

- Parameter groups: line, programmable, expansion, alarm_button.
- Key loops: programmable.
- Softkey URL states: None found.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- Literal programmable type slots: 19.
- [y000000000180.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/ax83h/y000000000180.cfg#L404): line, programmable, expansion, alarm_button.

### ax86r

- Parameter groups: line, programmable, expansion, alarm_button.
- Key loops: programmable.
- Softkey URL states: None found.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- Literal programmable type slots: 19.
- [y000000000182.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/ax86r/y000000000182.cfg#L404): line, programmable, expansion, alarm_button.

### cp860

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 22, 23, 27, 28, 29, 30, 31, 32, 33, 40, 43, 44, 45, 46, 47, 50.
- Literal line type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27.
- [y000000000037.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/cp860/y000000000037.cfg#L941): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/cp860/%7B%24mac%7D.cfg#L3046): line.

### cp920

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 22, 23, 27, 28, 29, 30, 31, 32, 33, 40, 43, 44, 45, 46, 47, 50.
- Literal line type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27.
- [y000000000078.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/cp920/y000000000078.cfg#L939): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/cp920/%7B%24mac%7D.cfg#L575): line.

### cp925

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 22, 23, 27, 28, 29, 30, 31, 32, 33, 40, 43, 44, 45, 46, 47, 50.
- Literal line type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27.
- [y000000000148.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/cp925/y000000000148.cfg#L939): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/cp925/%7B%24mac%7D.cfg#L575): line.

### cp960

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 22, 23, 27, 28, 29, 30, 31, 32, 33, 40, 43, 44, 45, 46, 47, 50.
- Literal line type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27.
- [y000000000073.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/cp960/y000000000073.cfg#L940): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/cp960/%7B%24mac%7D.cfg#L575): line.

### cp965

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000143.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/cp965/y000000000143.cfg#L503): line, programmable, expansion, softkey_layout.

### t19p

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 14, 22, 23, 27, 28, 29, 30, 32, 33, 43.
- Literal line type slots: 1, 2, 3, 4, 5, 6.
- Literal memory type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10.
- [y000000000053.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t19p/y000000000053.cfg#L405): line, programmable, memory, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t19p/%7B%24mac%7D.cfg#L2037): line.

### t20p

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 14, 22, 23, 27, 28, 29, 30, 32, 33, 43.
- Literal line type slots: 1, 2, 3, 4, 5, 6.
- Literal memory type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10.
- [y000000000007.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t20p/y000000000007.cfg#L405): line, programmable, memory, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t20p/%7B%24mac%7D.cfg#L2039): line.

### t21p

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, memory, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 22, 23, 27, 28, 29, 30, 31, 32, 33, 40, 43, 44, 45, 46, 47, 50.
- Literal line type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27.
- [y000000000052.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t21p/y000000000052.cfg#L899): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t21p/%7B%24mac%7D.cfg#L3030): line, memory, expansion.

### t22p

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 14, 22, 23, 27, 28, 29, 30, 32, 33, 43.
- Literal line type slots: 1, 2, 3, 4, 5, 6.
- Literal memory type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10.
- [y000000000005.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t22p/y000000000005.cfg#L405): line, programmable, memory, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t22p/%7B%24mac%7D.cfg#L2037): line.

### t23g

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, memory, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 22, 23, 27, 28, 29, 30, 31, 32, 33, 40, 43, 44, 45, 46, 47, 50.
- Literal line type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27.
- [y000000000044.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t23g/y000000000044.cfg#L939): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t23g/%7B%24mac%7D.cfg#L2940): line, memory, expansion.

### t23p

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, memory, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 22, 23, 27, 28, 29, 30, 31, 32, 33, 40, 43, 44, 45, 46, 47, 50.
- Literal line type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27.
- [y000000000044.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t23p/y000000000044.cfg#L938): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t23p/%7B%24mac%7D.cfg#L2919): line, memory, expansion.

### t26p

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, memory, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 14, 22, 23, 27, 28, 29, 30, 32, 33, 43.
- Literal line type slots: 1, 2, 3, 4, 5, 6.
- Literal memory type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10.
- [y000000000004.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t26p/y000000000004.cfg#L405): line, programmable, memory, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t26p/%7B%24mac%7D.cfg#L2037): line, memory, expansion.

### t27g

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, expansion-3, expansion-4, expansion-5, expansion-6, line, memory, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 22, 23, 27, 28, 29, 30, 31, 32, 33, 40, 43, 44, 45, 46, 47, 50.
- Literal line type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27.
- [y000000000069.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t27g/y000000000069.cfg#L966): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t27g/%7B%24mac%7D.cfg#L2935): line, memory, expansion.

### t27p

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, memory, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 22, 23, 27, 28, 29, 30, 31, 32, 33, 40, 43, 44, 45, 46, 47, 50.
- Literal line type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27.
- [y000000000045.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t27p/y000000000045.cfg#L939): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t27p/%7B%24mac%7D.cfg#L2928): line, memory, expansion.

### t28p

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, memory, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 14, 22, 23, 27, 28, 29, 30, 32, 33, 43.
- Literal line type slots: 1, 2, 3, 4, 5, 6.
- Literal memory type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10.
- [y000000000000.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t28p/y000000000000.cfg#L405): line, programmable, memory, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t28p/%7B%24mac%7D.cfg#L2037): line, memory, expansion.

### t29g

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, expansion-3, expansion-4, expansion-5, expansion-6, line, memory, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 22, 23, 27, 28, 29, 30, 31, 32, 33, 40, 43, 44, 45, 46, 47, 50.
- Literal line type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27.
- [y000000000046.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t29g/y000000000046.cfg#L969): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t29g/%7B%24mac%7D.cfg#L2940): line, memory, expansion.

### t2x

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, expansion-3, expansion-4, expansion-5, expansion-6, line, memory, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 14, 22, 23, 27, 28, 29, 30, 31, 32, 33, 40, 43, 44, 45, 46, 47, 50.
- Literal line type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27.
- Literal memory type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10.
- [y000000000044.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t2x/y000000000044.cfg#L939): line, programmable, expansion, softkey_layout.
- [y000000000052.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t2x/y000000000052.cfg#L899): line, programmable, expansion, softkey_layout.
- [y000000000053.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t2x/y000000000053.cfg#L409): line, programmable, memory, expansion, softkey_layout.
- [y000000000069.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t2x/y000000000069.cfg#L938): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t2x/%7B%24mac%7D.cfg#L499): line, memory, expansion.

### t31g

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, memory, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 14, 22, 23, 27, 28, 29, 30, 32, 33, 43.
- Literal line type slots: 1, 2, 3, 4, 5, 6.
- Literal memory type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10.
- [y000000000123.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t31g/y000000000123.cfg#L405): line, programmable, memory, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t31g/%7B%24mac%7D.cfg#L1991): line, memory, expansion.

### t31w

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000172.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t31w/y000000000172.cfg#L484): line, programmable, expansion, softkey_layout.

### t32g

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, memory, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 14, 22, 23, 27, 28, 29, 30, 32, 33, 43.
- Literal line type slots: 1, 2, 3, 4, 5, 6.
- Literal memory type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10.
- [y000000000032.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t32g/y000000000032.cfg#L405): line, programmable, memory, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t32g/%7B%24mac%7D.cfg#L1991): line, memory, expansion.

### t33g

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000124.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t33g/y000000000124.cfg#L491): line, programmable, expansion, softkey_layout.

### t34w

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000171.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t34w/y000000000171.cfg#L486): line, programmable, expansion, softkey_layout.

### t38g

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, memory, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 14, 22, 23, 27, 28, 29, 30, 32, 33, 43.
- Literal line type slots: 1, 2, 3, 4, 5, 6.
- Literal memory type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10.
- [y000000000038.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t38g/y000000000038.cfg#L405): line, programmable, memory, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t38g/%7B%24mac%7D.cfg#L1991): line, memory, expansion.

### t40g

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, memory, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 22, 23, 27, 28, 29, 30, 31, 32, 33, 40, 43, 44, 45, 46, 47, 50.
- Literal line type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27.
- [y000000000076.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t40g/y000000000076.cfg#L941): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t40g/%7B%24mac%7D.cfg#L3064): line, memory, expansion.

### t40p

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, memory, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 22, 23, 27, 28, 29, 30, 31, 32, 33, 40, 43, 44, 45, 46, 47, 50.
- Literal line type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27.
- [y000000000054.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t40p/y000000000054.cfg#L940): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t40p/%7B%24mac%7D.cfg#L3066): line, memory, expansion.

### t41p

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, memory, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 22, 23, 27, 28, 29, 30, 31, 32, 33, 40, 43, 44, 45, 46, 47, 50.
- Literal line type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27.
- [y000000000036.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t41p/y000000000036.cfg#L940): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t41p/%7B%24mac%7D.cfg#L3028): line, memory, expansion.

### t41s

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, expansion-3, expansion-4, expansion-5, expansion-6, line, memory, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000068.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t41s/y000000000068.cfg#L453): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t41s/%7B%24mac%7D.cfg#L555): line, memory, expansion.

### t42g

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, memory, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 22, 23, 27, 28, 29, 30, 31, 32, 33, 40, 43, 44, 45, 46, 47, 50.
- Literal line type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27.
- [y000000000029.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t42g/y000000000029.cfg#L939): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t42g/%7B%24mac%7D.cfg#L2940): line, memory, expansion.

### t42s

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, memory, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000067.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t42s/y000000000067.cfg#L460): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t42s/%7B%24mac%7D.cfg#L2926): line, memory, expansion.

### t42u

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, memory, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 22, 23, 27, 28, 29, 30, 31, 32, 33, 40, 43, 44, 45, 46, 47, 50.
- Literal line type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27.
- [y000000000116.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t42u/y000000000116.cfg#L940): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t42u/%7B%24mac%7D.cfg#L2940): line, memory, expansion.

### t43u

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000107.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t43u/y000000000107.cfg#L495): line, programmable, expansion, softkey_layout.

### t44w

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000174.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t44w/y000000000174.cfg#L486): line, programmable, expansion, softkey_layout.

### t46g

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, expansion-3, expansion-4, expansion-5, expansion-6, line, memory, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 22, 23, 27, 28, 29, 30, 31, 32, 33, 40, 43, 44, 45, 46, 47, 50.
- Literal line type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27.
- [y000000000028.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t46g/y000000000028.cfg#L966): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t46g/%7B%24mac%7D.cfg#L2950): line, memory, expansion.

### t46s

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, expansion-3, expansion-4, expansion-5, expansion-6, line, memory, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000066.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t46s/y000000000066.cfg#L450): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t46s/%7B%24mac%7D.cfg#L557): line, memory, expansion.

### t46u

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000108.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t46u/y000000000108.cfg#L447): line, programmable, expansion, softkey_layout.

### t48g

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, expansion-3, expansion-4, expansion-5, expansion-6, line, memory, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 22, 23, 27, 28, 29, 30, 31, 32, 33, 40, 43, 44, 45, 46, 47, 50.
- Literal line type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27.
- [y000000000035.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t48g/y000000000035.cfg#L965): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t48g/%7B%24mac%7D.cfg#L2949): line, memory, expansion.

### t48s

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, expansion-3, expansion-4, expansion-5, expansion-6, line, memory, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000065.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t48s/y000000000065.cfg#L493): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t48s/%7B%24mac%7D.cfg#L3006): line, memory, expansion.

### t48u

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000109.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t48u/y000000000109.cfg#L483): line, programmable, expansion, softkey_layout.

### t49g

- Parameter groups: line, programmable, memory, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, memory, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 22, 23, 27, 28, 29, 30, 31, 32, 33, 40, 43, 44, 45, 46, 47, 50.
- Literal line type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27.
- [y000000000051.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t49g/y000000000051.cfg#L899): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t49g/%7B%24mac%7D.cfg#L3073): line, memory, expansion.

### t4x

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, expansion-3, expansion-4, expansion-5, expansion-6, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: 0, 2, 5, 6, 7, 8, 9, 13, 22, 23, 27, 28, 29, 30, 31, 32, 33, 40, 43, 44, 45, 46, 47, 50.
- Literal line type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27.
- [y000000000028.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t4x/y000000000028.cfg#L939): line, programmable, expansion, softkey_layout.
- [y000000000029.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t4x/y000000000029.cfg#L939): line, programmable, expansion, softkey_layout.
- [y000000000035.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t4x/y000000000035.cfg#L939): line, programmable, expansion, softkey_layout.
- [y000000000036.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t4x/y000000000036.cfg#L940): line, programmable, expansion, softkey_layout.
- [y000000000054.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t4x/y000000000054.cfg#L940): line, programmable, expansion, softkey_layout.
- [y000000000065.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t4x/y000000000065.cfg#L493): line, programmable, expansion, softkey_layout.
- [y000000000066.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t4x/y000000000066.cfg#L452): line, programmable, expansion, softkey_layout.
- [y000000000067.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t4x/y000000000067.cfg#L452): line, programmable, expansion, softkey_layout.
- [y000000000068.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t4x/y000000000068.cfg#L452): line, programmable, expansion, softkey_layout.
- [y000000000076.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t4x/y000000000076.cfg#L940): line, programmable, expansion, softkey_layout.
- [y000000000107.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t4x/y000000000107.cfg#L493): line, programmable, expansion, softkey_layout.
- [y000000000108.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t4x/y000000000108.cfg#L452): line, programmable, expansion, softkey_layout.
- [y000000000109.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t4x/y000000000109.cfg#L493): line, programmable, expansion, softkey_layout.
- [y000000000116.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t4x/y000000000116.cfg#L452): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t4x/%7B%24mac%7D.cfg#L508): line, expansion.

### t52s

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000074.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t52s/y000000000074.cfg#L460): line, programmable, expansion, softkey_layout.

### t53

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000095.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t53/y000000000095.cfg#L484): line, programmable, expansion, softkey_layout.

### t53w

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000095.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t53w/y000000000095.cfg#L483): line, programmable, expansion, softkey_layout.

### t54s

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000070.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t54s/y000000000070.cfg#L459): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t54s/%7B%24mac%7D.cfg#L727): line.

### t54w

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000096.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t54w/y000000000096.cfg#L484): line, programmable, expansion, softkey_layout.

### t56a

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 22, 23, 27, 28, 30, 31, 32.
- [y000000000056.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t56a/y000000000056.cfg#L1145): line, programmable, expansion, softkey_layout.

### t57w

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000097.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t57w/y000000000097.cfg#L483): line, programmable, expansion, softkey_layout.

### t58a

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 22, 23, 27, 28, 30, 31, 32.
- [y000000000058.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t58a/y000000000058.cfg#L1145): line, programmable, expansion, softkey_layout.

### t58v

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: 22, 23, 27, 28, 30, 31, 32.
- [y000000000058.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t58v/y000000000058.cfg#L1146): line, programmable, expansion, softkey_layout.

### t58w

- Parameter groups: line, programmable, expansion, softkey_layout, dss_assets.
- Key loops: expansion, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: 22.
- [y000000000150.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t58w/y000000000150.cfg#L1028): line, programmable, expansion, softkey_layout, dss_assets.

### t5x

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000070.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t5x/y000000000070.cfg#L457): line, programmable, expansion, softkey_layout.
- [y000000000095.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t5x/y000000000095.cfg#L479): line, programmable, expansion, softkey_layout.
- [y000000000096.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t5x/y000000000096.cfg#L480): line, programmable, expansion, softkey_layout.
- [y000000000097.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t5x/y000000000097.cfg#L480): line, programmable, expansion, softkey_layout.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t5x/%7B%24mac%7D.cfg#L521): line, expansion.

### t73u

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000186.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t73u/y000000000186.cfg#L484): line, programmable, expansion, softkey_layout.

### t73w

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000183.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t73w/y000000000183.cfg#L438): line, programmable, expansion, softkey_layout.

### t74u

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000187.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t74u/y000000000187.cfg#L484): line, programmable, expansion, softkey_layout.

### t74w

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000184.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t74w/y000000000184.cfg#L438): line, programmable, expansion, softkey_layout.

### t77u

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000188.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t77u/y000000000188.cfg#L484): line, programmable, expansion, softkey_layout.

### t85w

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000196.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t85w/y000000000196.cfg#L484): line, programmable, expansion, softkey_layout.

### t87w

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000197.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t87w/y000000000197.cfg#L484): line, programmable, expansion, softkey_layout.

### t88v

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000192.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t88v/y000000000192.cfg#L484): line, programmable, expansion, softkey_layout.

### t88w

- Parameter groups: line, programmable, expansion, softkey_layout.
- Key loops: expansion, expansion-1, expansion-2, line, programmable.
- Softkey URL states: call_failed, call_in, connecting, dialing, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, dialing, dialtone, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000192.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/t88w/y000000000192.cfg#L484): line, programmable, expansion, softkey_layout.

### vp530

- Parameter groups: line, memory, expansion.
- Key loops: expansion, line, memory.
- Softkey URL states: None found.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- Literal line type slots: 1, 2, 3, 4.
- Literal memory type slots: 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18.
- [y000000000023.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/vp530/y000000000023.cfg#L317): line, memory.
- [{$mac}.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/vp530/%7B%24mac%7D.cfg#L3015): line, memory, expansion.

### vp59

- Parameter groups: line, programmable, softkey_layout, dss_assets.
- Key loops: line.
- Softkey URL states: call_failed, call_in, connecting, ring_back, talking.
- Inline softkey state flags: call_failed, conferenced, connecting, held, hold, idle, incoming_call, on_talk, ring_back, transfer_connecting, transfer_ring_back.
- Legacy type-code comment hints: 22.
- [y000000000091.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/vp59/y000000000091.cfg#L1024): line, programmable, softkey_layout, dss_assets.

### w52p

- Parameter groups: None found.
- Key loops: None found.
- Softkey URL states: None found.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: No parsed type list; check source documentation.

### w56p

- Parameter groups: None found.
- Key loops: None found.
- Softkey URL states: None found.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: No parsed type list; check source documentation.

### w60b

- Parameter groups: None found.
- Key loops: None found.
- Softkey URL states: None found.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: No parsed type list; check source documentation.

### w70b

- Parameter groups: None found.
- Key loops: None found.
- Softkey URL states: None found.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: No parsed type list; check source documentation.

### w7xp

- Parameter groups: None found.
- Key loops: None found.
- Softkey URL states: None found.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: No parsed type list; check source documentation.

### w80

- Parameter groups: handset_keys.
- Key loops: None found.
- Softkey URL states: None found.
- Inline softkey state flags: None found.
- Legacy type-code comment hints: No parsed type list; check source documentation.
- [y000000000175.cfg](https://github.com/fusionpbx/fusionpbx/blob/be0d0e3220e6b3666775fa46db6f1accc60c79c6/resources/templates/provision/yealink/w80/y000000000175.cfg#L678): handset_keys.
