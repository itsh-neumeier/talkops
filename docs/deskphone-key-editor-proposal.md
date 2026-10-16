# Yealink desk phone provisioning and graphical key editor

Status: Approved by the user on 2026-10-16. Touch models use a scrollable DSS
overview with all logical keys.
Research date: 2026-10-16, Europe/Berlin.

## Requested scope

Add the Yealink desk phones listed in the Telekom CPBX help site. Keep the
desk phones that TalkOps already supports. Add a graphical key editor to
the provisioning settings for each desk phone. The user reviewed the
interactive T41P design and approved implementation on 2026-10-16.

The CPBX desk phone list contains 12 models: T41P, T41S, T46G, T46S, T48G,
T48S, T53, T54W, T57W, T73W, T85W and T87W. TalkOps already contains T54W
and T57W. The combined scope contains 21 desk phone models.

CP860, CP920 and CP935W are conference phones. The W-series devices are
DECT devices. EXP50 and EXP55 are expansion modules. These devices are
separate from the requested desk phone scope.

## Model research

The following table records capacities from Yealink documentation. A logical
key is a stored DSS key assignment. A physical key is a button on the phone.
These counts are different on phones with display pages. Touchscreen models
use controls inside the display instead of buttons beside the display.

| Models | Display | Key layout | Proposed logical keys | Catalog change | Source |
| --- | --- | --- | --- | --- | --- |
| T31G | 2.3-inch monochrome | 2 physical keys | 2 | Add layout metadata | [T3](#sources) |
| T33G | 2.4-inch color | 4 physical keys; 4 pages of 3 assignments when paging is active | 12 | Correct current limit of 4 | [T3](#sources) |
| T41P, T41S | 2.7-inch monochrome | 6 physical keys; 3 pages of 5 assignments when paging is active | 15 each | Add both models | [T41](#sources), [DSS](#sources) |
| T42U | 2.7-inch monochrome | 6 physical keys; 3 pages of 5 assignments when paging is active | 15 | Add layout metadata | [T4U](#sources) |
| T43U | 3.7-inch monochrome | 8 physical keys; 3 pages of 7 assignments when paging is active | 21 | Add layout metadata | [T4U](#sources) |
| T46G, T46S, T46U | 4.3-inch color | 10 physical keys; 3 pages of 9 assignments when paging is active | 27 each | Add G/S; correct U limit of 40 | [DSS](#sources), [T4U](#sources) |
| T48G, T48S, T48U | 7-inch touch | Touchscreen DSS view | 29 each | Add G/S; add U layout metadata | [DSS](#sources), [T4U](#sources) |
| T53, T53W | 3.7-inch monochrome | 8 physical keys; 3 pages of 7 assignments when paging is active | 21 each | Add T53; add W layout metadata | [T5](#sources) |
| T54W | 4.3-inch color | 10 physical keys; 3 pages of 9 assignments when paging is active | 27 | Add layout metadata | [T5](#sources) |
| T57W | 7-inch touch | Touchscreen DSS view | 29 | Add layout metadata | [T5](#sources) |
| T58W | 7-inch touch | Touchscreen DSS view | 27 | Add layout metadata | [T58](#sources) |
| VP59 | 8-inch touch | Touchscreen DSS view | 27 | Correct current limit of 29 | [VP59](#sources) |
| T73W | 2.8-inch color | 8 physical keys; separate page control; 5 pages of 8 assignments | 40 | Add model | [T73](#sources) |
| T85W | 5.5-inch color | 12 physical keys; separate page control; 5 pages of 12 assignments | 60 | Add model | [T85](#sources) |
| T87W | 7-inch touch | Touchscreen DSS view | 84 | Add model | [T87](#sources) |

The counts and display types are research results, not hardware test results.
Physical key maps are explicit. Touchscreen models use an expanded,
scrollable editor overview. The editor does not claim to reproduce the
folded idle screen or a firmware-specific touchscreen page layout.

For older phones with page switching, the page key can replace an editable
physical position when later pages are used. The editor must use explicit
page-to-key maps. Newer T73W and T85W phones have a separate page control.

## Proposed appearance

- Show a small model graphic in the phone list and model selector.
- Show a larger graphic in the phone detail page under function keys.
- Draw the phone case and display from model layout data. Use original SVG
  or HTML/CSS assets. Use the source diagrams as references. Store the assets
  in TalkOps so the page needs no image connection to an external site.
- Use a full phone silhouette for model identification. Use a larger display
  and key area for editing, so labels stay readable.
- Put key labels at the correct positions. Make both the button and its label
  selectable. Show the current page and each logical key number.
- Use a restrained status symbol for the configured function. Do not show
  live BLF state unless TalkOps has real status data for that state.
- Keep a list view for keyboard users, small screens and large key sets.
- Use the shared TalkOps modal, tabs, colors and responsive layout rules.
  A narrow screen uses a full-width dialog. Escape and Cancel discard the
  dialog draft. Focus returns to the selected key.

The first review preview shows the T41P display and a working key dialog.
It uses fictional contacts. It does not contact a phone or the TalkOps API.
It is a schematic design, not a pixel-exact firmware display reproduction.

## Key dialog

The title identifies the logical key, page and physical position.

1. Select the function: speed dial, BLF, own line or unused.
2. Select the target source when the function needs a number.
3. Review the number, display label and assigned account.
4. Select Apply to update the local page draft.

The target sources are:

| Source | Controls | Result |
| --- | --- | --- |
| Phonebook | Book selector, contact search, contact result, number selector | Copy the chosen number and contact name into the key. Offer work, mobile and other numbers separately. |
| Internal extension | Extension selector with number and name | Use **<number> for speed dial. Use the existing bare extension target for BLF. |
| Manual number | Number or feature code, editable display label | Store the entered dial target without inferred internal or external conversion. |

The book selector offers global contacts and the books assigned to the
phone. A combined view searches those books. Do not silently add another
phonebook to the phone. If assignment changes have not been saved, use the
current draft assignments in the picker and save them with the key changes.

BLF offers internal extensions. Do not offer an external phonebook number
as a monitored extension. Keep the existing *8*<number> pickup behavior.

The account selector shows assigned accounts by number and name. Keep an
existing missing account visible as an error that the user can correct.
If no account exists, explain how to assign an extension before provisioning.

Standard removes the explicit key override. Unused writes an explicit empty
assignment. These actions have different results for keys with default lines.

## Save and contact behavior

Offer Save and Save & Resync. Apply in the key dialog changes only the page
draft. Save & Resync first saves the phone and account changes. Send resync
only after save succeeds. If notification fails or the phone is offline,
report that state separately from the successful save.

Recommendation: Copy the selected contact number and name into the key.
This keeps the current key storage. Later contact edits do not silently
change a phone key. A persistent contact link with automatic updates is a
separate option that needs a data model and deletion rules. The user has not
selected that option.

## Implementation boundary

Approved implementation boundary:

- `presets/phones/yealink.yaml`: New models, corrected capacities, documented
  sources and explicit display/key layout metadata.
- `crates/talkops-provisioning/src/catalog.rs`: Parse and validate layout
  metadata. Make model capabilities explicit where required.
- `crates/talkops-api/src/routes/phones.rs`: Expose additive layout metadata
  and enforce per-model function limits. The current common configuration
  route and template must be checked for every added model and firmware.
- `crates/talkops-api/src/routes/provisioning.rs`: Extend the common CFG
  filename recognition for the new models. Yealink lists
  `y0000000000183.cfg` for T73W, `y0000000000196.cfg` for T85W and
  `y0000000000197.cfg` for T87W. These names have 13 digits after y. The
  current `is_common_cfg` function accepts exactly 12 digits. Add tests for
  documented filenames and invalid inputs. Confirm the documented filenames
  with device requests when hardware is available.
- `web/src/lib/api.ts`: Describe the additive metadata.
- New shared key editor, layout and contact picker components in
  `web/src/lib/components/`. Use the same draft data for graphics and list.
- `web/src/routes/phones/`: Use the editor and model graphics in phone setup.
- `web/src/lib/i18n/en.ts` and `de.ts`: All new UI text.
- English/German Yealink guides, functional tests and this task summary.

No new package dependency or database migration is expected for copied
contact values. Preserve the existing line_keys JSON representation.
Keep default line assignments and the AX Wi-Fi handset restrictions.

Correcting the T46U and VP59 capacities affects stored overrides above the
documented limit. Keep such overrides visible for review. Never drop them
silently. Check the existing data and define a compatible edit path before
enforcing smaller limits on saved phone records.

Do not enable ringtones, wallpapers or optional phone functions from a
family assumption. Each model needs documented limits. Mark real phone
tests as pending until hardware checks are available.

## Verification

Planned implementation checks:

- Model coverage: all 21 models have a source and a complete layout.
- Layout maps: each supported key has the right position; navigation controls
  do not overwrite configurable keys; no duplicate or unreachable key exists.
- Catalog and API: models load, User-Agent identification works, invalid slots
  and unsupported function types fail, account limits and existing layouts
  remain compatible.
- Provisioning: generated common and MAC configuration, speed dial, BLF,
  pickup, default, unused keys and resync authentication.
- Contact picker: book assignment filters, multiple numbers, empty results,
  manual targets, labels and explicit internal dialing.
- Browser: keyboard and touch, focus return, cancel, reset, save/reload,
  save failure, offline resync, list/graphic consistency, light/dark themes
  and widths down to 320 pixels.
- Run the Rust and web checks from `.github/workflows/ci.yml`. Report local
  service prerequisites and real-device tests separately.

Review preview checks passed on 2026-10-16 with headless Edge and Playwright:
key dialogs, contact/mobile selection, label application, internal dial
targets, BLF targets, last key access, list view, invalid-number feedback,
mock save feedback, Escape and dialog bounds. Widths: 780, 736, 390 and 320.
No browser script errors occurred. These checks validate the preview only.

## Sources

- CPBX model list and reference diagrams:
  https://cpbx-hilfe.deutschland-lan.de/de/endgeraete-und-clients/endgeraete-yealink
- T41: https://www.yealink.com/upfiles/products/201808/1534730197968.pdf
- T3: https://www.yealink.com/website-service/attachment/product/documents/20240823/202408231210042832c4d.pdf
- T4U: https://www.yealink.com/website-service/attachment/product_resource/documents/20240204/202402040855167303286a979457fa5fb9aec9faaaec7.pdf
- T5: https://www.yealink.com/website-service/attachment/product/documents/20240702/202407020628251454a7b.pdf
  The search index returned the specification table. Direct retrieval failed
  during this review. Check the source again before implementation.
- DSS: https://support.yealink.com/docs/sip-t85w/dss-keys/5d994e5739cb43a0830a6868633ebcd2
- T58: https://support.yealink.com/docs/sip-t58w/yealink-sip-t58w-datasheet/53449c354af8436eba417305e5c121d1
- VP59: https://support.yealink.com/docs/vp59/yealink-sip-vp59-datasheet/9e9914c4bd504a7eb2327d058ec16934
- T73: https://support.yealink.com/docs/sip-t73w/yealink-sip-t73w-datasheet/54808c276f624adeb2d15783b32f133c
- T85: https://support.yealink.com/docs/sip-t85w/yealink-sip-t85w-datasheet/4d6a6d5f370e48fca7171f3b61ae5134
- T87: https://support.yealink.com/docs/sip-t87w/yealink-sip-t87w-datasheet/8ff1cfd14e074150997c07aa50dbd45d
- Common CFG filenames: https://support.yealink.com/docs/sip-t44w/manage-configuration-files/470c62f9fe4c47b3af969dafdb965973

Language review used the repository documentation rules. The full official
ASD-STE100 standard was unavailable. Full STE review is pending.
